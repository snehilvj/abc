//! The viewer's state for one worktree: the open file as abcd sent it, its
//! highlighting once that's done, and how far it's scrolled. Drawing is in
//! `ui::viewer`.

use std::cell::Cell;

use abc_core::model::FileContents;

use crate::explorer::NodePath;
use crate::highlight::{self, Lang, Span};

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
    pub spans: Option<Vec<Vec<Span>>>,
}

impl Text {
    pub fn new(source: &str, lang: Lang) -> Self {
        let lines = source
            .split('\n')
            .map(|l| l.replace('\t', &" ".repeat(TAB_WIDTH)))
            .collect();
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
    /// Rows the text took when last drawn, for paging.
    pub(crate) height: Cell<usize>,
}

impl Viewer {
    pub fn new(path: NodePath, document: Document) -> Self {
        Self {
            path,
            document,
            scroll: 0,
            height: Cell::new(30),
        }
    }

    pub fn name(&self) -> &str {
        self.path.last().map_or("", String::as_str)
    }

    fn lines(&self) -> usize {
        match &self.document {
            Document::Text(t) => t.lines.len(),
            _ => 0,
        }
    }

    /// The furthest the text can scroll: its last line at the bottom.
    pub fn max_scroll(&self) -> usize {
        self.lines().saturating_sub(self.height.get().max(1))
    }

    /// Scrolls by `rows` (negative is up), within the text.
    pub fn scroll_by(&mut self, rows: isize) {
        self.scroll = self
            .scroll
            .saturating_add_signed(rows)
            .min(self.max_scroll());
    }

    pub fn page(&self) -> isize {
        self.height.get().max(1) as isize
    }

    pub fn to_top(&mut self) {
        self.scroll = 0;
    }

    pub fn to_end(&mut self) {
        self.scroll = self.max_scroll();
    }
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
