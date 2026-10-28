//! The viewer's state for one worktree: the open file as abcd sent it, its
//! highlighting once that's done, the cursor, and how far it's scrolled.
//! Motions are in `motion`; drawing is in `ui::viewer`.

use std::cell::Cell;

use abc_core::model::FileContents;

use ratatui::text::Span;

use crate::explorer::NodePath;
use crate::highlight::{self, Lang};
use crate::motion::{self, Pos};

/// Spaces a tab takes in the viewer (`viewer.tab_width`).
pub const TAB_WIDTH: usize = 4;

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
    /// The file's lines, tabs already expanded.
    pub lines: Vec<String>,
    pub lang: Lang,
    /// Spans per line; None until highlighting finishes (or for plain text).
    pub spans: Option<Vec<Vec<highlight::Span>>>,
}

impl Text {
    pub fn new(source: &str, lang: Lang) -> Self {
        let lines = source.split('\n').map(shown).collect();
        Self {
            lines,
            lang,
            spans: None,
        }
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
fn shown(line: &str) -> String {
    let line = line.strip_suffix('\r').unwrap_or(line);
    let mut out = String::with_capacity(line.len());
    for c in line.chars() {
        match c {
            '\t' => out.extend(std::iter::repeat_n(' ', TAB_WIDTH)),
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
            FileContents::Text { text } => Self::Text(Text::new(&text, Lang::of(name))),
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
    /// Rows the text took when last drawn, for paging.
    pub(crate) height: Cell<usize>,
    /// The first column on screen. Drawing moves it to keep the cursor in view.
    pub(crate) hscroll: Cell<usize>,
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
