//! The client's state and how keys change it. Drawing lives in `ui`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use abc_core::model::{Repo, Worktree};
use crokey::KeyCombination;
use ratatui::crossterm::event::{KeyEvent, KeyEventKind};

use crate::explorer::Listing;
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
    /// Each worktree's files and explorer, kept while abc runs.
    pub listings: HashMap<PathBuf, Listing>,
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
            listings: HashMap::new(),
            current: None,
            last: None,
            focus: Focus::Sidebar,
            sidebar_cursor: 0,
            sidebar_style: SidebarStyle::default(),
            repo_gap: 1,
            worktree_gap: 0,
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
                    format!("{keys} isn't bound · {help} lists the keys"),
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
            Action::First if self.context() == Context::Sidebar => {
                if let Some(i) = self.selectable().next() {
                    self.sidebar_cursor = i;
                }
            }
            Action::Last if self.context() == Context::Sidebar => {
                if let Some(i) = self.selectable().last() {
                    self.sidebar_cursor = i;
                }
            }
            Action::Open if self.context() == Context::Sidebar => self.open_at_cursor(),
            Action::Down | Action::Up | Action::First | Action::Last | Action::Open
                if self.context() == Context::Explorer =>
            {
                self.explorer_action(action)
            }
            // The switcher, dialogs, help, explorer and viewer arrive with their screens.
            _ => {}
        }
    }

    /// The current worktree's listing.
    pub fn current_listing(&self) -> Option<&Listing> {
        self.current.as_ref().and_then(|p| self.listings.get(p))
    }

    fn explorer_action(&mut self, action: Action) {
        let Some(Listing::Ready(explorer)) =
            self.current.as_ref().and_then(|p| self.listings.get_mut(p))
        else {
            return;
        };
        match action {
            Action::Down => explorer.down(),
            Action::Up => explorer.up(),
            Action::First => explorer.first(),
            Action::Last => explorer.last(),
            Action::Open => explorer.activate(),
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

    /// Indexes of the sidebar rows the cursor can rest on: worktrees, and repos
    /// with nothing under them (a missing repo, say), so they can still be removed.
    fn selectable(&self) -> impl Iterator<Item = usize> + use<> {
        let entries = self.entries();
        let has_worktrees: Vec<bool> = self
            .repos
            .iter()
            .map(|r| !r.worktrees().is_empty())
            .collect();
        (0..entries.len()).filter(move |i| match entries[*i] {
            Entry::Worktree(..) => true,
            Entry::Repo(r) => !has_worktrees[r],
        })
    }

    fn move_cursor(&mut self, step: i32) {
        let cursor = self.sidebar_cursor;
        let next = if step > 0 {
            self.selectable().find(|i| *i > cursor)
        } else {
            self.selectable().filter(|i| *i < cursor).last()
        };
        if let Some(i) = next {
            self.sidebar_cursor = i;
        }
    }

    fn open_at_cursor(&mut self) {
        let path = match self.entry_at_cursor() {
            Some(Entry::Worktree(r, w)) => {
                let w = &self.repos[r].worktrees()[w];
                (!w.missing).then(|| w.path.clone())
            }
            // The cursor only rests on a repo row when it has no worktrees to open.
            Some(Entry::Repo(_)) | None => None,
        };
        if let Some(path) = path {
            self.open(&path);
        }
    }

    /// Makes `path` the current worktree and focuses the explorer.
    pub fn open(&mut self, path: &Path) {
        // abcd sends the file list; until it arrives the explorer says it's loading.
        self.listings
            .entry(path.to_path_buf())
            .or_insert(Listing::Loading);
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
    fn the_cursor_steps_over_repo_rows() {
        let mut app = app();
        app.focus = Focus::Sidebar;
        // From privue-platform's last worktree, down lands on ecourts' first, not on the ecourts row.
        app.sidebar_cursor = app
            .entries()
            .iter()
            .position(|e| *e == Entry::Worktree(0, 3))
            .unwrap();
        press(&mut app, "j");
        assert_eq!(app.entry_at_cursor(), Some(Entry::Worktree(1, 0)));
        press(&mut app, "k");
        assert_eq!(app.entry_at_cursor(), Some(Entry::Worktree(0, 3)));
        press(&mut app, "g g");
        assert_eq!(app.entry_at_cursor(), Some(Entry::Worktree(0, 0)));
    }

    #[test]
    fn a_repo_with_no_worktrees_can_still_be_selected() {
        let mut app = app();
        app.repos = fake::repos_with_variants();
        app.focus = Focus::Sidebar;
        app.handle_key(KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT));
        assert_eq!(app.entry_at_cursor(), Some(Entry::Repo(2)));
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
            "space x isn't bound · ? lists the keys"
        );
    }

    /// The usual worktree open, with every worktree's files loaded.
    fn with_files() -> App {
        let mut app = App::new("dev");
        app.repos = fake::repos();
        app.listings = fake::listings(&app.repos);
        app.connection = Connection::Connected;
        app.open(&fake::current());
        app
    }

    /// Presses keys the way the real loop does: drawing before each one, since
    /// the tree widget moves through the rows it last drew.
    fn press_drawn(app: &mut App, keys: &str) {
        for key in keys.split(' ') {
            let mut buf = ratatui::buffer::Buffer::empty(ratatui::layout::Rect::new(0, 0, 120, 40));
            let area = buf.area;
            crate::ui::draw_buffer(&mut buf, area, app, &crate::theme::Theme::dark());
            press(app, key);
        }
    }

    fn explorer(app: &App) -> &crate::explorer::Explorer {
        match app.current_listing() {
            Some(Listing::Ready(e)) => e,
            other => panic!("no explorer: {other:?}"),
        }
    }

    fn selected(app: &App) -> String {
        explorer(app).selected().join("/")
    }

    #[test]
    fn explorer_keys_move_through_the_tree_and_fold_folders() {
        let mut app = with_files();
        assert_eq!(app.focus, Focus::Explorer);
        assert_eq!(selected(&app), "packages/integrations/gst/verify.ts");
        press_drawn(&mut app, "k k");
        assert_eq!(selected(&app), "packages/integrations/gst");
        press_drawn(&mut app, "enter j");
        // gst is closed, so down skips its files.
        assert_eq!(selected(&app), "packages/scoring");
        press_drawn(&mut app, "enter j enter");
        assert_eq!(selected(&app), "packages/scoring/score.py");
        assert_eq!(
            explorer(&app).open.as_ref().map(|p| p.join("/")).as_deref(),
            Some("packages/scoring/score.py")
        );
    }

    #[test]
    fn gg_and_g_jump_to_the_ends_of_the_tree() {
        let mut app = with_files();
        press_drawn(&mut app, "g g");
        assert_eq!(selected(&app), ".github");
        press_drawn(&mut app, "G");
        assert_eq!(selected(&app), "tsconfig.json");
    }

    #[test]
    fn each_worktree_keeps_its_own_explorer() {
        let mut app = with_files();
        press_drawn(&mut app, "k");
        assert_eq!(selected(&app), "packages/integrations/gst/client.ts");
        press_drawn(&mut app, "h j enter");
        assert_eq!(
            app.current.as_deref(),
            Some(Path::new("~/code/wt/privue-platform/gst-rounding"))
        );
        press_drawn(&mut app, "tab");
        assert_eq!(selected(&app), "packages/integrations/gst/client.ts");
    }

    #[test]
    fn q_quits() {
        let mut app = app();
        press(&mut app, "q");
        assert!(app.quit);
    }
}
