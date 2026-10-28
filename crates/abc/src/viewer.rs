//! The viewer's state for one worktree: the open file as abcd sent it, its
//! highlighting once that's done, the cursor, and how far it's scrolled.
//! Motions are in `motion`; drawing is in `ui::viewer`.

use std::cell::Cell;
use std::collections::HashMap;

use abc_core::model::FileContents;

use ratatui::text::Span;

use crate::explorer::NodePath;
use crate::highlight::{self, Lang};
use crate::motion::{self, Pos};

/// Spaces a tab takes in the viewer by default (`viewer.tab_width`).
pub const TAB_WIDTH: usize = 4;

/// Spaces a tab takes now: a display setting every line shares, so it lives
/// here rather than threaded through every drawing function.
static TAB: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(TAB_WIDTH);

/// Sets the spaces a tab takes, from the `viewer.tab_width` setting.
pub fn set_tab_width(width: usize) {
    TAB.store(width.max(1), std::sync::atomic::Ordering::Relaxed);
}

fn tab_width() -> usize {
    TAB.load(std::sync::atomic::Ordering::Relaxed)
}

#[derive(Debug, Clone)]
pub enum Document {
    Loading,
    Failed(String),
    Binary { size: u64 },
    TooLarge { size: u64, limit: u64 },
    Text(Text),
}

#[derive(Debug, Clone)]
pub struct Text {
    /// The file's lines as the viewer shows them (see `shown`).
    pub lines: Vec<String>,
    /// The lines shown differently from the file, as the file has them, for copying.
    originals: HashMap<usize, String>,
    pub lang: Lang,
    /// Spans per line; None until highlighting finishes (or for plain text).
    pub spans: Option<Vec<Vec<highlight::Span>>>,
}

impl Text {
    pub fn new(source: &str, lang: Lang) -> Self {
        let mut originals = HashMap::new();
        let lines = source
            .split('\n')
            .enumerate()
            .map(|(i, line)| {
                let line = line.strip_suffix('\r').unwrap_or(line);
                let shown = shown(line);
                if shown != line {
                    originals.insert(i, line.to_string());
                }
                shown
            })
            .collect();
        Self {
            lines,
            originals,
            lang,
            spans: None,
        }
    }

    /// The text from one place to another, both included, as the file has it.
    /// By line, it's the whole lines, each ending with a newline.
    pub fn copy(&self, from: Pos, to: Pos, by_line: bool) -> String {
        let mut out = String::new();
        let last = to.line.min(self.lines.len().saturating_sub(1));
        for i in from.line..=last {
            let len = self.lines[i].chars().count();
            let start = if by_line || i > from.line {
                0
            } else {
                from.col
            };
            let end = if by_line || i < last {
                len
            } else {
                (to.col + 1).min(len)
            };
            out.push_str(&self.original(i, start, end));
            if by_line || i < last {
                out.push('\n');
            }
        }
        out
    }

    /// Shown columns `start..end` of line `i`, as the file has them: a tab
    /// partly in range is copied whole.
    fn original(&self, i: usize, start: usize, end: usize) -> String {
        let Some(line) = self.originals.get(&i) else {
            return self.lines[i]
                .chars()
                .skip(start)
                .take(end.saturating_sub(start))
                .collect();
        };
        let mut col = 0;
        let mut out = String::new();
        for c in line.chars() {
            let width = if c == '\t' { tab_width() } else { 1 };
            if col + width > start && col < end {
                out.push(c);
            }
            col += width;
        }
        out
    }

    /// The lines as one string, for the highlighter.
    pub fn source(&self) -> String {
        self.lines.join("\n")
    }

    /// Highlights now, on this thread. The app does it off the UI thread instead.
    pub fn highlighted(mut self) -> Self {
        self.spans = highlight::highlight(&self.source(), self.lang);
        self
    }
}

