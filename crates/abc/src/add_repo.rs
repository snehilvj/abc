//! Adding a repository: the path and name being typed, folder completions,
//! and what abcd made of the path. abcd lists folders and checks the path;
//! the client never reads the disk. Drawing is in `ui::add_repo`.

use std::time::{Duration, Instant};

use abc_core::model::RepoCheck;
use tui_input::Input;

/// Where the path starts (`add_repo.start_dir`).
pub const START_DIR: &str = "~/";
/// How long typing pauses before abcd checks the path (`add_repo.check_delay_ms`).
pub const DELAY: Duration = Duration::from_millis(150);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Path,
    Name,
}

/// Folders that complete the path, and which one is in it, once `tab` cycles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completions {
    pub dirs: Vec<String>,
    pub selected: Option<usize>,
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
    pub path: Input,
    /// The name typed; empty means the folder's name.
    pub name: Input,
    pub field: Field,
    pub completions: Option<Completions>,
    pub check: Check,
    /// When the path last changed, while its check has yet to start.
    pub typed_at: Option<Instant>,
    pub stage: Stage,
}

impl Default for AddRepo {
    fn default() -> Self {
        Self {
            path: Input::new(START_DIR.into()),
            name: Input::default(),
            field: Field::Path,
            completions: None,
            check: Check::Idle,
            typed_at: None,
            stage: Stage::Editing,
        }
    }
}

impl AddRepo {
    /// The path's last folder, which names the repository unless a name is typed.
    pub fn folder_name(&self) -> &str {
        let name = self
            .path
            .value()
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or("");
        if name == "~" { "" } else { name }
    }

    /// The name the repository will have.
    pub fn name(&self) -> &str {
        match self.name.value() {
            "" => self.folder_name(),
            typed => typed,
        }
    }

    /// The path changed at `at`: completions close, and abcd checks it once typing pauses.
    pub fn path_edited(&mut self, at: Instant) {
        self.completions = None;
        self.check = Check::Idle;
        self.typed_at = Some(at);
        self.stage = Stage::Editing;
    }

    pub fn due(&self, now: Instant) -> bool {
        self.typed_at
            .is_some_and(|typed| now.saturating_duration_since(typed) >= DELAY)
    }

    /// Starts checking the path as it is, and gives it.
    pub fn start_check(&mut self) -> String {
        self.typed_at = None;
        self.check = Check::Checking;
        self.path.value().to_string()
    }

    /// abcd's answer for `path`; dropped if the path has changed since.
    pub fn checked(&mut self, path: &str, check: Result<RepoCheck, String>) {
        if path == self.path.value() {
            self.check = Check::Checked(check);
        }
    }

    /// Folders that complete `path` arrived. One goes straight into the path;
    /// several are listed and the path takes what they share. True when the
    /// path changed.
    pub fn completed(&mut self, path: &str, dirs: Vec<String>) -> bool {
        if path != self.path.value() || dirs.is_empty() {
            return false;
        }
        let shared = shared_start(&dirs);
        if dirs.len() > 1 {
            self.completions = Some(Completions {
                dirs,
                selected: None,
            });
        }
        if shared == path {
            return false;
        }
        self.path = Input::new(shared);
        // The old path's check, or one waiting to start, no longer applies.
        self.check = Check::Idle;
        self.typed_at = None;
        true
    }

    /// `tab` again, or `shift-tab`: the next or previous folder into the path.
    pub fn cycle(&mut self, forward: bool) {
        let Some(c) = &mut self.completions else {
            return;
        };
        let n = c.dirs.len();
        let next = match (c.selected, forward) {
            (None, true) => 0,
            (None, false) => n - 1,
            (Some(i), true) => (i + 1) % n,
            (Some(i), false) => (i + n - 1) % n,
        };
        c.selected = Some(next);
        self.path = Input::new(c.dirs[next].clone());
    }

