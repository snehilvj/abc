//! The client's state and how keys change it. Drawing lives in `ui`.

use std::path::{Path, PathBuf};

use abc_core::model::{Repo, Worktree};
use crokey::KeyCombination;
use ratatui::crossterm::event::{KeyEvent, KeyEventKind};

use crate::keys::{self, Action, Context, Feed, Keymap};
use crate::layout::Widths;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Sidebar,
    Explorer,
    Viewer,
}

/// The `sidebar.style` setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, clap::ValueEnum)]
pub enum SidebarStyle {
    /// Two lines per worktree: folder and PR, then branch and git status.
    #[default]
    Detailed,
    /// One line per worktree.
    Compact,
}

/// How things stand with abcd.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Connection {
    /// Starting or connecting to abcd; nothing to show yet.
    Starting,
    /// abcd didn't answer after being started.
    StartFailed,
    Connected,
    /// The connection dropped; the last data stays on screen while abc reconnects.
    Lost,
    /// The running abcd speaks a different protocol.
    Incompatible {
        ours: u32,
        theirs: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
    Info,
    Ok,
    Warning,
    Error,
}

/// A message on the left of the status line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    pub kind: StatusKind,
    pub text: String,
}

impl Status {
    pub fn new(kind: StatusKind, text: impl Into<String>) -> Self {
        Self {
            kind,
            text: text.into(),
        }
    }
}

/// A row in the sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entry {
    Repo(usize),
    Worktree(usize, usize),
}

#[derive(Debug, Clone)]
pub struct App {
    pub profile: String,
    pub repos: Vec<Repo>,
    pub current: Option<PathBuf>,
    pub last: Option<PathBuf>,
    pub focus: Focus,
    /// Index into `entries()`.
    pub sidebar_cursor: usize,
    pub sidebar_style: SidebarStyle,
    /// Blank rows between repos in the sidebar.
    pub repo_gap: u16,
    /// Blank rows between worktrees in the detailed sidebar.
    pub worktree_gap: u16,
    pub widths: Widths,
    pub connection: Connection,
    pub status: Option<Status>,
    /// This abc is older than the running abcd but can still talk to it.
    pub stale_client: bool,
    pub keymap: Keymap,
    /// Keys of an unfinished sequence, such as `space r`.
    pub pending: Vec<KeyCombination>,
    pub quit: bool,
}

impl App {
    pub fn new(profile: impl Into<String>) -> Self {
        Self {
            profile: profile.into(),
            repos: Vec::new(),
            current: None,
            last: None,
            focus: Focus::Sidebar,
            sidebar_cursor: 0,
            sidebar_style: SidebarStyle::default(),
            repo_gap: 2,
            worktree_gap: 1,
            widths: Widths::default(),
            connection: Connection::Starting,
            status: None,
            stale_client: false,
            keymap: Keymap::defaults(),
            pending: Vec::new(),
            quit: false,
        }
    }

    pub fn entries(&self) -> Vec<Entry> {
        let mut entries = Vec::new();
        for (r, repo) in self.repos.iter().enumerate() {
            entries.push(Entry::Repo(r));
            entries.extend((0..repo.worktrees().len()).map(|w| Entry::Worktree(r, w)));
        }
        entries
    }

    pub fn entry_at_cursor(&self) -> Option<Entry> {
        self.entries().get(self.sidebar_cursor).copied()
    }

    pub fn find(&self, path: &Path) -> Option<(&Repo, &Worktree)> {
        self.repos.iter().find_map(|r| {
            r.worktrees()
                .iter()
                .find(|w| w.path == path)
                .map(|w| (r, w))
        })
    }

    pub fn current_worktree(&self) -> Option<(&Repo, &Worktree)> {
        self.current.as_deref().and_then(|p| self.find(p))
    }

    /// The pane whose keys apply now.
    pub fn context(&self) -> Context {
        match self.focus {
            _ if self.current_worktree().is_none() => Context::Sidebar,
            Focus::Sidebar => Context::Sidebar,
            Focus::Explorer => Context::Explorer,
            Focus::Viewer => Context::Viewer,
        }
    }

    pub fn handle_key(&mut self, event: KeyEvent) {
        if event.kind != KeyEventKind::Press {
            return;
        }
        let context = self.context();
        match self
            .keymap
            .feed(context, &mut self.pending, keys::from_event(event))
        {
            Feed::Action(action) => self.act(action),
            Feed::Unbound(keys) => {
                let help = self
                    .keymap
                    .keys_for(Context::Global, Action::Help)
                    .map(keys::short)
                    .unwrap_or_default();
                self.status = Some(Status::new(
                    StatusKind::Error,
                    format!("✗ {keys} isn't bound · {help} lists the keys"),
                ));
            }
            Feed::Pending(_) | Feed::Ignored => {}
        }
    }