/// A line as the viewer shows it: tabs as spaces, no `\r` from a CRLF ending,
/// and other control characters as their symbols (␛), so a file can't send
/// the terminal commands.
pub(crate) fn shown(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    for c in line.chars() {
        match c {
            '\t' => out.extend(std::iter::repeat_n(' ', tab_width())),
            '\0'..='\x1f' => out.push(char::from_u32(0x2400 + c as u32).unwrap_or('\u{fffd}')),
            '\x7f' => out.push('\u{2421}'),
            c if c.is_control() => out.push('\u{fffd}'),
            c => out.push(c),
        }
    }
    out
}

impl Document {
    pub fn from_contents(contents: FileContents, name: &str) -> Self {
        match contents {
            FileContents::Text { text } => Self::Text(Text::new(&text, Lang::detect(name, &text))),
            FileContents::Binary { size } => Self::Binary { size },
            FileContents::TooLarge { size, limit } => Self::TooLarge { size, limit },
        }
    }
}

#[derive(Debug, Clone)]
pub struct Viewer {
    pub path: NodePath,
    pub document: Document,
    /// The first line on screen.
    pub scroll: usize,
    pub cursor: Pos,
    /// The column `j` and `k` aim for, so a short line on the way doesn't pull
    /// the cursor left for good. `usize::MAX` after `$`: every line's end.
    want: usize,
    /// Where `ctrl-o` goes back to, the latest last.
    jumps: Vec<Pos>,
    /// Visual mode's selection, from its anchor to the cursor.
    pub selection: Option<Selection>,
    /// A line to go to once the text arrives, when find in files opened the file.
    pub landing: Option<usize>,
    /// Rows the text took when last drawn, for paging.
    pub(crate) height: Cell<usize>,
    /// The first column on screen. Drawing moves it to keep the cursor in view.
    pub(crate) hscroll: Cell<usize>,
}

/// An @-mention of `path` to paste into a Claude Code chat, with the lines
/// (counting from 1) when there are some: "@src/main.rs:12-20".
pub fn reference(path: &[String], lines: Option<(usize, usize)>) -> String {
    let path = path.join("/");
    match lines {
        None => format!("@{path}"),
        Some((first, last)) if first == last => format!("@{path}:{first}"),
        Some((first, last)) => format!("@{path}:{first}-{last}"),
    }
}

/// Visual mode: where it started, and whether it takes whole lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub anchor: Pos,
    pub by_line: bool,
}

/// Where `zt`, `zz` and `zb` put the cursor's line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    Top,
    Middle,
    Bottom,
}

/// Jumps `ctrl-o` remembers.
const MAX_JUMPS: usize = 100;

impl Viewer {
    pub fn new(path: NodePath, document: Document) -> Self {
        Self {
            path,
            document,
            scroll: 0,
            cursor: Pos::default(),
            want: 0,
            jumps: Vec::new(),
            selection: None,
            landing: None,
            height: Cell::new(30),
            hscroll: Cell::new(0),
        }
    }

    pub fn name(&self) -> &str {
        self.path.last().map_or("", String::as_str)
    }

    /// The text's lines; none for a file that isn't text, or isn't here yet.
    pub fn lines(&self) -> &[String] {
        match &self.document {
            Document::Text(t) => &t.lines,
            _ => &[],
        }
    }

    pub fn has_text(&self) -> bool {
        matches!(self.document, Document::Text(_))
    }

    /// The selection's first and last places, in order, and whether it's by line.
    pub fn selected(&self) -> Option<(Pos, Pos, bool)> {
        let s = self.selection?;
        let (from, to) = if s.anchor <= self.cursor {
            (s.anchor, self.cursor)
        } else {
            (self.cursor, s.anchor)
        };
        Some((from, to, s.by_line))
    }

