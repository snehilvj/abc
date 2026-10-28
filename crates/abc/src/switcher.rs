//! The switcher's state: a fuzzy palette over worktrees or the current
//! worktree's files, matched with nucleo as the query is typed. Drawing is in
//! `ui::switcher`; the app fills it from what abcd sent.

use std::path::PathBuf;

use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo::{Config, Matcher, Utf32Str};
use tui_input::Input;

/// Rows of results on screen by default; the `switcher.rows` setting changes it.
pub const ROWS: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Worktrees,
    Files,
}

impl Mode {
    pub fn other(self) -> Self {
        match self {
            Self::Worktrees => Self::Files,
            Self::Files => Self::Worktrees,
        }
    }
}

/// What a row opens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Worktree(PathBuf),
    /// A path in the current worktree.
    File(String),
}

/// Something to match against: what it opens, and the text the query matches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub target: Target,
    pub text: String,
}

/// What the switcher has to match against in its mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    Ready(Vec<Candidate>),
    /// abcd is still listing the worktree's files.
    Loading,
    Failed(String),
    /// Files, with no worktree open to list.
    NoWorktree,
}

/// A candidate that matches the query, and which characters of its text matched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub candidate: usize,
    pub matched: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct Switcher {
    pub mode: Mode,
    pub input: Input,
    pub source: Source,
    /// The matches, best first; with no query, every candidate in order.
    pub hits: Vec<Hit>,
    pub selected: usize,
    /// The first result on screen.
    pub offset: usize,
    /// Rows of results on screen.
    pub rows: usize,
    matcher: Matcher,
}

impl Switcher {
    pub fn new(mode: Mode, source: Source) -> Self {
        let mut switcher = Self {
            mode,
            input: Input::default(),
            source,
            hits: Vec::new(),
            selected: 0,
            offset: 0,
            rows: ROWS,
            matcher: Matcher::new(Config::DEFAULT),
        };
        switcher.refresh();
        switcher
    }

    pub fn candidates(&self) -> &[Candidate] {
        match &self.source {
            Source::Ready(candidates) => candidates,
            _ => &[],
        }
    }

    /// Changes mode, keeping the query.
    pub fn switch(&mut self, mode: Mode, source: Source) {
        self.mode = mode;
        self.source = source;
        self.refresh();
    }

    /// Matches the query again, from the top of the results.
    pub fn refresh(&mut self) {
        let mut config = Config::DEFAULT;
        if self.mode == Mode::Files {
            config.set_match_paths();
        }
        self.matcher = Matcher::new(config);
        let pattern = Pattern::parse(
            self.input.value(),
            CaseMatching::Smart,
            Normalization::Smart,
        );
        let mut buf = Vec::new();
        let mut scored: Vec<(u32, Hit)> = Vec::new();
        let Source::Ready(candidates) = &self.source else {
            self.hits.clear();
            return;
        };
        for (i, candidate) in candidates.iter().enumerate() {
            let mut matched = Vec::new();
            let text = Utf32Str::new(&candidate.text, &mut buf);
            if let Some(score) = pattern.indices(text, &mut self.matcher, &mut matched) {
                matched.sort_unstable();
                matched.dedup();
                scored.push((
                    score,
                    Hit {
                        candidate: i,
                        matched,
                    },
                ));
            }
        }
        // Best first; equal scores keep the candidates' order (recency, or path).
        scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.candidate.cmp(&b.1.candidate)));
        self.hits = scored.into_iter().map(|(_, hit)| hit).collect();
        self.selected = 0;
        self.offset = 0;
    }

    /// Selects result `i`, scrolling just enough to show it.
    pub fn select(&mut self, i: usize) {
        self.selected = i.min(self.hits.len().saturating_sub(1));
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if self.selected >= self.offset + self.rows {
            self.offset = self.selected + 1 - self.rows;
        }
    }

    pub fn down(&mut self) {
        self.select(self.selected + 1);
    }

    pub fn up(&mut self) {
        self.select(self.selected.saturating_sub(1));
    }

    /// What the selected row opens.
    pub fn target(&self) -> Option<&Target> {
        let hit = self.hits.get(self.selected)?;
        Some(&self.candidates()[hit.candidate].target)
    }
}

