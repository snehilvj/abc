//! Find in files: the query, abcd's results as they arrive, and which line is
//! selected. abcd does the searching; the client never reads files. Drawing
//! is in `ui::find`.

use std::cell::Cell;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use abc_core::model::FoundFile;
use tui_input::Input;

/// Result rows on screen by default (the `find.rows` setting).
pub const ROWS: usize = 20;
/// The most matching lines one search returns by default (`find.max_lines`).
pub const MAX_LINES: usize = 1000;
/// How long typing pauses before a search starts by default (`find.delay_ms`).
pub const DELAY: Duration = Duration::from_millis(150);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Progress {
    /// Nothing searched for, or the query was cleared.
    Idle,
    Searching,
    /// `capped` when more lines matched than a search returns.
    Done {
        capped: bool,
    },
    Failed(String),
}

/// A row of results: a file, or one of its lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    File(usize),
    Line(usize, usize),
}

#[derive(Debug, Clone)]
pub struct Find {
    pub input: Input,
    /// The worktree the results are from.
    pub worktree: Option<PathBuf>,
    /// When the query last changed, while its search has yet to start.
    pub typed_at: Option<Instant>,
    /// The latest search; answers to earlier ones are dropped.
    pub id: u64,
    /// What the results are for.
    pub query: String,
    /// In path order, without regard to case.
    pub files: Vec<FoundFile>,
    pub progress: Progress,
    /// The selected line, by path and number, so it stays put as results arrive.
    pub selected: Option<(String, u32)>,
    /// The first row on screen; drawing keeps the selection in view.
    pub(crate) offset: Cell<usize>,
    /// Result rows on screen.
    pub rows: usize,
    /// The most matching lines a search returns.
    pub max_lines: usize,
    /// How long typing pauses before a search starts.
    pub delay: Duration,
}

impl Default for Find {
    fn default() -> Self {
        Self {
            input: Input::default(),
            worktree: None,
            typed_at: None,
            id: 0,
            query: String::new(),
            files: Vec::new(),
            progress: Progress::Idle,
            selected: None,
            offset: Cell::new(0),
            rows: ROWS,
            max_lines: MAX_LINES,
            delay: DELAY,
        }
    }
}

impl Find {
    /// The query changed at `at`: its search starts when typing pauses.
    pub fn edited(&mut self, at: Instant) {
        self.typed_at = Some(at);
    }

    /// Whether typing has paused long enough at `now` for its search to start.
    pub fn due(&self, now: Instant) -> bool {
        self.typed_at
            .is_some_and(|typed| now.saturating_duration_since(typed) >= self.delay)
    }

    /// Starts a search for the query as it is: its id and text. Clearing the
    /// query clears the results and starts nothing.
    pub fn start(&mut self) -> Option<(u64, String)> {
        self.typed_at = None;
        self.id += 1;
        self.query = self.input.value().to_string();
        self.files.clear();
        self.selected = None;
        self.offset.set(0);
        if self.query.is_empty() {
            self.progress = Progress::Idle;
            return None;
        }
        self.progress = Progress::Searching;
        Some((self.id, self.query.clone()))
    }

    /// Takes in files search `id` found. The first line found is selected.
    pub fn found(&mut self, id: u64, files: Vec<FoundFile>) {
        if id != self.id {
            return;
        }
        for file in files {
            let at = self
                .files
                .partition_point(|f| order(&f.path) < order(&file.path));
            self.files.insert(at, file);
        }
        if self.selected.is_none() {
            self.selected = self.line_at(0);
        }
    }

    pub fn done(&mut self, id: u64, capped: bool) {
        if id == self.id {
            self.progress = Progress::Done { capped };
        }
    }

    pub fn failed(&mut self, id: u64, why: String) {
        if id == self.id {
            self.progress = Progress::Failed(why);
        }
    }

    /// Every row: each file, then its lines.
    pub fn rows(&self) -> Vec<Row> {
        let mut rows = Vec::new();
        for (f, file) in self.files.iter().enumerate() {
            rows.push(Row::File(f));
            rows.extend((0..file.lines.len()).map(|l| Row::Line(f, l)));
        }
        rows
    }

    /// Matching lines found so far.
    pub fn lines(&self) -> usize {
        self.files.iter().map(|f| f.lines.len()).sum()
    }

