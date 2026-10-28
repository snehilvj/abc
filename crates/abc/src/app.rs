//! The client's state and how keys change it. Drawing lives in `ui`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use abc_core::model::{Repo, Worktree};
use crokey::KeyCombination;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::explorer::{Listing, NodePath};
use crate::highlight::{self, Lang};
use crate::keys::{self, Action, Context, Feed, Keymap};
use crate::layout::Widths;
use crate::motion::{self, Find, Pos};
use crate::viewer::{Document, Place, Viewer};

/// Work the app asks for: abcd reads files; highlighting runs off the UI thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cmd {
    ReadFile {
        worktree: PathBuf,
        path: String,
    },
    Highlight {
        worktree: PathBuf,
        path: String,
        source: String,
        lang: Lang,
    },
}

/// Answers to commands.
#[derive(Debug, Clone)]
pub enum Msg {
    FileRead {
        worktree: PathBuf,
        path: String,
        contents: Result<abc_core::model::FileContents, String>,
    },
    Highlighted {
        worktree: PathBuf,
        path: String,
        spans: Option<Vec<Vec<highlight::Span>>>,
    },
}

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
    /// Each worktree's open file in the viewer.
    pub viewers: HashMap<PathBuf, Viewer>,
    /// Commands waiting to be carried out.
    pub outbox: Vec<Cmd>,
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
    /// A count typed before a viewer motion: the 12 of `12j`.
    pub count: Option<usize>,
    /// `f`, `F`, `t` or `T`, waiting for the character to find.
    pub finding: Option<Action>,
    /// The last character find, for `;` and `,`.
    pub last_find: Option<Find>,
    pub quit: bool,
}

/// The largest count a motion takes.
const MAX_COUNT: usize = 999_999;

