//! The client's state and how keys change it. Drawing lives in `ui`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use abc_core::model::{FoundFile, Head, Repo, RepoCheck, Worktree};
use crokey::KeyCombination;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use tui_input::Input;
use tui_input::backend::crossterm::to_input_request;

use crate::add_repo::{AddRepo, Field, Stage};
use crate::explorer::{Listing, NodePath};
use crate::find;
use crate::highlight::{self, Lang};
use crate::keys::{self, Action, Context, Feed, Keymap};
use crate::layout::Widths;
use crate::motion::{self, Find, Pos};
use crate::search;
use crate::switcher::{Candidate, Mode, Source, Switcher, Target};
use crate::viewer::{Document, Place, Viewer, reference};

/// Work the app asks for: abcd reads files; highlighting runs off the UI
/// thread; the terminal puts text on the clipboard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cmd {
    Copy(String),
    /// Folders that complete `path`, for adding a repository.
    Complete {
        path: String,
    },
    /// What abcd makes of `path` as a repository to add.
    CheckRepo {
        path: String,
    },
    AddRepo {
        path: String,
        name: String,
    },
    /// Finds `query` in the worktree's files; answers come as `Found`, then `SearchDone`.
    Search {
        worktree: PathBuf,
        id: u64,
        query: String,
        max_lines: usize,
    },
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
    /// A copy didn't reach the clipboard, and why.
    CopyFailed(String),
    /// Some of what a search found, a file or more at a time.
    Found {
        id: u64,
        files: Vec<FoundFile>,
    },
    /// A search finished; `capped` if more lines matched than it returns.
    SearchDone {
        id: u64,
        capped: bool,
    },
    SearchFailed {
        id: u64,
        why: String,
    },
    Completions {
        path: String,
        dirs: Vec<String>,
    },
    RepoChecked {
        path: String,
        check: Result<RepoCheck, String>,
    },
    /// A repository was added; `repos` is the list with it.
    RepoAdded {
        name: String,
        repos: Vec<Repo>,
    },
    AddFailed {
        why: String,
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
    /// No icon, in the text colour: "/gstin · 3 of 12".
    Plain,
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

/// A line typed in the status line: `/` text to find, `:` a line to go to.
#[derive(Debug, Clone)]
pub struct Prompt {
    pub kind: PromptKind,
    pub input: Input,
    /// Where the cursor and the view were when it opened, to go back to.
    origin: (Pos, usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptKind {
    Search,
    Line,
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
    /// A prompt being typed; it takes every key until enter or esc.
    pub prompt: Option<Prompt>,
    /// The last search, for `n` and `N`.
    pub search: Option<search::Search>,
    /// Whether the viewer shows the last search's matches; esc hides them.
    pub highlight: bool,
    /// The switcher, while it's open; it takes every key.
    pub switcher: Option<Switcher>,
    /// Find in files, kept when closed so it opens on the last search.
    pub find: find::Find,
    /// Whether find in files is open; it takes every key.
    pub find_open: bool,
    /// The add repository dialog, while it's open; it takes every key.
    pub add_repo: Option<AddRepo>,
    /// Seconds since the Unix epoch, for "2h ago". The loop that runs the app keeps it current.
    pub now: u64,
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
            prompt: None,
            search: None,
            highlight: false,
            switcher: None,
            find: find::Find::default(),
            find_open: false,
            add_repo: None,
            now: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs()),
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
            Focus::Viewer if self.current_viewer().is_some_and(|v| v.selection.is_some()) => {
                Context::Visual
            }
            Focus::Viewer => Context::Viewer,
        }
    }

    pub fn handle_key(&mut self, event: KeyEvent) {
        if event.kind != KeyEventKind::Press {
            return;
        }
        // A message lasts until the next key, as in Vim.
        self.status = None;
        if self.switcher.is_some() {
            return self.switcher_key(event);
        }
        if self.find_open {
            return self.find_key(event);
        }
        if self.add_repo.is_some() {
            return self.add_repo_key(event);
        }
        if self.prompt.is_some() {
            return self.prompt_key(event);
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
        if matches!(context, Context::Viewer | Context::Visual) && self.pending.is_empty() {
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
        if matches!(self.context(), Context::Viewer | Context::Visual)
            && self.viewer_action(action, count)
        {
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
            Action::CopyReference => self.copy_reference(),
            Action::Switcher | Action::FindWorktree => self.open_switcher(Mode::Worktrees),
            Action::FindFile => self.open_switcher(Mode::Files),
            Action::FindInFiles => self.open_find(),
            Action::AddRepo => self.add_repo = Some(AddRepo::default()),
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
            First => viewer.jump_to_line(count.map_or(0, |n| n - 1)),
            Last => viewer.jump_to_line(count.map_or(usize::MAX, |n| n - 1)),
            ScreenTop | ScreenMiddle | ScreenBottom => {
                let (top, bottom) = viewer.screen();
                let line = match action {
                    ScreenTop => (top + n - 1).min(bottom),
                    ScreenBottom => bottom.saturating_sub(n - 1).max(top),
                    _ => top + (bottom - top) / 2,
                };
                viewer.jump_to_line(line);
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
                    viewer.jump_to_line(line.saturating_sub(1));
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
            VisualChars => viewer.visual(false),
            VisualLines => viewer.visual(true),
            OtherEnd => viewer.other_end(),
            ExitVisual => viewer.selection = None,
            Copy => {
                if let Some((text, from, to, by_line)) = viewer.take_selection() {
                    let size = if by_line || from.line != to.line {
                        plural(to.line - from.line + 1, "line")
                    } else {
                        plural(to.col - from.col + 1, "character")
                    };
                    self.copy(text, size);
                }
            }
            // In visual mode, the selected lines whole, as Vim's Y does.
            CopyLine if viewer.selection.is_some() => {
                if let Some(s) = &mut viewer.selection {
                    s.by_line = true;
                }
                if let Some((text, from, to, _)) = viewer.take_selection() {
                    self.copy(text, plural(to.line - from.line + 1, "line"));
                }
            }
            CopyLine => {
                if let Some(text) = viewer.copy_lines(n) {
                    let size = plural(text.matches('\n').count(), "line");
                    self.copy(text, size);
                }
            }
            Action::Search | GoToLine => {
                let kind = if action == GoToLine {
                    PromptKind::Line
                } else {
                    PromptKind::Search
                };
                self.prompt = Some(Prompt {
                    kind,
                    input: Input::default(),
                    origin: (viewer.cursor, viewer.scroll),
                });
            }
            NextMatch | PreviousMatch => {
                let from = viewer.cursor;
                self.find_match(from, n, action == PreviousMatch);
            }
            SearchWord | SearchWordBack => match motion::word_bounds(viewer.lines(), at) {
                Some((start, end)) => {
                    let word: String = viewer.lines()[at.line]
                        .chars()
                        .skip(start)
                        .take(end - start)
                        .collect();
                    self.search = Some(search::Search::word(word, action == SearchWord));
                    // From the word's start, so `#` doesn't find the word itself.
                    self.find_match(Pos::new(at.line, start), n, false);
                }
                None => {
                    self.status = Some(Status::new(StatusKind::Error, "no word under the cursor"));
                }
            },
            FocusLeft if self.highlight => self.highlight = false,
            _ => return false,
        }
        true
    }

    /// The next match of the last search from `from`, `count` times over, the
    /// way it last went, or the other way for `N`. Says which match it is.
    fn find_match(&mut self, from: Pos, count: usize, reverse: bool) {
        let Some(search) = self.search.clone() else {
            self.status = Some(Status::new(StatusKind::Error, "nothing to find yet"));
            return;
        };
        self.highlight = true;
        let Some(viewer) = self.current.as_ref().and_then(|p| self.viewers.get_mut(p)) else {
            return;
        };
        let forward = search.forward != reverse;
        let (mut at, mut wrapped) = (from, false);
        for _ in 0..count.max(1) {
            let Some(found) = search.next(viewer.lines(), at, forward) else {
                let text = format!("not found: {}", search.pattern);
                self.status = Some(Status::new(StatusKind::Error, text));
                return;
            };
            (at, wrapped) = (found.at, wrapped || found.wrapped);
        }
        viewer.jump(at);
        let (index, total) = search.count(viewer.lines(), at);
        let summary = format!("/{} · {index} of {total}", search.pattern);
        self.status = Some(if wrapped {
            let end = if forward { "top" } else { "bottom" };
            Status::new(
                StatusKind::Warning,
                format!("{summary} · continued from the {end}"),
            )
        } else {
            Status::new(StatusKind::Plain, summary)
        });
    }

    /// A key for the open prompt: enter runs it, esc (or backspace with
    /// nothing typed) cancels it, and the rest edit it.
    fn prompt_key(&mut self, event: KeyEvent) {
        let empty = self
            .prompt
            .as_ref()
            .is_some_and(|p| p.input.value().is_empty());
        match event.code {
            KeyCode::Enter => {
                if let Some(prompt) = self.prompt.take() {
                    self.run_prompt(prompt);
                }
            }
            KeyCode::Esc => self.cancel_prompt(),
            KeyCode::Backspace if empty => self.cancel_prompt(),
            _ => {
                let Some(prompt) = &mut self.prompt else {
                    return;
                };
                if let Some(request) = to_input_request(&Event::Key(event)) {
                    prompt.input.handle(request);
                }
                if prompt.kind == PromptKind::Search {
                    self.search_as_typed();
                }
            }
        }
    }

    /// Moves the cursor to the first match of what's typed so far, from
    /// where it was when `/` opened, as Vim's `incsearch` does.
    fn search_as_typed(&mut self) {
        let Some(prompt) = &self.prompt else {
            return;
        };
        let search = search::Search::new(prompt.input.value(), true);
        let (cursor, scroll) = prompt.origin;
        if let Some(viewer) = self.current.as_ref().and_then(|p| self.viewers.get_mut(p)) {
            viewer.restore(cursor, scroll);
            if let Some(found) = search.next(viewer.lines(), cursor, true) {
                viewer.go(found.at);
            }
        }
    }

    /// Closes the prompt with the cursor back where it was.
    fn cancel_prompt(&mut self) {
        if let Some(Prompt {
            origin: (cursor, scroll),
            ..
        }) = self.prompt.take()
            && let Some(viewer) = self.current.as_ref().and_then(|p| self.viewers.get_mut(p))
        {
            viewer.restore(cursor, scroll);
        }
    }

    fn run_prompt(&mut self, prompt: Prompt) {
        let (cursor, scroll) = prompt.origin;
        if let Some(viewer) = self.current.as_ref().and_then(|p| self.viewers.get_mut(p)) {
            viewer.restore(cursor, scroll);
        }
        let value = prompt.input.value();
        match prompt.kind {
            PromptKind::Search => {
                // Nothing typed finds the last search again, onwards, as in Vim.
                match &mut self.search {
                    _ if !value.is_empty() => {
                        self.search = Some(search::Search::new(value, true));
                    }
                    Some(last) => last.forward = true,
                    None => return,
                }
                self.find_match(cursor, 1, false);
            }
            PromptKind::Line => self.go_to_line(value.trim()),
        }
    }

    /// What `:` takes: a line number, `$` for the last line, or `noh` to
    /// hide the search's matches.
    fn go_to_line(&mut self, value: &str) {
        let line = match value {
            "" => return,
            "noh" | "nohlsearch" => {
                self.highlight = false;
                return;
            }
            "$" => usize::MAX,
            other => match other.parse::<usize>() {
                Ok(n) => n.saturating_sub(1),
                Err(_) => {
                    let text = format!("not a line number: {other}");
                    self.status = Some(Status::new(StatusKind::Error, text));
                    return;
                }
            },
        };
        if let Some(viewer) = self.current.as_ref().and_then(|p| self.viewers.get_mut(p)) {
            viewer.jump_to_line(line);
        }
    }

    /// What the switcher matches in `mode`: every worktree that's there, most
    /// recently opened first, or the current worktree's files in path order.
    pub fn switcher_source(&self, mode: Mode) -> Source {
        match mode {
            Mode::Worktrees => {
                let mut worktrees: Vec<(&Repo, &Worktree)> = self
                    .repos
                    .iter()
                    .flat_map(|r| r.worktrees().iter().map(move |w| (r, w)))
                    .filter(|(_, w)| !w.missing)
                    .collect();
                // Never-opened last; the sort is stable, so ties keep the sidebar's order.
                worktrees.sort_by_key(|(_, w)| std::cmp::Reverse(w.last_opened));
                let candidates = worktrees
                    .into_iter()
                    .map(|(repo, w)| Candidate {
                        target: Target::Worktree(w.path.clone()),
                        text: format!("{} {} {}", repo.name, w.name, head_name(&w.head)),
                    })
                    .collect();
                Source::Ready(candidates)
            }
            Mode::Files => match self.current_listing() {
                None => Source::NoWorktree,
                Some(Listing::Loading) => Source::Loading,
                Some(Listing::Failed(why)) => Source::Failed(why.clone()),
                Some(Listing::Ready(explorer)) => Source::Ready(
                    explorer
                        .files()
                        .into_iter()
                        .map(|path| Candidate {
                            target: Target::File(path.clone()),
                            text: path,
                        })
                        .collect(),
                ),
            },
        }
    }

    /// Opens the switcher. Listing worktrees, it starts on the one before the
    /// current, so `ctrl-p enter` goes back to it.
    fn open_switcher(&mut self, mode: Mode) {
        let mut switcher = Switcher::new(mode, self.switcher_source(mode));
        if mode == Mode::Worktrees {
            let current = self.current.clone().map(Target::Worktree);
            let candidates = switcher.candidates();
            if let Some(i) = switcher
                .hits
                .iter()
                .position(|h| Some(&candidates[h.candidate].target) != current.as_ref())
            {
                switcher.select(i);
            }
        }
        self.switcher = Some(switcher);
    }

    /// A key for the open switcher: enter opens the selection, esc closes, tab
    /// changes mode, the arrows move, and the rest edit the query.
    fn switcher_key(&mut self, event: KeyEvent) {
        let Some(switcher) = &mut self.switcher else {
            return;
        };
        match event.code {
            KeyCode::Esc => self.switcher = None,
            KeyCode::Enter => {
                let target = switcher.target().cloned();
                if let Some(target) = target {
                    self.switcher = None;
                    self.open_target(target);
                }
            }
            KeyCode::Tab => {
                let mode = switcher.mode.other();
                let source = self.switcher_source(mode);
                if let Some(switcher) = &mut self.switcher {
                    switcher.switch(mode, source);
                }
            }
            KeyCode::Down => switcher.down(),
            KeyCode::Up => switcher.up(),
            _ => {
                if let Some(request) = to_input_request(&Event::Key(event))
                    && switcher
                        .input
                        .handle(request)
                        .is_some_and(|changed| changed.value)
                {
                    switcher.refresh();
                }
            }
        }
    }

    /// Opens what the switcher chose: a worktree, or a file in the current
    /// one, its folders opened in the explorer and the viewer focused.
    fn open_target(&mut self, target: Target) {
        match target {
            Target::Worktree(path) => self.open(&path),
            Target::File(file) => {
                let path: NodePath = file.split('/').map(String::from).collect();
                if let Some(Listing::Ready(explorer)) =
                    self.current.as_ref().and_then(|p| self.listings.get_mut(p))
                {
                    explorer.reveal(&path);
                }
                // A file already open keeps its place.
                if self.current_viewer().is_none_or(|v| v.path != path) {
                    self.open_file(path);
                }
                self.focus = Focus::Viewer;
            }
        }
    }

    /// Opens find in files on the last search. Its results stand unless the
    /// worktree changed since, when the search runs again here.
    fn open_find(&mut self) {
        if self.current.is_none() {
            return;
        }
        self.find_open = true;
        if self.find.worktree != self.current {
            self.find.worktree = self.current.clone();
            self.start_search();
        }
    }

    /// Starts a search for find's query, if it has one.
    fn start_search(&mut self) {
        if let (Some(worktree), Some((id, query))) = (self.find.worktree.clone(), self.find.start())
        {
            self.outbox.push(Cmd::Search {
                worktree,
                id,
                query,
                max_lines: find::MAX_LINES,
            });
        }
    }

    /// Time passing: starts find's search once typing has paused. The loop
    /// that runs the app calls it between keys.
    pub fn tick(&mut self, now: std::time::Instant) {
        if self.find.due(now) {
            self.start_search();
        }
        if let Some(add) = &mut self.add_repo
            && add.due(now)
        {
            let path = add.start_check();
            self.outbox.push(Cmd::CheckRepo { path });
        }
    }

    /// A key for find in files: enter opens the selected line, esc closes,
    /// the arrows move, and the rest edit the query.
    fn find_key(&mut self, event: KeyEvent) {
        match event.code {
            KeyCode::Esc => self.find_open = false,
            KeyCode::Enter => {
                if let Some((path, number)) = self.find.selected.clone() {
                    self.find_open = false;
                    self.open_found(&path, number as usize - 1);
                }
            }
            KeyCode::Down => self.find.down(),
            KeyCode::Up => self.find.up(),
            _ => {
                if let Some(request) = to_input_request(&Event::Key(event))
                    && self
                        .find
                        .input
                        .handle(request)
                        .is_some_and(|changed| changed.value)
                {
                    self.find.edited(std::time::Instant::now());
                }
            }
        }
    }

    /// Whether a repository already has `name`.
    pub fn name_taken(&self, name: &str) -> bool {
        self.repos.iter().any(|r| r.name == name)
    }

    /// A key for the add repository dialog: tab completes the path and then
    /// cycles, the arrows move between path and name, enter adds, esc
    /// cancels, and the rest edit the field.
    fn add_repo_key(&mut self, event: KeyEvent) {
        let taken: Vec<String> = self.repos.iter().map(|r| r.name.clone()).collect();
        let Some(add) = self.add_repo.as_mut() else {
            return;
        };
        match event.code {
            KeyCode::Esc => self.add_repo = None,
            KeyCode::Enter => {
                if add.ready(|name| taken.iter().any(|t| t == name)) {
                    add.stage = Stage::Adding;
                    let (path, name) = (add.path.value().to_string(), add.name().to_string());
                    self.outbox.push(Cmd::AddRepo { path, name });
                }
            }
            KeyCode::Up | KeyCode::Down => {
                add.completions = None;
                add.field = match add.field {
                    Field::Path => Field::Name,
                    Field::Name => Field::Path,
                };
            }
            KeyCode::Tab | KeyCode::BackTab if add.field == Field::Path => {
                if add.completions.is_some() {
                    add.cycle(event.code == KeyCode::Tab);
                    let path = add.start_check();
                    self.outbox.push(Cmd::CheckRepo { path });
                } else {
                    let path = add.path.value().to_string();
                    self.outbox.push(Cmd::Complete { path });
                }
            }
            _ => {
                let input = match add.field {
                    Field::Path => &mut add.path,
                    Field::Name => &mut add.name,
                };
                let changed = to_input_request(&Event::Key(event))
                    .and_then(|request| input.handle(request))
                    .is_some_and(|changed| changed.value);
                if changed && add.field == Field::Path {
                    add.path_edited(std::time::Instant::now());
                }
            }
        }
    }

    /// Opens a file find in files found at `line`, with the query as the
    /// viewer's search so its matches stay highlighted and `n` goes on.
    fn open_found(&mut self, path: &str, line: usize) {
        self.search = Some(search::Search::new(self.find.query.clone(), true));
        self.highlight = true;
        self.open_target(Target::File(path.to_string()));
        let Some(viewer) = self.current.as_ref().and_then(|p| self.viewers.get_mut(p)) else {
            return;
        };
        if viewer.has_text() {
            self.land(line);
        } else {
            viewer.landing = Some(line);
        }
    }

    /// Puts the viewer's cursor on the search's first match in `line`, as a
    /// jump, and says which match it is.
    fn land(&mut self, line: usize) {
        let Some(search) = self.search.clone() else {
            return;
        };
        let Some(viewer) = self.current.as_ref().and_then(|p| self.viewers.get_mut(p)) else {
            return;
        };
        let col = viewer
            .lines()
            .get(line)
            .and_then(|l| search.matches(l).first().map(|m| m.0))
            .unwrap_or(0);
        viewer.jump(Pos::new(line, col));
        let at = viewer.cursor;
        let (index, total) = search.count(viewer.lines(), at);
        let summary = format!("/{} · {index} of {total}", search.pattern);
        self.status = Some(Status::new(StatusKind::Plain, summary));
    }

    /// Puts `text` on the clipboard and says what went.
    fn copy(&mut self, text: String, what: String) {
        self.outbox.push(Cmd::Copy(text));
        self.status = Some(Status::new(StatusKind::Ok, format!("copied {what}")));
    }

    /// `g y`: copies an @-mention for a Claude Code chat: of the explorer's
    /// selection, or of the viewer's file with the selected lines. A
    /// selection ends with the cursor at its start, as `y` leaves it.
    fn copy_reference(&mut self) {
        let reference = match self.context() {
            Context::Explorer => match self.current_listing() {
                Some(Listing::Ready(explorer)) => {
                    let path = explorer.selected();
                    if path.is_empty() {
                        return;
                    }
                    reference(&path, None)
                }
                _ => return,
            },
            Context::Viewer | Context::Visual => {
                let Some(viewer) = self.current.as_ref().and_then(|p| self.viewers.get_mut(p))
                else {
                    return;
                };
                let selected = viewer.selected();
                if let Some((from, ..)) = selected {
                    viewer.selection = None;
                    viewer.go(from);
                }
                let lines = selected.map(|(from, to, _)| (from.line + 1, to.line + 1));
                reference(&viewer.path, lines)
            }
            Context::Global | Context::Sidebar => return,
        };
        self.copy(reference.clone(), reference);
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
                let landing = viewer.landing.take().filter(|_| viewer.has_text());
                if let Document::Text(text) = &viewer.document
                    && text.lang != Lang::Plain
                {
                    let (source, lang) = (text.source(), text.lang);
                    self.outbox.push(Cmd::Highlight {
                        worktree: worktree.clone(),
                        path,
                        source,
                        lang,
                    });
                }
                if let Some(line) = landing
                    && self.current.as_ref() == Some(&worktree)
                {
                    self.land(line);
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
            Msg::CopyFailed(why) => {
                let text = format!("couldn't copy: {why}");
                self.status = Some(Status::new(StatusKind::Error, text));
            }
            Msg::Found { id, files } => self.find.found(id, files),
            Msg::Completions { path, dirs } => {
                // One folder completes the path, so check it; several leave it partial.
                if let Some(add) = &mut self.add_repo
                    && add.completed(&path, dirs)
                    && add.completions.is_none()
                {
                    let path = add.start_check();
                    self.outbox.push(Cmd::CheckRepo { path });
                }
            }
            Msg::RepoChecked { path, check } => {
                if let Some(add) = &mut self.add_repo {
                    add.checked(&path, check);
                }
            }
            Msg::RepoAdded { name, repos } => {
                self.repos = repos;
                self.add_repo = None;
                self.status = Some(Status::new(StatusKind::Ok, format!("added {name}")));
            }
            Msg::AddFailed { why } => {
                if let Some(add) = &mut self.add_repo {
                    add.stage = Stage::Failed(why);
                }
            }
            Msg::SearchDone { id, capped } => self.find.done(id, capped),
            Msg::SearchFailed { id, why } => self.find.failed(id, why),
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
        // Leaving a viewer ends its visual mode, as leaving a window does in Vim.
        if let Some(viewer) = self.current.as_ref().and_then(|p| self.viewers.get_mut(p)) {
            viewer.selection = None;
        }
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

/// A branch's name, or a detached HEAD's commit.
pub fn head_name(head: &Head) -> &str {
    match head {
        Head::Branch(name) | Head::Detached(name) => name,
    }
}

/// "1 line", "3 lines".
pub fn plural(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
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
    use crate::add_repo::{self, Check};
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
    fn visual_mode_selects_and_y_copies_to_the_clipboard() {
        let mut app = viewing("packages/integrations/gst/client.ts");
        press_drawn(&mut app, "w w v e");
        assert_eq!(app.context(), Context::Visual);
        press_drawn(&mut app, "y");
        assert_eq!(app.outbox, [Cmd::Copy("Record".into())]);
        assert_eq!(app.status.as_ref().unwrap().text, "copied 6 characters");
        assert_eq!((app.context(), cursor(&app)), (Context::Viewer, (0, 12)));
        press_drawn(&mut app, "j");
        assert_eq!(app.status, None, "the message goes with the next key");
    }

    #[test]
    fn a_copy_that_fails_says_so_instead() {
        let mut app = viewing("packages/integrations/gst/client.ts");
        press_drawn(&mut app, "y y");
        app.apply(Msg::CopyFailed("pbcopy: exited with 1".into()));
        assert_eq!(
            app.status,
            Some(Status::new(
                StatusKind::Error,
                "couldn't copy: pbcopy: exited with 1"
            ))
        );
    }

    fn copied(app: &mut App) -> String {
        match app.outbox.pop() {
            Some(Cmd::Copy(text)) => text,
            other => panic!("nothing copied: {other:?}"),
        }
    }

    #[test]
    fn g_y_copies_a_reference_to_the_file_or_its_selected_lines() {
        let mut app = viewing("packages/integrations/gst/client.ts");
        press_drawn(&mut app, "j g y");
        assert_eq!(copied(&mut app), "@packages/integrations/gst/client.ts");
        assert_eq!(status(&app), "copied @packages/integrations/gst/client.ts");
        press_drawn(&mut app, "V j j g y");
        assert_eq!(copied(&mut app), "@packages/integrations/gst/client.ts:2-4");
        assert_eq!((app.context(), cursor(&app)), (Context::Viewer, (1, 0)));
        press_drawn(&mut app, "v l g y");
        assert_eq!(copied(&mut app), "@packages/integrations/gst/client.ts:2");
    }

    #[test]
    fn g_y_in_the_explorer_copies_the_selected_path() {
        let mut app = with_files();
        press_drawn(&mut app, "g y");
        assert_eq!(copied(&mut app), "@packages/integrations/gst/verify.ts");
        press_drawn(&mut app, "k k k g y");
        assert_eq!(copied(&mut app), "@packages/integrations");
    }

    #[test]
    fn shift_y_copies_lines_or_the_selected_lines_whole() {
        let mut app = viewing("packages/integrations/gst/client.ts");
        press_drawn(&mut app, "Y");
        assert_eq!(
            copied(&mut app),
            "export type Record = { gstin: string; legalName: string; state: string }\n"
        );
        press_drawn(&mut app, "j j w v j Y");
        assert_eq!(
            copied(&mut app),
            "export interface GstClient {\n  lookup(gstin: string): Promise<Record | undefined>\n"
        );
        assert_eq!(status(&app), "copied 2 lines");
    }

    #[test]
    fn yy_copies_lines_and_shift_v_selects_them() {
        let mut app = viewing("packages/integrations/gst/client.ts");
        press_drawn(&mut app, "2 y y");
        assert_eq!(
            app.outbox,
            [Cmd::Copy(
                "export type Record = { gstin: string; legalName: string; state: string }\n\n"
                    .into()
            )]
        );
        assert_eq!(app.status.as_ref().unwrap().text, "copied 2 lines");
        app.outbox.clear();
        app.handle_key(KeyEvent::new(KeyCode::Char('V'), KeyModifiers::SHIFT));
        press_drawn(&mut app, "j j y");
        let Some(Cmd::Copy(text)) = app.outbox.pop() else {
            panic!("nothing copied")
        };
        assert_eq!(text.lines().count(), 3);
        assert_eq!(app.status.as_ref().unwrap().text, "copied 3 lines");
    }

    #[test]
    fn esc_leaves_visual_mode_before_the_viewer() {
        let mut app = viewing("packages/integrations/gst/client.ts");
        let esc = || KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        press_drawn(&mut app, "v l");
        app.handle_key(esc());
        assert_eq!((app.context(), app.focus), (Context::Viewer, Focus::Viewer));
        app.handle_key(esc());
        assert_eq!(app.focus, Focus::Explorer);
    }

    fn key(app: &mut App, code: KeyCode) {
        app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
    }

    fn status(app: &App) -> &str {
        app.status.as_ref().map_or("", |s| s.text.as_str())
    }

    #[test]
    fn slash_finds_as_it_is_typed_and_n_goes_on() {
        let mut app = viewing("packages/integrations/gst/client.ts");
        press_drawn(&mut app, "/ g s t");
        assert_eq!(cursor(&app), (0, 23), "found while typing");
        press_drawn(&mut app, "i n enter");
        assert_eq!((cursor(&app), status(&app)), ((0, 23), "/gstin · 1 of 5"));
        press_drawn(&mut app, "n n");
        assert_eq!(cursor(&app), (9, 15));
        press_drawn(&mut app, "N");
        assert_eq!(cursor(&app), (3, 9));
        press_drawn(&mut app, "3 n");
        assert_eq!(cursor(&app), (10, 55));
        press_drawn(&mut app, "n");
        assert_eq!(cursor(&app), (0, 23));
        assert_eq!(status(&app), "/gstin · 1 of 5 · continued from the top");
        press_drawn(&mut app, "ctrl-o");
        assert_eq!(cursor(&app), (10, 55), "each find is a jump");
    }

    #[test]
    fn esc_cancels_a_search_then_hides_its_matches_then_leaves() {
        let mut app = viewing("packages/integrations/gst/client.ts");
        press_drawn(&mut app, "j / g s t");
        assert_eq!(
            cursor(&app),
            (2, 17),
            "GstClient: lower case matches either"
        );
        key(&mut app, KeyCode::Esc);
        assert_eq!((cursor(&app), app.prompt.is_none()), ((1, 0), true));
        press_drawn(&mut app, "/ g s t enter");
        assert!(app.highlight);
        key(&mut app, KeyCode::Esc);
        assert_eq!((app.highlight, app.focus), (false, Focus::Viewer));
        key(&mut app, KeyCode::Esc);
        assert_eq!(app.focus, Focus::Explorer);
    }

    #[test]
    fn backspace_with_nothing_typed_closes_the_prompt() {
        let mut app = viewing("packages/integrations/gst/client.ts");
        press_drawn(&mut app, "/ x");
        key(&mut app, KeyCode::Backspace);
        assert!(app.prompt.is_some());
        key(&mut app, KeyCode::Backspace);
        assert!(app.prompt.is_none());
    }

    #[test]
    fn colon_goes_to_a_line() {
        let mut app = viewing("generated/rows.rs");
        press_drawn(&mut app, ": 4 2 enter");
        assert_eq!(cursor(&app).0, 41);
        press_drawn(&mut app, ": $ enter");
        assert_eq!(cursor(&app).0, 9_999);
        press_drawn(&mut app, ": x enter");
        assert_eq!(status(&app), "not a line number: x");
        press_drawn(&mut app, "ctrl-o");
        assert_eq!(cursor(&app).0, 41);
    }

    #[test]
    fn star_and_hash_find_the_word_under_the_cursor() {
        let mut app = viewing("packages/integrations/gst/client.ts");
        press_drawn(&mut app, "4 G w w");
        assert_eq!(cursor(&app), (3, 9));
        press_drawn(&mut app, "*");
        assert_eq!(cursor(&app), (9, 15));
        press_drawn(&mut app, "#");
        assert_eq!(cursor(&app), (3, 9));
        press_drawn(&mut app, "n");
        assert_eq!(cursor(&app), (0, 23), "after #, n goes on backwards");
    }

    fn ctrl_p(app: &mut App) {
        app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));
    }

    fn switcher(app: &App) -> &Switcher {
        app.switcher.as_ref().expect("the switcher is open")
    }

    #[test]
    fn ctrl_p_enter_goes_back_to_the_last_worktree() {
        let mut app = with_files();
        let gst_check = fake::current();
        press_drawn(&mut app, "h j enter");
        assert_ne!(app.current.as_ref(), Some(&gst_check));
        ctrl_p(&mut app);
        key(&mut app, KeyCode::Enter);
        assert_eq!(
            (app.current, app.switcher.is_none()),
            (Some(gst_check), true)
        );
    }

    #[test]
    fn the_switcher_filters_as_the_query_is_typed() {
        let mut app = with_files();
        ctrl_p(&mut app);
        assert_eq!(switcher(&app).hits.len(), 7);
        press(&mut app, "g s t");
        assert_eq!(switcher(&app).hits.len(), 3);
        key(&mut app, KeyCode::Down);
        assert_eq!(switcher(&app).selected, 1);
        key(&mut app, KeyCode::Esc);
        assert!(app.switcher.is_none());
        assert_eq!(app.current, Some(fake::current()), "esc opened nothing");
    }

    #[test]
    fn space_f_opens_a_file_with_its_folders_and_focuses_the_viewer() {
        let mut app = with_files();
        press(&mut app, "space f s c o r e");
        assert_eq!(switcher(&app).mode, Mode::Files);
        key(&mut app, KeyCode::Enter);
        assert_eq!(app.focus, Focus::Viewer);
        assert_eq!(selected(&app), "packages/scoring/score.py");
        assert_eq!(
            app.outbox,
            [Cmd::ReadFile {
                worktree: fake::current(),
                path: "packages/scoring/score.py".into()
            }]
        );
    }

    #[test]
    fn a_file_already_open_keeps_its_place() {
        let mut app = with_files();
        app.viewers.insert(
            fake::current(),
            fake::viewer("packages/integrations/gst/verify.ts"),
        );
        press(&mut app, "space f v e r i f y . t s");
        key(&mut app, KeyCode::Enter);
        assert!(app.outbox.is_empty(), "not read again");
        assert_eq!(app.focus, Focus::Viewer);
    }

    #[test]
    fn tab_changes_mode_and_keeps_the_query() {
        let mut app = with_files();
        ctrl_p(&mut app);
        press(&mut app, "g s t");
        key(&mut app, KeyCode::Tab);
        assert_eq!(
            (switcher(&app).mode, switcher(&app).input.value()),
            (Mode::Files, "gst")
        );
        assert_eq!(
            app.current,
            Some(fake::current()),
            "tab didn't switch worktrees"
        );
    }

    /// Answers the search waiting in the outbox from the fake files.
    fn answer_search(app: &mut App) -> String {
        let Some(Cmd::Search {
            worktree,
            id,
            query,
            max_lines,
        }) = app.outbox.pop()
        else {
            panic!("no search asked for")
        };
        let files = match app.listings.get(&worktree) {
            Some(Listing::Ready(e)) => e.files(),
            _ => panic!("not listed"),
        };
        let (found, capped) = fake::search(&files, &query, max_lines);
        app.apply(Msg::Found { id, files: found });
        app.apply(Msg::SearchDone { id, capped });
        query
    }

    fn after_a_pause(app: &mut App) {
        app.tick(std::time::Instant::now() + find::DELAY);
    }

    #[test]
    fn space_slash_finds_in_files_once_typing_pauses() {
        let mut app = with_files();
        press(&mut app, "space / g s t i n");
        assert!(app.find_open);
        assert!(app.outbox.is_empty(), "nothing asked while typing");
        after_a_pause(&mut app);
        assert_eq!(answer_search(&mut app), "gstin");
        assert_eq!((app.find.lines(), app.find.files.len()), (25, 5));
        assert_eq!(
            app.find.selected,
            Some(("apps/web/src/pages/home.tsx".into(), 5))
        );
    }

    #[test]
    fn enter_opens_the_line_found_with_its_matches_highlighted() {
        let mut app = with_files();
        press(&mut app, "space / g s t i n");
        after_a_pause(&mut app);
        answer_search(&mut app);
        for _ in 0..7 {
            key(&mut app, KeyCode::Down);
        }
        key(&mut app, KeyCode::Enter);
        assert_eq!(
            (app.find_open, app.focus, app.highlight),
            (false, Focus::Viewer, true)
        );
        let path = "packages/integrations/gst/client.ts".to_string();
        assert_eq!(
            app.outbox,
            [Cmd::ReadFile {
                worktree: fake::current(),
                path: path.clone()
            }]
        );
        app.outbox.clear();
        app.apply(Msg::FileRead {
            worktree: fake::current(),
            path: path.clone(),
            contents: fake::read(&path),
        });
        assert_eq!(
            cursor(&app),
            (3, 9),
            "on line 4's match once the file arrives"
        );
        assert_eq!(status(&app), "/gstin · 2 of 5");
        press_drawn(&mut app, "n");
        assert_eq!(cursor(&app), (9, 15), "n goes on through the file");
    }

    #[test]
    fn find_opens_again_on_the_last_search() {
        let mut app = with_files();
        press(&mut app, "space / g s t i n");
        after_a_pause(&mut app);
        answer_search(&mut app);
        key(&mut app, KeyCode::Esc);
        assert!(!app.find_open);
        press(&mut app, "space /");
        assert_eq!((app.find.input.value(), app.find.lines()), ("gstin", 25));
        assert!(app.outbox.is_empty(), "not searched again");
    }

    #[test]
    fn find_in_another_worktree_searches_again() {
        let mut app = with_files();
        press(&mut app, "space / g s t i n");
        after_a_pause(&mut app);
        answer_search(&mut app);
        key(&mut app, KeyCode::Esc);
        press_drawn(&mut app, "h j enter space /");
        let Some(Cmd::Search {
            worktree, query, ..
        }) = app.outbox.last()
        else {
            panic!("no search")
        };
        assert_eq!(
            (worktree.as_path(), query.as_str()),
            (Path::new("~/code/wt/privue-platform/gst-rounding"), "gstin")
        );
    }

    /// Types `text`, one key per character.
    fn type_text(app: &mut App, text: &str) {
        for c in text.chars() {
            key(app, KeyCode::Char(c));
        }
    }

    /// Answers what the add repository dialog asked abcd, from the fake disk.
    fn answer_repo_cmds(app: &mut App) {
        for cmd in std::mem::take(&mut app.outbox) {
            let msg = match cmd {
                Cmd::Complete { path } => Msg::Completions {
                    dirs: fake::complete(&path),
                    path,
                },
                Cmd::CheckRepo { path } => Msg::RepoChecked {
                    check: Ok(fake::check_repo(&path, &app.repos)),
                    path,
                },
                Cmd::AddRepo { path, name } => {
                    let mut repos = app.repos.clone();
                    repos.push(fake::added(&path, &name));
                    Msg::RepoAdded { name, repos }
                }
                other => panic!("not asked of the dialog: {other:?}"),
            };
            app.apply(msg);
        }
    }

    fn adding(app: &App) -> &AddRepo {
        app.add_repo.as_ref().expect("the dialog is open")
    }

    #[test]
    fn space_r_a_checks_the_path_once_typing_pauses() {
        let mut app = with_files();
        press(&mut app, "space r a");
        type_text(&mut app, "code/privue-agents/");
        assert!(app.outbox.is_empty(), "nothing asked while typing");
        app.tick(std::time::Instant::now() + add_repo::DELAY);
        answer_repo_cmds(&mut app);
        assert_eq!(
            adding(&app).check,
            Check::Checked(Ok(RepoCheck::Ready { worktrees: 3 }))
        );
        assert_eq!(adding(&app).name(), "privue-agents");
    }

    #[test]
    fn tab_completes_and_cycles_then_enter_adds_the_repository() {
        let mut app = with_files();
        press(&mut app, "space r a");
        type_text(&mut app, "code/privue-a");
        key(&mut app, KeyCode::Tab);
        answer_repo_cmds(&mut app);
        assert_eq!(adding(&app).path.value(), "~/code/privue-ag");
        key(&mut app, KeyCode::Tab);
        assert_eq!(adding(&app).path.value(), "~/code/privue-agents/");
        answer_repo_cmds(&mut app);
        key(&mut app, KeyCode::Enter);
        answer_repo_cmds(&mut app);
        assert!(app.add_repo.is_none());
        assert_eq!(status(&app), "added privue-agents");
        assert_eq!(
            app.repos.last().map(|r| r.name.as_str()),
            Some("privue-agents")
        );
    }

    #[test]
    fn enter_adds_nothing_but_a_repository_under_a_free_name() {
        let mut app = with_files();
        press(&mut app, "space r a");
        type_text(&mut app, "Downloads/");
        app.tick(std::time::Instant::now() + add_repo::DELAY);
        answer_repo_cmds(&mut app);
        key(&mut app, KeyCode::Enter);
        assert!(app.outbox.is_empty(), "not a git repository");
        key(&mut app, KeyCode::Esc);
        press(&mut app, "space r a");
        type_text(&mut app, "code/privue-agents/");
        app.tick(std::time::Instant::now() + add_repo::DELAY);
        answer_repo_cmds(&mut app);
        key(&mut app, KeyCode::Down);
        type_text(&mut app, "ecourts");
        key(&mut app, KeyCode::Enter);
        assert!(app.outbox.is_empty(), "the name is taken");
        key(&mut app, KeyCode::Esc);
        assert!(app.add_repo.is_none());
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