    /// `v` or `V`: starts visual mode, switches between by character and by
    /// line, or leaves it when it's already that kind.
    pub fn visual(&mut self, by_line: bool) {
        self.selection = match self.selection {
            Some(s) if s.by_line == by_line => None,
            Some(s) => Some(Selection { by_line, ..s }),
            None => Some(Selection {
                anchor: self.cursor,
                by_line,
            }),
        };
    }

    /// `o`: the cursor goes to the selection's other end.
    pub fn other_end(&mut self) {
        if let Some(s) = &mut self.selection {
            let anchor = std::mem::replace(&mut s.anchor, self.cursor);
            self.go(anchor);
        }
    }

    /// The selection's text, as the file has it; visual mode ends with the
    /// cursor at its start, as Vim leaves it after a yank.
    pub fn take_selection(&mut self) -> Option<(String, Pos, Pos, bool)> {
        let (from, to, by_line) = self.selected()?;
        let Document::Text(text) = &self.document else {
            return None;
        };
        let copied = text.copy(from, to, by_line);
        self.selection = None;
        self.go(from);
        Some((copied, from, to, by_line))
    }

    /// `count` lines from the cursor's, as the file has them.
    pub fn copy_lines(&self, count: usize) -> Option<String> {
        let Document::Text(text) = &self.document else {
            return None;
        };
        let line = self.cursor.line;
        let last = line + count.max(1) - 1;
        Some(text.copy(Pos::new(line, 0), Pos::new(last, 0), true))
    }

    fn rows(&self) -> usize {
        self.height.get().max(1)
    }

    /// The furthest the text can scroll: its last line at the bottom.
    pub fn max_scroll(&self) -> usize {
        self.lines().len().saturating_sub(self.rows())
    }

    /// The first line on screen: the scroll, moved if need be to show the
    /// cursor (after the viewer got shorter, say).
    pub fn top(&self) -> usize {
        let line = self.cursor.line;
        self.scroll
            .min(self.max_scroll())
            .clamp(line.saturating_sub(self.rows() - 1), line)
    }

    /// The first and last lines on screen.
    pub fn screen(&self) -> (usize, usize) {
        let top = self.top();
        let last = (top + self.rows() - 1).min(self.lines().len().saturating_sub(1));
        (top, last.max(top))
    }

    /// Moves the cursor to `to`, kept on the text, and scrolls to show it.
    pub fn go(&mut self, to: Pos) {
        self.cursor = motion::clamp(self.lines(), to);
        self.want = self.cursor.col;
        self.follow();
    }

    /// Moves the cursor where `motion` takes it, `count` times over.
    pub fn go_by(&mut self, count: usize, motion: impl Fn(&[String], Pos) -> Pos) {
        let to = self.repeat(count, motion);
        self.go(to);
    }

    /// Like `go_by`, as a jump `ctrl-o` comes back from.
    pub fn jump_by(&mut self, count: usize, motion: impl Fn(&[String], Pos) -> Pos) {
        let to = self.repeat(count, motion);
        self.jump(to);
    }

    fn repeat(&self, count: usize, motion: impl Fn(&[String], Pos) -> Pos) -> Pos {
        (0..count.max(1)).fold(self.cursor, |p, _| motion(self.lines(), p))
    }

    /// Goes to a line by number (`12G`, `H`, `50%`, `:12`), at its first
    /// non-blank, as a jump. Past the end is the last line.
    pub fn jump_to_line(&mut self, line: usize) {
        self.jump_by(1, |l, _| {
            motion::first_non_blank(l, line.min(l.len().saturating_sub(1)))
        });
    }

    /// Puts the cursor and the view back as they were, as cancelling `/` does.
    pub fn restore(&mut self, cursor: Pos, scroll: usize) {
        self.cursor = cursor;
        self.want = cursor.col;
        self.scroll = scroll;
        self.follow();
    }

    /// Goes to `to` as a jump (`gg`, `G`, `%`…): `ctrl-o` comes back here.
    pub fn jump(&mut self, to: Pos) {
        let from = self.cursor;
        self.go(to);
        if self.cursor != from {
            if self.jumps.len() == MAX_JUMPS {
                self.jumps.remove(0);
            }
            self.jumps.push(from);
        }
    }