/// How long ago `then` was, as the switcher says it: "now", "5m ago", "2h ago",
/// "yesterday", "3d ago", "2w ago". Nothing for never.
pub fn ago(then: Option<u64>, now: u64) -> String {
    const MINUTE: u64 = 60;
    const HOUR: u64 = 60 * MINUTE;
    const DAY: u64 = 24 * HOUR;
    let Some(then) = then else {
        return String::new();
    };
    match now.saturating_sub(then) {
        s if s < MINUTE => "now".into(),
        s if s < HOUR => format!("{}m ago", s / MINUTE),
        s if s < DAY => format!("{}h ago", s / HOUR),
        s if s < 2 * DAY => "yesterday".into(),
        s if s < 14 * DAY => format!("{}d ago", s / DAY),
        s => format!("{}w ago", s / (7 * DAY)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(paths: &[&str]) -> Source {
        Source::Ready(
            paths
                .iter()
                .map(|p| Candidate {
                    target: Target::File(p.to_string()),
                    text: p.to_string(),
                })
                .collect(),
        )
    }

    fn typed(switcher: &mut Switcher, text: &str) {
        switcher.input = Input::new(text.into());
        switcher.refresh();
    }

    fn shown(switcher: &Switcher) -> Vec<&str> {
        switcher
            .hits
            .iter()
            .map(|h| switcher.candidates()[h.candidate].text.as_str())
            .collect()
    }

    #[test]
    fn no_query_shows_everything_in_order() {
        let s = Switcher::new(Mode::Files, files(&["b.rs", "a.rs"]));
        assert_eq!(shown(&s), ["b.rs", "a.rs"]);
    }

    #[test]
    fn a_query_keeps_the_matches_best_first_with_what_matched() {
        let mut s = Switcher::new(
            Mode::Files,
            files(&[
                "docs/gst.md",
                "src/verify.ts",
                "apps/verify-gst.tsx",
                "lib/server.rs",
            ]),
        );
        typed(&mut s, "verif");
        assert_eq!(shown(&s), ["src/verify.ts", "apps/verify-gst.tsx"]);
        assert_eq!(s.hits[0].matched, [4, 5, 6, 7, 8]);
        typed(&mut s, "zoho");
        assert!(s.hits.is_empty());
        assert_eq!(s.target(), None);
    }

    #[test]
    fn moving_scrolls_the_results_just_enough() {
        let paths: Vec<String> = (0..25).map(|i| format!("f{i:02}.rs")).collect();
        let paths: Vec<&str> = paths.iter().map(String::as_str).collect();
        let mut s = Switcher::new(Mode::Files, files(&paths));
        for _ in 0..12 {
            s.down();
        }
        assert_eq!((s.selected, s.offset), (12, 3));
        s.select(1);
        assert_eq!((s.selected, s.offset), (1, 1));
        s.select(99);
        assert_eq!(s.target(), Some(&Target::File("f24.rs".into())));
    }

    #[test]
    fn switching_mode_keeps_the_query() {
        let mut s = Switcher::new(Mode::Files, files(&["gst.md"]));
        typed(&mut s, "gst");
        let worktrees = Source::Ready(vec![Candidate {
            target: Target::Worktree("~/code/gst-check".into()),
            text: "privue-platform gst-check feat/gst-check".into(),
        }]);
        s.switch(Mode::Worktrees, worktrees);
        assert_eq!((s.input.value(), s.hits.len()), ("gst", 1));
    }

    #[test]
    fn times_read_as_how_long_ago() {
        let now: u64 = 10_000_000;
        let cases = [
            (Some(now - 5), "now"),
            (Some(now - 300), "5m ago"),
            (Some(now - 2 * 3600), "2h ago"),
            (Some(now - 30 * 3600), "yesterday"),
            (Some(now - 3 * 86400), "3d ago"),
            (Some(now - 15 * 86400), "2w ago"),
            (None, ""),
        ];
        for (then, said) in cases {
            assert_eq!(ago(then, now), said);
        }
    }
}