    /// Whether `enter` adds: abcd found a repository to add, the name is
    /// free, and nothing is being added already.
    pub fn ready(&self, taken: impl Fn(&str) -> bool) -> bool {
        matches!(self.check, Check::Checked(Ok(RepoCheck::Ready { .. })))
            && !self.name().is_empty()
            && !taken(self.name())
            && self.stage != Stage::Adding
    }
}

/// The longest start every one of `dirs` shares.
fn shared_start(dirs: &[String]) -> String {
    let first = &dirs[0];
    let mut len = first.len();
    for dir in &dirs[1..] {
        len = first
            .char_indices()
            .zip(dir.chars())
            .take_while(|((_, a), b)| a == b)
            .last()
            .map_or(0, |((i, a), _)| i + a.len_utf8())
            .min(len);
    }
    first[..len].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(path: &str) -> AddRepo {
        AddRepo {
            path: Input::new(path.into()),
            ..AddRepo::default()
        }
    }

    fn dirs(names: &[&str]) -> Vec<String> {
        names.iter().map(|d| d.to_string()).collect()
    }

    #[test]
    fn the_name_is_the_folders_until_one_is_typed() {
        let mut add = at("~/code/privue-agents/");
        assert_eq!(add.name(), "privue-agents");
        add.name = Input::new("agents".into());
        assert_eq!(add.name(), "agents");
        assert_eq!(AddRepo::default().name(), "", "~ isn't a name");
    }

    #[test]
    fn one_completion_goes_straight_into_the_path() {
        let mut add = at("~/Dow");
        assert!(add.completed("~/Dow", dirs(&["~/Downloads/"])));
        assert_eq!(
            (add.path.value(), add.completions.is_none()),
            ("~/Downloads/", true)
        );
    }

    #[test]
    fn several_share_their_start_then_tab_cycles_through_them() {
        let mut add = at("~/code/privue-a");
        let found = dirs(&["~/code/privue-agents/", "~/code/privue-agreements/"]);
        assert!(add.completed("~/code/privue-a", found));
        assert_eq!(add.path.value(), "~/code/privue-ag");
        add.cycle(true);
        assert_eq!(add.path.value(), "~/code/privue-agents/");
        add.cycle(true);
        add.cycle(true);
        assert_eq!(add.path.value(), "~/code/privue-agents/", "round again");
        add.cycle(false);
        assert_eq!(add.path.value(), "~/code/privue-agreements/");
    }

    #[test]
    fn completing_forgets_the_last_paths_check() {
        let mut add = at("~/code/privue-a");
        add.checked("~/code/privue-a", Ok(RepoCheck::NotFound));
        add.path_edited(Instant::now());
        let found = dirs(&["~/code/privue-agents/", "~/code/privue-agreements/"]);
        assert!(add.completed("~/code/privue-a", found));
        assert_eq!((&add.check, add.typed_at), (&Check::Idle, None));
    }

    #[test]
    fn answers_for_a_path_since_changed_are_dropped() {
        let mut add = at("~/code/x/");
        assert!(!add.completed("~/code/", dirs(&["~/code/a/"])));
        add.checked("~/code/", Ok(RepoCheck::NotGit));
        assert_eq!(add.check, Check::Idle);
    }

    #[test]
    fn enter_adds_only_a_repository_under_a_free_name() {
        let mut add = at("~/code/privue-agents/");
        let taken = |name: &str| name == "ecourts";
        assert!(!add.ready(taken), "not checked yet");
        add.checked(
            "~/code/privue-agents/",
            Ok(RepoCheck::Ready { worktrees: 3 }),
        );
        assert!(add.ready(taken));
        add.name = Input::new("ecourts".into());
        assert!(!add.ready(taken));
        add.name = Input::default();
        add.checked("~/code/privue-agents/", Ok(RepoCheck::NotGit));
        assert!(!add.ready(taken));
    }

    #[test]
    fn a_check_starts_once_typing_pauses() {
        let mut add = AddRepo::default();
        let typed = Instant::now();
        add.path_edited(typed);
        assert!(!add.due(typed));
        assert!(add.due(typed + DELAY));
        assert_eq!(add.start_check(), "~/");
        assert_eq!(add.check, Check::Checking);
    }
}