    /// `ctrl-o`: back to where the last jump left from.
    pub fn back(&mut self) {
        if let Some(from) = self.jumps.pop() {
            self.go(from);
        }
    }

    /// Moves `rows` lines down (negative is up), aiming for the wanted column.
    pub fn go_lines(&mut self, rows: isize) {
        self.go_line(self.cursor.line.saturating_add_signed(rows));
    }

    fn go_line(&mut self, line: usize) {
        self.cursor = motion::clamp(self.lines(), Pos::new(line, self.want));
        self.follow();
    }

    /// `$`: the end of this line, and of every line `j` and `k` reach after it.
    pub fn go_line_end(&mut self) {
        self.go(Pos::new(self.cursor.line, usize::MAX));
        self.want = usize::MAX;
    }

    /// Scrolls just enough to show the cursor.
    fn follow(&mut self) {
        let line = self.cursor.line;
        if line < self.scroll {
            self.scroll = line;
        } else if line >= self.scroll + self.rows() {
            self.scroll = line + 1 - self.rows();
        }
        self.scroll = self.scroll.min(self.max_scroll());
    }

    /// Moves the cursor just enough to keep it on screen.
    fn keep_cursor_on_screen(&mut self) {
        let line = self
            .cursor
            .line
            .clamp(self.scroll, self.scroll + self.rows() - 1);
        if line != self.cursor.line {
            self.go_line(line);
        }
    }

    /// Scrolls by `rows` (negative is up), within the text. The cursor moves
    /// only to stay on screen, as with `ctrl-e` and `ctrl-y`.
    pub fn scroll_by(&mut self, rows: isize) {
        self.scroll = self
            .scroll
            .saturating_add_signed(rows)
            .min(self.max_scroll());
        self.keep_cursor_on_screen();
    }

    /// Scrolls and moves the cursor together, as `ctrl-d` and `ctrl-f` do.
    pub fn scroll_with_cursor(&mut self, rows: isize) {
        self.scroll = self
            .scroll
            .saturating_add_signed(rows)
            .min(self.max_scroll());
        self.go_lines(rows);
    }

    /// `zt`, `zz`, `zb`: scrolls so the cursor's line is at the top, middle or bottom.
    pub fn place(&mut self, at: Place) {
        let line = self.cursor.line;
        self.scroll = match at {
            Place::Top => line,
            Place::Middle => line.saturating_sub(self.rows() / 2),
            Place::Bottom => line.saturating_sub(self.rows() - 1),
        }
        .min(self.max_scroll());
    }

    pub fn page(&self) -> isize {
        self.rows() as isize
    }

    pub fn to_top(&mut self) {
        self.scroll = 0;
        self.keep_cursor_on_screen();
    }

    pub fn to_end(&mut self) {
        self.scroll = self.max_scroll();
        self.keep_cursor_on_screen();
    }

    /// The first column on screen for text `room` columns wide: the last one
    /// drawn, moved just enough to show the cursor short of the last column,
    /// which a cut line gives to "…".
    pub(crate) fn left(&self, room: usize) -> usize {
        let Some(line) = self.lines().get(self.cursor.line) else {
            return 0;
        };
        let chars: Vec<char> = line.chars().collect();
        let col = self.cursor.col.min(chars.len().saturating_sub(1));
        let mut left = self.hscroll.get().min(col);
        while left < col && width(&chars[left..=col]) > room.saturating_sub(1) {
            left += 1;
        }
        self.hscroll.set(left);
        left
    }
}