impl App {
    pub fn new(profile: impl Into<String>) -> Self {
        Self {
            profile: profile.into(),
            repos: Vec::new(),
            listings: HashMap::new(),
            viewers: HashMap::new(),
            outbox: Vec::new(),
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
            count: None,
            finding: None,
            last_find: None,
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
        if let Some(action) = self.finding.take() {
            let count = self.count.take();
            if let Some(target) = typed_char(event) {
                self.find_char(action, target, count);
            }
            return;
        }
        let context = self.context();
        // Counts are Vim's grammar rather than bindings: a digit starts one,
        // except a leading 0, which is a motion of its own.
        if context == Context::Viewer && self.pending.is_empty() {
            match (typed_char(event), self.count) {
                (Some(c @ '0'..='9'), count) if c != '0' || count.is_some() => {
                    let digit = c as usize - '0' as usize;
                    let count = count.unwrap_or(0).saturating_mul(10) + digit;
                    self.count = Some(count.min(MAX_COUNT));
                    return;
                }
                (None, Some(_)) if event.code == KeyCode::Esc => {
                    self.count = None;
                    return;
                }
                _ => {}
            }
        }
        match self
            .keymap
            .feed(context, &mut self.pending, keys::from_event(event))
        {
            Feed::Action(action) => {
                let count = self.count.take();
                self.act(action, count)
            }
            Feed::Unbound(keys) => {
                self.count = None;
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
            Feed::Ignored => self.count = None,
            Feed::Pending(_) => {}
        }
    }

    fn act(&mut self, action: Action, count: Option<usize>) {
        if self.context() == Context::Viewer && self.viewer_action(action, count) {
            return;
        }
        match action {
            Action::Quit => self.quit = true,
            // With no text to move through, h and l still move between panes.
            Action::FocusLeft | Action::Left => self.move_focus(-1),
            Action::FocusRight | Action::Right => self.move_focus(1),
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
            Action::Open => {
                if let Some(path) = explorer.activate() {
                    self.open_file(path);
                }
            }
            _ => {}
        }
    }

    pub fn current_viewer(&self) -> Option<&Viewer> {
        self.current.as_ref().and_then(|p| self.viewers.get(p))
    }

    /// Moves through the open file, Vim style. False if there's no text to
    /// move through, or `action` isn't the viewer's.
    fn viewer_action(&mut self, action: Action, count: Option<usize>) -> bool {
        use Action::*;
        let Some(viewer) = self.current.as_ref().and_then(|p| self.viewers.get_mut(p)) else {
            return false;
        };
        if !viewer.has_text() {
            return false;
        }
        let n = count.unwrap_or(1);
        let at = viewer.cursor;
        let page = viewer.page();
        // Lines by number: `12G`, `H`, `50%`. The cursor lands on the first non-blank.
        let to_line = |viewer: &mut Viewer, line: usize| {
            viewer.jump_by(1, |l, _| {
                motion::first_non_blank(l, line.min(l.len().saturating_sub(1)))
            })
        };
        match action {
            Left => viewer.go(Pos::new(at.line, at.col.saturating_sub(n))),
            Right => viewer.go(Pos::new(at.line, at.col.saturating_add(n))),
            Down => viewer.go_lines(n as isize),
            Up => viewer.go_lines(-(n as isize)),
            WordForward => viewer.go_by(n, |l, p| motion::word_forward(l, p, false)),
            WordBackward => viewer.go_by(n, |l, p| motion::word_backward(l, p, false)),
            WordEnd => viewer.go_by(n, |l, p| motion::word_end(l, p, false)),
            BigWordForward => viewer.go_by(n, |l, p| motion::word_forward(l, p, true)),
            BigWordBackward => viewer.go_by(n, |l, p| motion::word_backward(l, p, true)),
            BigWordEnd => viewer.go_by(n, |l, p| motion::word_end(l, p, true)),
            LineStart => viewer.go(Pos::new(at.line, 0)),
            FirstNonBlank => viewer.go_by(1, |l, p| motion::first_non_blank(l, p.line)),
            LineEnd => {
                viewer.go_lines(n as isize - 1);
                viewer.go_line_end();
            }
            First => to_line(viewer, count.map_or(0, |n| n - 1)),
            Last => to_line(viewer, count.map_or(usize::MAX, |n| n - 1)),
            ScreenTop | ScreenMiddle | ScreenBottom => {
                let (top, bottom) = viewer.screen();
                let line = match action {
                    ScreenTop => (top + n - 1).min(bottom),
                    ScreenBottom => bottom.saturating_sub(n - 1).max(top),
                    _ => top + (bottom - top) / 2,
                };
                to_line(viewer, line);
            }
            HalfPageDown => viewer.scroll_with_cursor(page / 2),
            HalfPageUp => viewer.scroll_with_cursor(-page / 2),
            // A page, keeping two lines of the last one in view.
            PageDown => viewer.scroll_with_cursor((page - 2).max(1)),
            PageUp => viewer.scroll_with_cursor(-(page - 2).max(1)),
            ScrollDown => viewer.scroll_by(n as isize),
            ScrollUp => viewer.scroll_by(-(n as isize)),
            CursorLineTop => viewer.place(Place::Top),
            CursorLineMiddle => viewer.place(Place::Middle),
            CursorLineBottom => viewer.place(Place::Bottom),
            ParagraphForward => viewer.jump_by(n, motion::paragraph_forward),
            ParagraphBackward => viewer.jump_by(n, motion::paragraph_backward),
            MatchingBracket => match count {
                // `50%` goes halfway through the file.
                Some(percent) => {
                    let line = (percent.min(100) * viewer.lines().len()).div_ceil(100);
                    to_line(viewer, line.saturating_sub(1));
                }
                None => viewer.jump_by(1, |l, p| motion::matching_bracket(l, p).unwrap_or(p)),
            },
            FindForward | FindBackward | TillForward | TillBackward => {
                self.finding = Some(action);
                self.count = count;
            }
            RepeatFind | RepeatFindBack => {
                if let Some(find) = self.last_find {
                    let find = if action == RepeatFind {
                        find
                    } else {
                        find.reversed()
                    };
                    viewer.go_by(1, |l, p| find.apply(l, p, n, true).unwrap_or(p));
                }
            }
            JumpBack => viewer.back(),
            _ => return false,
        }
        true
    }

    /// Finishes an `f`, `F`, `t` or `T` with the character typed after it.
    fn find_char(&mut self, action: Action, target: char, count: Option<usize>) {
        let find = Find {
            target,
            forward: matches!(action, Action::FindForward | Action::TillForward),
            till: matches!(action, Action::TillForward | Action::TillBackward),
        };
        self.last_find = Some(find);
        if let Some(viewer) = self.current.as_ref().and_then(|p| self.viewers.get_mut(p)) {
            let n = count.unwrap_or(1);
            viewer.go_by(1, |l, p| find.apply(l, p, n, false).unwrap_or(p));
        }
    }

    /// Opens `path` in the current worktree's viewer and asks abcd for it.
    fn open_file(&mut self, path: NodePath) {
        let Some(worktree) = self.current.clone() else {
            return;
        };
        let file = path.join("/");
        self.viewers
            .insert(worktree.clone(), Viewer::new(path, Document::Loading));
        self.outbox.push(Cmd::ReadFile {
            worktree,
            path: file,
        });
    }

    /// Takes in an answer to a command. Answers for a file no longer open are dropped.
    pub fn apply(&mut self, msg: Msg) {
        match msg {
            Msg::FileRead {
                worktree,
                path,
                contents,
            } => {
                let Some(viewer) = self
                    .viewers
                    .get_mut(&worktree)
                    .filter(|v| v.path.join("/") == path)
                else {
                    return;
                };
                viewer.document = match contents {
                    Ok(contents) => Document::from_contents(contents, viewer.name()),
                    Err(why) => Document::Failed(why),
                };
                if let Document::Text(text) = &viewer.document
                    && text.lang != Lang::Plain
                {
                    let (source, lang) = (text.source(), text.lang);
                    self.outbox.push(Cmd::Highlight {
                        worktree,
                        path,
                        source,
                        lang,
                    });
                }
            }
            Msg::Highlighted {
                worktree,
                path,
                spans,
            } => {
                if let Some(viewer) = self
                    .viewers
                    .get_mut(&worktree)
                    .filter(|v| v.path.join("/") == path)
                    && let Document::Text(text) = &mut viewer.document
                {
                    text.spans = spans;
                }
            }
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

/// The character a key types, if it types one: not with ctrl or alt held.
fn typed_char(event: KeyEvent) -> Option<char> {
    match event.code {
        KeyCode::Char(c)
            if !event
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
        {
            Some(c)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake;
    use crate::viewer::Document;
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
            let (k, modifiers) = match k.strip_prefix("ctrl-") {
                Some(k) => (k, KeyModifiers::CONTROL),
                None => (k, KeyModifiers::NONE),
            };
            let code = match k {
                "enter" => KeyCode::Enter,
                "tab" => KeyCode::Tab,
                "space" => KeyCode::Char(' '),
                _ => KeyCode::Char(k.chars().next().unwrap()),
            };
            app.handle_key(KeyEvent::new(code, modifiers));
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
    fn opening_a_file_reads_it_then_highlights_it() {
        let mut app = with_files();
        let worktree = fake::current();
        let path = "packages/integrations/gst/client.ts".to_string();
        press_drawn(&mut app, "k enter");
        assert_eq!(
            app.outbox,
            [Cmd::ReadFile {
                worktree: worktree.clone(),
                path: path.clone()
            }]
        );
        assert!(matches!(
            app.current_viewer().unwrap().document,
            Document::Loading
        ));
        app.outbox.clear();

        app.apply(Msg::FileRead {
            worktree: worktree.clone(),
            path: path.clone(),
            contents: fake::read(&path),
        });
        let Some(Cmd::Highlight { source, lang, .. }) = app.outbox.pop() else {
            panic!("no highlight asked for")
        };
        assert_eq!(lang, Lang::TypeScript);
        let Document::Text(text) = &app.current_viewer().unwrap().document else {
            panic!("not text")
        };
        assert!(
            text.spans.is_none(),
            "uncoloured until highlighting finishes"
        );

        let spans = highlight::highlight(&source, lang);
        app.apply(Msg::Highlighted {
            worktree,
            path,
            spans,
        });
        let Document::Text(text) = &app.current_viewer().unwrap().document else {
            panic!("not text")
        };
        assert!(text.spans.is_some());
    }

    #[test]
    fn an_answer_for_a_file_no_longer_open_is_dropped() {
        let mut app = with_files();
        // Open client.ts, then verify.ts before client.ts arrives.
        press_drawn(&mut app, "k enter j enter");
        let worktree = fake::current();
        let stale = "packages/integrations/gst/client.ts".to_string();
        app.apply(Msg::FileRead {
            worktree,
            path: stale.clone(),
            contents: fake::read(&stale),
        });
        let viewer = app.current_viewer().unwrap();
        assert_ne!(viewer.path.join("/"), stale);
        assert!(matches!(viewer.document, Document::Loading));
    }

    #[test]
    fn viewer_keys_scroll_within_the_file() {
        let mut app = with_files();
        app.viewers
            .insert(fake::current(), fake::viewer("generated/rows.rs"));
        app.focus = Focus::Viewer;
        let scroll = |app: &App| app.current_viewer().unwrap().scroll;
        press_drawn(&mut app, "G");
        let end = scroll(&app);
        assert_eq!(end, 10_000 - app.current_viewer().unwrap().height.get());
        press_drawn(&mut app, "j");
        assert_eq!(scroll(&app), end, "no scrolling past the last line");
        press_drawn(&mut app, "g g ctrl-d");
        assert_eq!(scroll(&app), app.current_viewer().unwrap().height.get() / 2);
        press_drawn(&mut app, "ctrl-u k");
        assert_eq!(scroll(&app), 0);
    }

    /// The usual worktree with `path` open in the focused viewer.
    fn viewing(path: &str) -> App {
        let mut app = with_files();
        app.viewers.insert(fake::current(), fake::viewer(path));
        app.focus = Focus::Viewer;
        app
    }

    fn cursor(app: &App) -> (usize, usize) {
        let c = app.current_viewer().unwrap().cursor;
        (c.line, c.col)
    }

    #[test]
    fn counts_repeat_motions_and_pick_lines() {
        let mut app = viewing("generated/rows.rs");
        press_drawn(&mut app, "5 j");
        assert_eq!(cursor(&app).0, 5);
        press_drawn(&mut app, "1 2 G");
        assert_eq!(cursor(&app).0, 11);
        press_drawn(&mut app, "5 0 %");
        assert_eq!(cursor(&app).0, 4_999, "halfway through 10,000 lines");
        press_drawn(&mut app, "ctrl-o");
        assert_eq!(cursor(&app).0, 11);
        press_drawn(&mut app, "0");
        assert_eq!(cursor(&app).1, 0, "a 0 with no count before it is a motion");
        assert_eq!(app.count, None);
    }

    #[test]
    fn h_and_l_move_the_cursor_through_text_and_esc_goes_back() {
        let mut app = viewing("packages/integrations/gst/client.ts");
        press_drawn(&mut app, "l l l h");
        assert_eq!((cursor(&app), app.focus), ((0, 2), Focus::Viewer));
        app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(app.focus, Focus::Explorer);
    }

    #[test]
    fn f_finds_a_character_and_semicolon_and_comma_repeat_it() {
        let mut app = viewing("packages/integrations/gst/client.ts");
        press_drawn(&mut app, "f ;");
        assert_eq!(cursor(&app), (0, 36));
        press_drawn(&mut app, ";");
        assert_eq!(cursor(&app), (0, 55));
        press_drawn(&mut app, ",");
        assert_eq!(cursor(&app), (0, 36));
        press_drawn(&mut app, "0 2 t ;");
        assert_eq!(cursor(&app), (0, 54), "a count finds the second");
    }

    #[test]
    fn a_count_waits_with_its_motion_and_esc_drops_it() {
        let mut app = viewing("generated/rows.rs");
        press_drawn(&mut app, "3 f");
        assert_eq!(
            (app.count, app.finding),
            (Some(3), Some(Action::FindForward))
        );
        app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!((app.count, app.finding), (None, None));
        assert_eq!(app.focus, Focus::Viewer, "esc only cancelled the find");
        press_drawn(&mut app, "4");
        app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!((app.count, app.focus), (None, Focus::Viewer));
    }

    #[test]
    fn digits_count_only_in_the_viewer() {
        let mut app = with_files();
        press_drawn(&mut app, "5");
        assert_eq!(app.count, None);
    }

    #[test]
    fn q_quits() {
        let mut app = app();
        press(&mut app, "q");
        assert!(app.quit);
    }
}