    /// The `i`th matching line, counting through every file.
    fn line_at(&self, i: usize) -> Option<(String, u32)> {
        self.files
            .iter()
            .flat_map(|f| f.lines.iter().map(move |l| (f.path.clone(), l.number)))
            .nth(i)
    }

    /// Where the selected line is among all the lines.
    fn selected_index(&self) -> Option<usize> {
        let (path, number) = self.selected.as_ref()?;
        self.files
            .iter()
            .flat_map(|f| f.lines.iter().map(move |l| (&f.path, l.number)))
            .position(|(p, n)| p == path && n == *number)
    }

    /// The selected line's row.
    pub fn selected_row(&self) -> Option<usize> {
        let (path, number) = self.selected.as_ref()?;
        self.rows().iter().position(|row| match *row {
            Row::Line(f, l) => {
                &self.files[f].path == path && self.files[f].lines[l].number == *number
            }
            Row::File(_) => false,
        })
    }

    pub fn down(&mut self) {
        let next = self
            .selected_index()
            .map_or(0, |i| (i + 1).min(self.lines().saturating_sub(1)));
        self.selected = self.line_at(next);
    }

    pub fn up(&mut self) {
        let previous = self.selected_index().map_or(0, |i| i.saturating_sub(1));
        self.selected = self.line_at(previous);
    }
}

/// Path order without regard to case, as the explorer sorts.
fn order(path: &str) -> (String, &str) {
    (path.to_lowercase(), path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use abc_core::model::FoundLine;

    fn file(path: &str, numbers: &[u32]) -> FoundFile {
        FoundFile {
            path: path.into(),
            lines: numbers
                .iter()
                .map(|&number| FoundLine {
                    number,
                    text: format!("line {number}"),
                })
                .collect(),
        }
    }

    fn searching(query: &str) -> (Find, u64) {
        let mut find = Find {
            input: Input::new(query.into()),
            ..Find::default()
        };
        let (id, _) = find.start().expect("a search");
        (find, id)
    }

    #[test]
    fn a_search_starts_once_typing_pauses() {
        let mut find = Find {
            input: Input::new("gst".into()),
            ..Find::default()
        };
        let typed = Instant::now();
        find.edited(typed);
        assert!(!find.due(typed + Duration::from_millis(100)));
        assert!(find.due(typed + DELAY));
        assert_eq!(find.start(), Some((1, "gst".into())));
        assert_eq!(find.progress, Progress::Searching);
        assert!(!find.due(typed + DELAY * 2), "started once");
    }

    #[test]
    fn clearing_the_query_clears_the_results() {
        let (mut find, id) = searching("gst");
        find.found(id, vec![file("a.rs", &[1])]);
        find.input = Input::default();
        assert_eq!(find.start(), None);
        assert_eq!((find.files.len(), &find.progress), (0, &Progress::Idle));
    }

    #[test]
    fn results_arrive_in_path_order_and_the_selection_holds() {
        let (mut find, id) = searching("gst");
        find.found(id, vec![file("src/b.rs", &[3, 9])]);
        assert_eq!(find.selected, Some(("src/b.rs".into(), 3)));
        find.found(id, vec![file("README.md", &[1]), file("src/a.rs", &[2])]);
        let paths: Vec<&str> = find.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, ["README.md", "src/a.rs", "src/b.rs"]);
        assert_eq!(
            find.selected,
            Some(("src/b.rs".into(), 3)),
            "still the same line"
        );
        assert_eq!(find.selected_row(), Some(5));
        find.down();
        find.down();
        assert_eq!(
            find.selected,
            Some(("src/b.rs".into(), 9)),
            "no further than the last"
        );
        find.up();
        find.up();
        assert_eq!(find.selected, Some(("src/a.rs".into(), 2)));
        find.done(id, false);
        assert_eq!(find.progress, Progress::Done { capped: false });
    }

    #[test]
    fn answers_to_an_older_search_are_dropped() {
        let (mut find, old) = searching("gs");
        let (new, _) = find.start().unwrap();
        find.found(old, vec![file("a.rs", &[1])]);
        find.done(old, true);
        assert!(find.files.is_empty());
        assert_eq!(find.progress, Progress::Searching);
        find.failed(new, "abcd isn't connected".into());
        assert_eq!(
            find.progress,
            Progress::Failed("abcd isn't connected".into())
        );
    }
}