/// Columns `chars` take on screen.
pub fn width(chars: &[char]) -> usize {
    let mut buf = [0; 4];
    chars
        .iter()
        .map(|c| Span::raw(&*c.encode_utf8(&mut buf)).width())
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn viewer(lines: usize) -> Viewer {
        let source = (1..=lines)
            .map(|n| format!("line {n}"))
            .collect::<Vec<_>>()
            .join("\n");
        let v = Viewer::new(
            vec!["f.txt".into()],
            Document::Text(Text::new(&source, Lang::Plain)),
        );
        v.height.set(10);
        v
    }

    fn text_viewer(source: &str, height: usize) -> Viewer {
        let v = Viewer::new(
            vec!["f.rs".into()],
            Document::Text(Text::new(source, Lang::Plain)),
        );
        v.height.set(height);
        v
    }

    #[test]
    fn j_and_k_keep_the_column_across_short_lines() {
        let mut v = text_viewer("let value = 1;\n\nlet other = 2;", 10);
        v.go(Pos::new(0, 8));
        v.go_lines(1);
        assert_eq!(v.cursor, Pos::new(1, 0));
        v.go_lines(1);
        assert_eq!(v.cursor, Pos::new(2, 8));
        v.go_line_end();
        v.go_lines(-2);
        assert_eq!(v.cursor, Pos::new(0, 13), "after $, every line's end");
    }

    #[test]
    fn moving_the_cursor_scrolls_just_enough_to_show_it() {
        let mut v = viewer(25);
        v.go_lines(12);
        assert_eq!((v.cursor.line, v.scroll), (12, 3));
        v.go_lines(-5);
        assert_eq!(v.scroll, 3, "still on screen, so no scrolling");
        v.go(Pos::new(0, 0));
        assert_eq!(v.scroll, 0);
    }

    #[test]
    fn scrolling_drags_the_cursor_only_to_keep_it_on_screen() {
        let mut v = viewer(25);
        v.go_lines(3);
        v.scroll_by(5);
        assert_eq!((v.scroll, v.cursor.line), (5, 5));
        v.scroll_by(-5);
        assert_eq!(v.cursor.line, 5);
        v.scroll_with_cursor(5);
        assert_eq!((v.scroll, v.cursor.line), (5, 10));
    }

    #[test]
    fn ctrl_o_goes_back_through_jumps() {
        let mut v = viewer(25);
        v.jump(Pos::new(20, 0));
        v.go_lines(1);
        v.jump(Pos::new(3, 0));
        v.back();
        assert_eq!(v.cursor.line, 21);
        v.back();
        assert_eq!(v.cursor.line, 0);
        v.back();
        assert_eq!(v.cursor.line, 0, "nothing left to go back to");
    }

    #[test]
    fn zt_zz_and_zb_place_the_cursor_line() {
        let mut v = viewer(25);
        v.go_lines(12);
        v.place(Place::Top);
        assert_eq!(v.scroll, 12);
        v.place(Place::Middle);
        assert_eq!(v.scroll, 7);
        v.place(Place::Bottom);
        assert_eq!(v.scroll, 3);
        v.go_lines(12);
        v.place(Place::Top);
        assert_eq!(v.scroll, 15, "no further than the last line at the bottom");
    }

    #[test]
    fn the_view_follows_the_cursor_sideways() {
        let v = &mut text_viewer(&"x".repeat(50), 10);
        v.go(Pos::new(0, 30));
        assert_eq!(v.left(20), 12, "the cursor short of the last column");
        v.go(Pos::new(0, 25));
        assert_eq!(v.left(20), 12, "still in view, so no scrolling");
        v.go(Pos::new(0, 4));
        assert_eq!(v.left(20), 4);
    }

    #[test]
    fn the_cursor_stays_on_screen_after_the_viewer_shrinks() {
        let mut v = viewer(25);
        v.go_lines(9);
        v.height.set(5);
        assert_eq!(v.top(), 5);
        assert_eq!(v.screen(), (5, 9));
    }

    #[test]
    fn control_characters_never_reach_the_terminal() {
        let t = Text::new("dos line\r\nan \x1b[31m escape\r\n\x07", Lang::Plain);
        assert_eq!(t.lines, ["dos line", "an \u{241b}[31m escape", "\u{2407}"]);
    }

    #[test]
    fn copying_gives_the_text_as_the_file_has_it() {
        let t = Text::new("fn main() {\r\n\tlet x = 1;\r\n}", Lang::Rust);
        assert_eq!(t.copy(Pos::new(0, 3), Pos::new(0, 6), false), "main");
        assert_eq!(
            t.copy(Pos::new(0, 10), Pos::new(1, 6), false),
            "{\n\tlet",
            "the tab, not its spaces; no \\r"
        );
        assert_eq!(
            t.copy(Pos::new(1, 2), Pos::new(1, 2), false),
            "\t",
            "a tab partly selected"
        );
        assert_eq!(
            t.copy(Pos::new(1, 5), Pos::new(2, 0), true),
            "\tlet x = 1;\n}\n",
            "by line: whole lines, each with its newline"
        );
    }

    #[test]
    fn v_and_shift_v_start_switch_and_leave_visual_mode() {
        let mut v = text_viewer("one two\nthree", 10);
        v.go(Pos::new(0, 4));
        v.visual(false);
        v.go(Pos::new(0, 1));
        assert_eq!(v.selected(), Some((Pos::new(0, 1), Pos::new(0, 4), false)));
        v.visual(true);
        assert_eq!(v.selected(), Some((Pos::new(0, 1), Pos::new(0, 4), true)));
        v.other_end();
        assert_eq!(v.cursor, Pos::new(0, 4));
        v.visual(true);
        assert_eq!(v.selection, None);
    }

    #[test]
    fn copying_a_selection_ends_visual_mode_at_its_start() {
        let mut v = text_viewer("one two\nthree", 10);
        v.go(Pos::new(1, 2));
        v.visual(false);
        v.go(Pos::new(0, 4));
        let (copied, ..) = v.take_selection().unwrap();
        assert_eq!(copied, "two\nthr");
        assert_eq!((v.selection, v.cursor), (None, Pos::new(0, 4)));
        assert_eq!(
            v.copy_lines(5).unwrap(),
            "one two\nthree\n",
            "no further than the end"
        );
    }

    #[test]
    fn a_reference_mentions_the_file_and_its_lines() {
        let path: Vec<String> = ["src", "main.rs"].map(String::from).to_vec();
        assert_eq!(reference(&path, None), "@src/main.rs");
        assert_eq!(reference(&path, Some((12, 12))), "@src/main.rs:12");
        assert_eq!(reference(&path, Some((12, 20))), "@src/main.rs:12-20");
    }

    #[test]
    fn tabs_become_spaces() {
        let t = Text::new("\tfn x() {}\n\t\tx", Lang::Rust);
        assert_eq!(t.lines, ["    fn x() {}", "        x"]);
    }

    #[test]
    fn scrolling_stops_with_the_last_line_at_the_bottom() {
        let mut v = viewer(25);
        v.scroll_by(100);
        assert_eq!(v.scroll, 15);
        v.scroll_by(-3);
        assert_eq!(v.scroll, 12);
        v.scroll_by(-100);
        assert_eq!(v.scroll, 0);
        v.to_end();
        assert_eq!(v.scroll, 15);
    }

    #[test]
    fn a_short_file_never_scrolls() {
        let mut v = viewer(4);
        v.scroll_by(5);
        assert_eq!(v.scroll, 0);
    }

    #[test]
    fn contents_become_documents() {
        let text = Document::from_contents(
            FileContents::Text {
                text: "a\nb".into(),
            },
            "x.py",
        );
        assert!(
            matches!(text, Document::Text(Text { lang: Lang::Python, ref lines, .. }) if lines.len() == 2)
        );
        assert!(matches!(
            Document::from_contents(FileContents::Binary { size: 9 }, "a.png"),
            Document::Binary { size: 9 }
        ));
    }
}
