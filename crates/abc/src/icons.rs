//! Glyphs for repos, branches, pull requests and checks. Nerd Font by default;
//! `icons = "unicode"` swaps in symbols that render in any font.

/// The `icons` setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, clap::ValueEnum)]
pub enum IconChoice {
    #[default]
    Nerd,
    Unicode,
}

/// An empty string means "no icon": the text stands alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Icons {
    pub nerd: bool,
    pub repo: &'static str,
    pub home: &'static str,
    pub branch: &'static str,
    pub commit: &'static str,
    pub pr_open: &'static str,
    pub pr_draft: &'static str,
    pub pr_merged: &'static str,
    pub pr_closed: &'static str,
    pub checks_passing: &'static str,
    pub checks_failing: &'static str,
    pub checks_pending: &'static str,
    pub changed: &'static str,
    pub ahead: &'static str,
    pub behind: &'static str,
    pub lock: &'static str,
    pub alert: &'static str,
}

impl Icons {
    pub const NERD: Icons = Icons {
        nerd: true,
        repo: "\u{f401}",
        home: "\u{f46d}",
        branch: "\u{f418}",
        commit: "\u{f417}",
        pr_open: "\u{f407}",
        pr_draft: "\u{f4dd}",
        pr_merged: "\u{f419}",
        pr_closed: "\u{f4dc}",
        checks_passing: "\u{f42e}",
        checks_failing: "\u{f467}",
        checks_pending: "\u{f43a}",
        changed: "\u{f444}",
        ahead: "\u{f431}",
        behind: "\u{f433}",
        lock: "\u{f023}",
        alert: "\u{f421}",
    };

    pub const UNICODE: Icons = Icons {
        nerd: false,
        repo: "",
        home: "",
        branch: "",
        commit: "@",
        pr_open: "",
        pr_draft: "",
        pr_merged: "",
        pr_closed: "",
        checks_passing: "✓",
        checks_failing: "✗",
        checks_pending: "…",
        changed: "●",
        ahead: "⇡",
        behind: "⇣",
        lock: "locked",
        alert: "",
    };

    pub fn new(choice: IconChoice) -> Self {
        match choice {
            IconChoice::Nerd => Self::NERD,
            IconChoice::Unicode => Self::UNICODE,
        }
    }
}