    fn act(&mut self, action: Action) {
        match action {
            Action::Quit => self.quit = true,
            Action::FocusLeft => self.move_focus(-1),
            Action::FocusRight => self.move_focus(1),
            Action::LastWorktree => {
                if let Some(last) = self.last.clone() {
                    self.open(&last);
                }
            }
            Action::Down if self.focus == Focus::Sidebar || self.current.is_none() => {
                self.move_cursor(1)
            }
            Action::Up if self.focus == Focus::Sidebar || self.current.is_none() => {
                self.move_cursor(-1)
            }
            Action::First if self.context() == Context::Sidebar => self.sidebar_cursor = 0,
            Action::Last if self.context() == Context::Sidebar => {
                self.sidebar_cursor = self.entries().len().saturating_sub(1);
            }
            Action::Open if self.context() == Context::Sidebar => self.open_at_cursor(),
            // The switcher, dialogs, help, explorer and viewer arrive with their screens.
            _ => {}
        }
    }

    fn move_focus(&mut self, step: i32) {
        if self.current_worktree().is_none() {
            return;
        }
        let order = [Focus::Sidebar, Focus::Explorer, Focus::Viewer];
        let i = order.iter().position(|f| *f == self.focus).unwrap_or(0) as i32;
        self.focus = order[(i + step).clamp(0, 2) as usize];
    }

    fn move_cursor(&mut self, step: i32) {
        let len = self.entries().len() as i32;
        if len > 0 {
            self.sidebar_cursor = (self.sidebar_cursor as i32 + step).clamp(0, len - 1) as usize;
        }
    }

    fn open_at_cursor(&mut self) {
        let path = match self.entry_at_cursor() {
            Some(Entry::Worktree(r, w)) => {
                let w = &self.repos[r].worktrees()[w];
                (!w.missing).then(|| w.path.clone())
            }
            // A repo row opens its main checkout, or in a bare repo its first worktree.
            Some(Entry::Repo(r)) => {
                let worktrees = self.repos[r].worktrees();
                worktrees
                    .iter()
                    .find(|w| w.is_main)
                    .or_else(|| worktrees.iter().find(|w| !w.missing))
                    .map(|w| w.path.clone())
            }
            None => None,
        };
        if let Some(path) = path {
            self.open(&path);
        }
    }

    /// Makes `path` the current worktree and focuses the explorer.
    pub fn open(&mut self, path: &Path) {
        if self.current.as_deref() != Some(path) {
            self.last = self.current.replace(path.to_path_buf());
        }
        if let Some(i) = self.entries().iter().position(|e| match e {
            Entry::Worktree(r, w) => self.repos[*r].worktrees()[*w].path == path,
            Entry::Repo(_) => false,
        }) {
            self.sidebar_cursor = i;
        }
        self.focus = Focus::Explorer;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake;
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};

    fn app() -> App {
        let mut app = App::new("dev");
        app.repos = fake::repos();
        app.connection = Connection::Connected;
        app.open(&fake::current());
        app
    }

    fn press(app: &mut App, keys: &str) {
        for k in keys.split(' ') {
            let code = match k {
                "enter" => KeyCode::Enter,
                "tab" => KeyCode::Tab,
                "space" => KeyCode::Char(' '),
                _ => KeyCode::Char(k.chars().next().unwrap()),
            };
            app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
        }
    }

    #[test]
    fn opening_a_worktree_focuses_the_explorer_and_remembers_the_last_one() {
        let mut app = app();
        press(&mut app, "h j enter");
        assert_eq!(app.focus, Focus::Explorer);
        assert_eq!(
            app.current.as_deref(),
            Some(Path::new("~/code/wt/privue-platform/gst-rounding"))
        );
        assert_eq!(app.last, Some(fake::current()));
    }

    #[test]
    fn tab_flips_between_the_last_two_worktrees() {
        let mut app = app();
        press(&mut app, "h j enter tab");
        assert_eq!(app.current, Some(fake::current()));
        press(&mut app, "tab");
        assert_eq!(
            app.current.as_deref(),
            Some(Path::new("~/code/wt/privue-platform/gst-rounding"))
        );
    }

    #[test]
    fn enter_on_a_repo_row_opens_its_main_checkout() {
        let mut app = app();
        app.focus = Focus::Sidebar;
        app.sidebar_cursor = app
            .entries()
            .iter()
            .position(|e| *e == Entry::Repo(1))
            .unwrap();
        press(&mut app, "enter");
        assert_eq!(app.current.as_deref(), Some(Path::new("~/code/ecourts")));
    }

    #[test]
    fn focus_moves_between_panes_and_stops_at_the_edges() {
        let mut app = app();
        press(&mut app, "l l l");
        assert_eq!(app.focus, Focus::Viewer);
        press(&mut app, "h h h");
        assert_eq!(app.focus, Focus::Sidebar);
    }

    #[test]
    fn an_unfinished_leader_sequence_then_a_wrong_key_says_so() {
        let mut app = app();
        press(&mut app, "space x");
        assert_eq!(
            app.status.unwrap().text,
            "✗ space x isn't bound · ? lists the keys"
        );
    }

    #[test]
    fn q_quits() {
        let mut app = app();
        press(&mut app, "q");
        assert!(app.quit);
    }
}
