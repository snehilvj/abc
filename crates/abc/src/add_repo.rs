//! Adding a repository: where it comes from (a folder on this machine, or a
//! git URL to clone), its name, and what abcd made of them. abcd checks,
//! clones and adds; the client never reads the disk or runs git. Drawing is in
//! `ui::add_repo`.

use std::time::{Duration, Instant};

use abc_core::model::RepoCheck;
use tui_input::Input;

/// How long typing pauses before abcd checks, by default (`add_repo.check_delay_ms`).
pub const DELAY: Duration = Duration::from_millis(150);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Path,
    Name,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Check {
    /// Not checked yet.
    Idle,
    Checking,
    /// abcd's answer, or why it couldn't give one.
    Checked(Result<RepoCheck, String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stage {
    Editing,
    Adding,
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct AddRepo {
    /// A folder, or a git URL; typed or pasted.
    pub path: Input,
    /// The name typed; empty means the default.
    pub name: Input,
    pub field: Field,
    pub check: Check,
    /// When the path or name last changed, while its check has yet to start.
    pub typed_at: Option<Instant>,
    pub stage: Stage,
    /// How long typing pauses before abcd checks.
    pub delay: Duration,
}

impl Default for AddRepo {
    fn default() -> Self {
        Self {
            path: Input::default(),
            name: Input::default(),
            field: Field::Path,
            check: Check::Idle,
            typed_at: None,
            stage: Stage::Editing,
            delay: DELAY,
        }
    }
}

impl AddRepo {
    /// The path as given, without the spaces a paste brings along.
    pub fn source(&self) -> &str {
        self.path.value().trim()
    }

    /// Whether the path is a git URL to clone rather than a folder here.
    pub fn is_url(&self) -> bool {
        is_url(self.source())
    }

    /// The name unless one is typed: the folder's, or the repository's in the URL.
    pub fn default_name(&self) -> &str {
        default_name(self.source())
    }

    pub fn name(&self) -> &str {
        match self.name.value().trim() {
            "" => self.default_name(),
            typed => typed,
        }
    }

    /// The path changed at `at`: abcd checks it once typing pauses.
    pub fn path_edited(&mut self, at: Instant) {
        self.check = Check::Idle;
        self.typed_at = Some(at);
        self.stage = Stage::Editing;
    }

    /// The name changed at `at`. For a URL it decides where the clone goes,
    /// so abcd checks again; a folder's check stands.
    pub fn name_edited(&mut self, at: Instant) {
        self.stage = Stage::Editing;
        if self.is_url() {
            self.path_edited(at);
        }
    }

    pub fn due(&self, now: Instant) -> bool {
        self.typed_at
            .is_some_and(|typed| now.saturating_duration_since(typed) >= self.delay)
    }

    /// Starts checking: the path and name to check, or nothing for an empty path.
    pub fn start_check(&mut self) -> Option<(String, String)> {
        self.typed_at = None;
        if self.source().is_empty() {
            self.check = Check::Idle;
            return None;
        }
        self.check = Check::Checking;
        Some((self.source().to_string(), self.name().to_string()))
    }

    /// abcd's answer for `path` named `name`; dropped if either has changed
    /// since in a way that matters.
    pub fn checked(&mut self, path: &str, name: &str, check: Result<RepoCheck, String>) {
        if path == self.source() && (!self.is_url() || name == self.name()) {
            self.check = Check::Checked(check);
        }
    }

    /// Where a clone goes, once abcd has said.
    pub fn clone_into(&self) -> Option<&str> {
        match &self.check {
            Check::Checked(Ok(RepoCheck::Remote { into })) => Some(into),
            _ => None,
        }
    }

    /// Whether `enter` adds: abcd found a repository to add or clone, its
    /// name is free, and nothing is being added already.
    pub fn ready(&self, taken: impl Fn(&str) -> bool) -> bool {
        matches!(
            self.check,
            Check::Checked(Ok(RepoCheck::Ready { .. } | RepoCheck::Remote { .. }))
        ) && !self.name().is_empty()
            && !taken(self.name())
            && self.stage != Stage::Adding
    }
}

pub use abc_core::source::{default_name, is_url};

#[cfg(test)]
mod tests {
    use super::*;

    fn with(path: &str, name: &str) -> AddRepo {
        AddRepo {
            path: Input::new(path.into()),
            name: Input::new(name.into()),
            ..AddRepo::default()
        }
    }

    #[test]
    fn the_name_defaults_to_the_folders_or_the_repositorys() {
        assert_eq!(default_name("~/code/privue-agents/"), "privue-agents");
        assert_eq!(
            default_name("https://github.com/privue/privue-agents"),
            "privue-agents"
        );
        assert_eq!(default_name("git@github.com:privue/agents.git"), "agents");
        assert_eq!(
            default_name("  ~/code/notes  "),
            "notes",
            "pasted with spaces"
        );
        assert_eq!(default_name("~/"), "");
        assert_eq!(with("~/code/notes", "jottings").name(), "jottings");
    }

    #[test]
    fn urls_are_told_from_folders() {
        assert!(is_url("https://github.com/privue/ecourts"));
        assert!(is_url("git@github.com:privue/ecourts.git"));
        assert!(!is_url("~/code/ecourts"));
        assert!(!is_url("/Users/me/code/ecourts"));
    }

    #[test]
    fn a_check_starts_once_typing_pauses() {
        let mut add = with("~/code/privue-agents", "");
        let typed = Instant::now();
        add.path_edited(typed);
        assert!(!add.due(typed));
        assert!(add.due(typed + DELAY));
        assert_eq!(
            add.start_check(),
            Some(("~/code/privue-agents".into(), "privue-agents".into()))
        );
        assert_eq!(add.check, Check::Checking);
        assert_eq!(AddRepo::default().start_check(), None, "nothing to check");
    }

    #[test]
    fn a_name_changes_the_check_only_for_a_url() {
        let mut folder = with("~/code/notes", "");
        folder.checked(
            "~/code/notes",
            "notes",
            Ok(RepoCheck::Ready { worktrees: 1 }),
        );
        folder.name_edited(Instant::now());
        assert!(folder.typed_at.is_none(), "a folder's check stands");
        let mut url = with("https://github.com/privue/notes", "");
        url.name_edited(Instant::now());
        assert!(url.typed_at.is_some(), "the name moves the clone");
    }

    #[test]
    fn answers_for_what_has_since_changed_are_dropped() {
        let mut add = with("~/code/x", "");
        add.checked("~/code/", "", Ok(RepoCheck::NotGit));
        assert_eq!(add.check, Check::Idle);
        let mut url = with("https://github.com/privue/agents", "a");
        url.checked(
            "https://github.com/privue/agents",
            "agents",
            Ok(RepoCheck::Remote {
                into: "~/code/agents".into(),
            }),
        );
        assert_eq!(url.check, Check::Idle, "checked under the old name");
    }

    #[test]
    fn enter_adds_or_clones_only_under_a_free_name() {
        let taken = |name: &str| name == "ecourts";
        let mut add = with("~/code/privue-agents", "");
        assert!(!add.ready(taken), "not checked yet");
        add.checked(
            "~/code/privue-agents",
            "privue-agents",
            Ok(RepoCheck::Ready { worktrees: 3 }),
        );
        assert!(add.ready(taken));
        let mut url = with("https://github.com/privue/agents", "");
        let into = "~/code/agents".to_string();
        url.checked(
            "https://github.com/privue/agents",
            "agents",
            Ok(RepoCheck::Remote { into }),
        );
        assert!(url.ready(taken));
        let mut clash = with("~/work/ecourts", "");
        clash.checked(
            "~/work/ecourts",
            "ecourts",
            Ok(RepoCheck::Ready { worktrees: 1 }),
        );
        assert!(!clash.ready(taken));
        add.checked(
            "~/code/privue-agents",
            "privue-agents",
            Ok(RepoCheck::NotGit),
        );
        assert!(!add.ready(taken));
    }
}
