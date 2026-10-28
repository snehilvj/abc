//! Drawing. Everything here is a pure function of the app state and theme.

pub mod add_repo;
pub mod catalog;
pub mod dialog;
pub mod explorer;
pub mod find;
pub mod help;
pub mod label;
pub mod remove_repo;
pub mod sidebar;
pub mod switcher;
pub mod viewer;
pub mod which_key;

use abc_core::model::{Checks, Head, PrState, Review};
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::app::{App, Connection, Entry, Focus, Prompt, PromptKind, StatusKind};
use crate::icons;
use crate::keys::{self, Action, Context};
use crate::layout::{self, Columns};
use crate::theme::Theme;

pub fn draw(frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    draw_buffer(frame.buffer_mut(), area, app, theme);
}

pub fn draw_buffer(buf: &mut Buffer, area: Rect, app: &App, theme: &Theme) {
    if layout::too_small(area.width, area.height) {
        let text = format!(
            "abc needs at least {}x{} (now {}x{})",
            layout::MIN_WIDTH,
            layout::MIN_HEIGHT,
            area.width,
            area.height
        );
        let x = area.x + area.width.saturating_sub(text.chars().count() as u16) / 2;
        buf.set_string(x, area.y + area.height / 2 - 1, text, theme.text());
        return;
    }
    let g = Geometry::new(area, layout::columns(area.width, app.widths), split(app));
    chrome(buf, &g, app, theme);
    sidebar(buf, &g, app, theme);
    match notice(app) {
        Some(lines) => centred(buf, &g, &lines, theme),
        None => panes(buf, &g, app, theme),
    }
    if app.connection == Connection::Lost {
        fade(
            buf,
            Rect::new(g.left + 1, g.top + 1, area.width - 2, area.height - 4),
            theme,
        );
    }
    keybar(buf, &g, app, theme);
    status_line(buf, &g, app, theme);
    if app.which_key && !app.pending.is_empty() {
        which_key::draw(buf, area, app, theme);
    }
    if let Some(s) = &app.switcher {
        fade(buf, area, theme);
        switcher::draw(buf, area, app, s, theme);
    }
    if app.find_open {
        fade(buf, area, theme);
        find::draw(buf, area, app, theme);
    }
    if let Some(add) = &app.add_repo {
        fade(buf, area, theme);
        add_repo::draw(buf, area, app, add, theme);
    }
    if let Some(remove) = &app.remove_repo {
        fade(buf, area, theme);
        remove_repo::draw(buf, area, app, remove, theme);
    }
    if app.help {
        fade(buf, area, theme);
        help::draw(buf, area, app, theme);
    }
}

/// Whether the Code tab shows the explorer and viewer side by side.
fn split(app: &App) -> bool {
    notice(app).is_none()
}

/// Positions of the frame's lines and columns.
struct Geometry {
    left: u16,
    right: u16,
    top: u16,
    bottom: u16,
    /// The sidebar | explorer divider, and the explorer | viewer one.
    d1: u16,
    d2: u16,
    cols: Columns,
    split: bool,
}

impl Geometry {
    fn new(area: Rect, cols: Columns, split: bool) -> Self {
        let d1 = area.x + 1 + cols.sidebar;
        Self {
            left: area.x,
            right: area.right() - 1,
            top: area.y,
            bottom: area.bottom() - 1,
            d1,
            d2: d1 + 1 + cols.explorer,
            cols,
            split,
        }
    }

    /// The row under the title: "Worktrees" heads the sidebar, "Code" the explorer and viewer.
    fn tab_row(&self) -> u16 {
        self.top + 1
    }

    /// The line under the tab bar, carrying the explorer's and viewer's headers.
    fn header_row(&self) -> u16 {
        self.top + 2
    }

    /// First and last content rows of the explorer and viewer, from under
    /// their headers down to the separator.
    fn content_rows(&self) -> (u16, u16) {
        (self.top + 3, self.bottom - 3)
    }

    /// The sidebar's rows: it has no header line, so they start right under "Worktrees".
    fn sidebar_rows(&self) -> (u16, u16) {
        (self.header_row(), self.bottom - 3)
    }
}

fn hline(buf: &mut Buffer, y: u16, x0: u16, x1: u16, style: Style) {
    for x in x0..=x1 {
        buf[(x, y)].set_symbol("─").set_style(style);
    }
}

fn vline(buf: &mut Buffer, x: u16, y0: u16, y1: u16, style: Style) {
    for y in y0..=y1 {
        buf[(x, y)].set_symbol("│").set_style(style);
    }
}

fn put(buf: &mut Buffer, x: u16, y: u16, symbol: &str, style: Style) {
    buf[(x, y)].set_symbol(symbol).set_style(style);
}

/// Borders, dividers, the tab bar, pane headers and the title.
fn chrome(buf: &mut Buffer, g: &Geometry, app: &App, theme: &Theme) {
    let border = theme.border();
    hline(buf, g.top, g.left, g.right, border);
    hline(buf, g.bottom, g.left, g.right, border);
    vline(buf, g.left, g.top, g.bottom, border);
    vline(buf, g.right, g.top, g.bottom, border);
    put(buf, g.left, g.top, "┌", border);
    put(buf, g.right, g.top, "┐", border);
    put(buf, g.left, g.bottom, "└", border);
    put(buf, g.right, g.bottom, "┘", border);

    // Sidebar divider, the explorer and viewer's heading, and the line under it.
    let header = g.header_row();
    vline(buf, g.d1, g.top + 1, g.bottom - 3, border);
    let code_focused = app.focus != Focus::Sidebar && app.current_worktree().is_some();
    buf.set_string(g.d1 + 2, g.tab_row(), "Code", heading(code_focused, theme));
    hline(buf, header, g.d1 + 1, g.right - 1, border);
    put(buf, g.d1, header, "├", border);
    put(buf, g.right, header, "┤", border);

    // The separator above the key bar.
    hline(buf, g.bottom - 2, g.left + 1, g.right - 1, border);
    put(buf, g.left, g.bottom - 2, "├", border);
    put(buf, g.right, g.bottom - 2, "┤", border);
    put(buf, g.d1, g.bottom - 2, "┴", border);

    if g.split {
        put(buf, g.d2, header, "┬", border);
        vline(buf, g.d2, header + 1, g.bottom - 3, border);
        put(buf, g.d2, g.bottom - 2, "┴", border);
        if let Some((_, w)) = app.current_worktree() {
            let room = g.cols.explorer.saturating_sub(3) as usize;
            let path = ltrunc(&w.path.display().to_string(), room);
            buf.set_string(
                g.d1 + 2,
                header,
                format!(" {path} "),
                theme.header(app.focus == Focus::Explorer),
            );
        }
    }

    // Title on the left of the top border, profile on the right.
    let profile_style = if app.profile == "dev" {
        theme.style(theme.warning).add_modifier(Modifier::BOLD)
    } else {
        theme.dim()
    };
    let profile = format!(" {} ", app.profile);
    let profile_x = g.right - profile.chars().count() as u16;
    buf.set_string(profile_x, g.top, &profile, profile_style);
    let title = title(app, theme);
    buf.set_line(
        g.left + 1,
        g.top,
        &title,
        profile_x.saturating_sub(g.left + 2),
    );
}

fn title<'a>(app: &'a App, theme: &Theme) -> Line<'a> {
    let dot = || Span::styled(" · ", theme.dim());
    let mut spans = vec![Span::raw(" "), Span::styled("abc", theme.bold())];
    let (repo, worktree) = match (app.current_worktree(), missing_repo_at_cursor(app)) {
        (_, Some(name)) => (Some(name), None),
        (Some((r, w)), None) => (Some(r.name.as_str()), Some(w)),
        (None, None) => (None, None),
    };
    if let Some(repo) = repo {
        spans.extend([dot(), Span::styled(repo, theme.text())]);
    }
    if let Some(w) = worktree {
        spans.extend([
            Span::styled(" / ", theme.dim()),
            Span::styled(w.name.as_str(), theme.text()),
        ]);
        if let Head::Branch(branch) = &w.head {
            spans.push(dot());
            spans.extend(label::labelled(
                icons::BRANCH,
                theme.dim(),
                branch.as_str(),
                theme.text(),
                theme,
            ));
        }
        if let Some(pr) = &w.pr {
            spans.push(dot());
            spans.extend(sidebar::pr_badge(pr, theme));
            let state = match pr.state {
                PrState::Open => "open",
                PrState::Draft => "draft",
                PrState::Merged => "merged",
                PrState::Closed => "closed",
            };
            spans.push(Span::styled(
                format!(" {state}"),
                theme.style(sidebar::pr_color(pr.state, theme)),
            ));
            if let Some(checks) = pr.checks {
                let words = match checks {
                    Checks::Passing => "checks passing",
                    Checks::Failing => "checks failing",
                    Checks::Pending => "checks running",
                };
                spans.push(dot());
                let color = sidebar::checks_color(checks, theme);
                spans.extend(label::badge(
                    sidebar::checks_icon(checks),
                    words,
                    color,
                    theme,
                ));
            }
            if let Some(review) = pr.review {
                let (words, color) = match review {
                    Review::Approved => ("approved", theme.ok),
                    Review::ChangesRequested => ("changes requested", theme.error),
                    Review::Required => ("review required", theme.dim),
                };
                spans.extend([dot(), Span::styled(words, theme.style(color))]);
            }
        }
    }
    spans.push(Span::raw(" "));
    Line::from(spans)
}

fn missing_repo_at_cursor(app: &App) -> Option<&str> {
    match app.entry_at_cursor() {
        Some(Entry::Repo(r)) if app.focus == Focus::Sidebar && app.repos[r].is_missing() => {
            Some(&app.repos[r].name)
        }
        _ => None,
    }
}

/// "Worktrees" and "Code": bold, in the accent colour on the side with focus.
fn heading(focused: bool, theme: &Theme) -> Style {
    if focused {
        theme.header(true)
    } else {
        theme.bold()
    }
}

fn sidebar(buf: &mut Buffer, g: &Geometry, app: &App, theme: &Theme) {
    let focused = app.focus == Focus::Sidebar || app.current_worktree().is_none();
    buf.set_string(
        g.left + 3,
        g.tab_row(),
        "Worktrees",
        heading(focused, theme),
    );
    let (first, last) = g.sidebar_rows();
    let height = (last - first + 1) as usize;
    let note = match app.connection {
        Connection::Starting => Some("Loading…"),
        Connection::StartFailed | Connection::Incompatible { .. } => Some(""),
        _ if app.repos.is_empty() => Some("No repositories yet."),
        _ => None,
    };
    if let Some(note) = note {
        buf.set_string(g.left + 3, first, note, theme.dim());
        return;
    }
    let options = sidebar::Options {
        style: app.sidebar_style,
        repo_gap: app.repo_gap,
        worktree_gap: app.worktree_gap,
        width: g.cols.sidebar,
    };
    let rows = sidebar::rows(&app.repos, app.current.as_deref(), &options, theme);
    // Scroll just far enough to keep the cursor's last line on screen.
    let cursor_last = rows
        .iter()
        .rposition(|r| r.entry == Some(app.sidebar_cursor))
        .unwrap_or(0);
    let offset = (cursor_last + 1).saturating_sub(height);
    for (i, row) in rows.iter().skip(offset).take(height).enumerate() {
        let y = first + i as u16;
        buf.set_line(g.left + 1, y, &row.line, g.cols.sidebar);
        if focused && row.entry == Some(app.sidebar_cursor) {
            let line = Rect::new(g.left + 1, y, g.cols.sidebar, 1);
            buf.set_style(line, Style::new().bg(theme.selection));
        }
    }
}

/// How a notice line is styled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Role {
    Text,
    Dim,
    Bold,
    Accent,
    Error,
    Warning,
}

impl Role {
    fn style(self, theme: &Theme) -> Style {
        match self {
            Role::Text => theme.text(),
            Role::Dim => theme.dim(),
            Role::Bold => theme.bold(),
            Role::Accent => theme.accent(),
            Role::Error => theme.style(theme.error),
            Role::Warning => theme.style(theme.warning),
        }
    }
}

/// A centred notice line: an optional icon, the text, and how to style it.
struct NoticeLine {
    icon: Option<&'static str>,
    text: String,
    role: Role,
}

fn line(text: impl Into<String>, role: Role) -> NoticeLine {
    NoticeLine {
        icon: None,
        text: text.into(),
        role,
    }
}

fn headline(icon: &'static str, text: impl Into<String>, role: Role) -> NoticeLine {
    NoticeLine {
        icon: Some(icon),
        text: text.into(),
        role,
    }
}

/// A message that replaces the explorer and viewer, as centred lines.
fn notice(app: &App) -> Option<Vec<NoticeLine>> {
    let blank = || line("", Role::Text);
    let lines = match &app.connection {
        Connection::Starting => vec![],
        Connection::StartFailed => vec![
            headline(
                icons::FAILURE,
                "abcd did not start within 3 seconds",
                Role::Error,
            ),
            blank(),
            line("Run  abc logs  in a shell to see why.", Role::Text),
            blank(),
            line(
                keys_hint(app, &[(Action::Retry, "retry"), (Action::Quit, "quit")]),
                Role::Dim,
            ),
        ],
        Connection::Incompatible { ours, theirs } => vec![
            headline(
                icons::WARNING,
                "This abc is too old for the running abcd",
                Role::Warning,
            ),
            blank(),
            line(
                format!("abc speaks protocol {ours}, abcd speaks protocol {theirs}."),
                Role::Text,
            ),
            line("Quit and reopen abc to use the new build.", Role::Text),
            blank(),
            line(keys_hint(app, &[(Action::Quit, "quit")]), Role::Dim),
        ],
        _ if app.repos.is_empty() => vec![
            line("No repositories yet", Role::Bold),
            blank(),
            line(
                format!(
                    "Press {} to add one, or run",
                    key_label(app, Context::Global, Action::AddRepo)
                ),
                Role::Text,
            ),
            line("abc repo add <path>", Role::Accent),
            line("in a shell.", Role::Text),
        ],
        _ => match missing_repo_at_cursor(app) {
            Some(name) => {
                let repo = app.repos.iter().find(|r| r.name == name)?;
                let remove = key_label(app, Context::Global, Action::RemoveRepo);
                vec![
                    headline(icons::FAILURE, format!("{name} is missing"), Role::Error),
                    blank(),
                    line(
                        format!("{} no longer exists.", repo.path.display()),
                        Role::Text,
                    ),
                    blank(),
                    line(format!("{remove} removes it from abc."), Role::Dim),
                ]
            }
            None if app.current_worktree().is_none() => {
                vec![line(
                    "Pick a worktree in the sidebar and press enter.",
                    Role::Dim,
                )]
            }
            None => return None,
        },
    };
    Some(lines)
}

fn centred(buf: &mut Buffer, g: &Geometry, lines: &[NoticeLine], theme: &Theme) {
    let (y0, y1) = g.content_rows();
    let height = y1 - y0 + 1;
    let width = g.right - g.d1 - 1;
    let top = y0 + height.saturating_sub(lines.len() as u16) / 2;
    for (i, l) in lines.iter().enumerate() {
        let style = l.role.style(theme);
        let spans = match l.icon {
            Some(icon) => label::labelled(icon, style, l.text.as_str(), style, theme),
            None => vec![Span::styled(l.text.as_str(), style)],
        };
        let line = Line::from(spans);
        let x = g.d1 + 1 + width.saturating_sub(line.width() as u16) / 2;
        buf.set_line(x, top + i as u16, &line, width);
    }
}

/// The explorer and the viewer, with the viewer's header and line range.
fn panes(buf: &mut Buffer, g: &Geometry, app: &App, theme: &Theme) {
    let (y0, y1) = g.content_rows();
    let height = y1 - y0 + 1;
    let explorer_area = Rect::new(g.d1 + 1, y0, g.cols.explorer, height);
    explorer::draw(
        buf,
        explorer_area,
        app.current_listing(),
        app.focus == Focus::Explorer,
        theme,
    );
    let viewer = app.current_viewer();
    let viewer_area = Rect::new(g.d2 + 1, y0, g.cols.viewer, height);
    let focused = app.focus == Focus::Viewer;
    // While `/` is typed, the matches of what's typed so far; after, the
    // last search's, until they're hidden.
    let typing = app
        .prompt
        .as_ref()
        .filter(|p| p.kind == PromptKind::Search)
        .map(|p| crate::search::Search::new(p.input.value(), true));
    let search = typing
        .as_ref()
        .or(app.search.as_ref().filter(|_| app.highlight));
    viewer::draw(buf, viewer_area, viewer, focused, search, theme);
    if let Some(v) = viewer {
        let width = g.cols.viewer.saturating_sub(3);
        let header = viewer::header(v, width as usize, focused, theme);
        buf.set_line(g.d2 + 2, g.header_row(), &header, width);
        if let Some(range) = viewer::range(v) {
            // The cursor's place shows only while it does.
            let range = match viewer::position(v).filter(|_| focused) {
                Some(position) => format!(" {position} · {range} "),
                None => format!(" {range} "),
            };
            buf.set_string(
                g.right - 1 - range.chars().count() as u16,
                g.bottom - 2,
                range,
                theme.dim(),
            );
        }
    }
}

fn fade(buf: &mut Buffer, area: Rect, theme: &Theme) {
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            buf[(x, y)].set_style(
                Style::new()
                    .fg(theme.faded)
                    .bg(ratatui::style::Color::Reset)
                    .remove_modifier(Modifier::all()),
            );
        }
    }
}

fn key_label(app: &App, context: Context, action: Action) -> String {
    app.keymap
        .keys_for(context, action)
        .map(keys::short)
        .unwrap_or_default()
}

fn keys_hint(app: &App, items: &[(Action, &str)]) -> String {
    items
        .iter()
        .map(|(a, label)| format!("{} {label}", key_label(app, Context::Global, *a)))
        .collect::<Vec<_>>()
        .join(" · ")
}

/// Key bar entries: the first keys of one or two actions ("j k"), and a label.
fn keybar_items(app: &App) -> Vec<(String, &'static str)> {
    use Action::*;
    let ctx = app.context();
    let one = |a: Action| key_label(app, ctx, a);
    let global = |a: Action| key_label(app, Context::Global, a);
    let two = |a: Action, b: Action| format!("{} {}", one(a), one(b));
    // The leader is the first key of the leader sequences, such as `space f`.
    let leader = app
        .keymap
        .keys_for(Context::Global, FindFile)
        .filter(|k| k.0.len() > 1)
        .map(|k| keys::short(&keys::Keys(vec![k.0[0]])))
        .unwrap_or_default();
    if let Some(prompt) = &app.prompt {
        let run = match prompt.kind {
            PromptKind::Search => "find",
            PromptKind::Line => "go",
        };
        return vec![("enter".into(), run), ("esc".into(), "cancel")];
    }
    match app.connection {
        Connection::Starting => return vec![(global(Help), "help"), (global(Quit), "quit")],
        Connection::StartFailed => return vec![(global(Retry), "retry"), (global(Quit), "quit")],
        Connection::Incompatible { .. } => return vec![(global(Quit), "quit")],
        Connection::Connected | Connection::Lost => {}
    }
    if app.repos.is_empty() {
        return vec![
            (global(AddRepo), "add repo"),
            (global(Help), "help"),
            (global(Quit), "quit"),
        ];
    }
    let mut items = match ctx {
        Context::Sidebar | Context::Global => vec![
            (two(Down, Up), "move"),
            (one(Open), "open"),
            (global(FocusRight), "explorer"),
        ],
        Context::Explorer => vec![
            (two(Down, Up), "move"),
            (one(Open), "open"),
            (two(First, Last), "top/end"),
            (
                format!("{} {}", global(FocusLeft), global(FocusRight)),
                "focus",
            ),
        ],
        // Loading, binary, too large or unreadable: nothing to move through.
        Context::Viewer if !app.current_viewer().is_some_and(|v| v.has_text()) => {
            vec![(one(FocusLeft), "explorer")]
        }
        Context::Viewer => vec![
            (
                format!("{} {} {} {}", one(Left), one(Down), one(Up), one(Right)),
                "move",
            ),
            (
                format!(
                    "{} {} {}",
                    one(WordForward),
                    one(WordBackward),
                    one(WordEnd)
                ),
                "words",
            ),
            (two(Search, NextMatch), "find"),
            (two(VisualChars, VisualLines), "select"),
            (one(GoToLine), "line"),
            (one(FocusLeft), "explorer"),
        ],
        Context::Visual => vec![
            (
                format!("{} {} {} {}", one(Left), one(Down), one(Up), one(Right)),
                "move",
            ),
            (
                format!(
                    "{} {} {}",
                    one(WordForward),
                    one(WordBackward),
                    one(WordEnd)
                ),
                "words",
            ),
            (two(Copy, CopyLine), "copy"),
            (one(CopyReference), "reference"),
            (one(OtherEnd), "other end"),
            (one(ExitVisual), "cancel"),
        ],
    };
    if ctx != Context::Visual {
        items.push((leader, "commands"));
    }
    if !matches!(ctx, Context::Viewer | Context::Visual) {
        items.extend([
            (global(Switcher), "find"),
            (global(LastWorktree), "last worktree"),
        ]);
    }
    items.extend([(global(Help), "help"), (global(Quit), "quit")]);
    items
}

fn keybar(buf: &mut Buffer, g: &Geometry, app: &App, theme: &Theme) {
    let mut spans = Vec::new();
    for (i, (keys, label)) in keybar_items(app).into_iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" · ", theme.dim()));
        }
        spans.push(Span::styled(keys, theme.bold()));
        spans.push(Span::styled(format!(" {label}"), theme.text()));
    }
    buf.set_line(
        g.left + 2,
        g.bottom - 1,
        &Line::from(spans),
        g.right - g.left - 3,
    );
}

/// The status line's message: an icon by kind, then the text, in one colour.
fn status_message(app: &App, theme: &Theme) -> Option<(Option<&'static str>, String, Color)> {
    let typed = typed(app);
    if !typed.is_empty() {
        return Some((None, format!("{typed} …"), theme.text));
    }
    if let Some(mode) = visual_mode(app) {
        return Some((None, mode, theme.accent));
    }
    let message = |icon, text: &str, color| Some((Some(icon), text.to_string(), color));
    match &app.connection {
        Connection::Starting => message(icons::SYNC, "starting abcd…", theme.accent),
        Connection::StartFailed => message(icons::FAILURE, "abcd did not start", theme.error),
        Connection::Lost => message(
            icons::DISCONNECTED,
            "lost abcd · reconnecting…",
            theme.error,
        ),
        Connection::Incompatible { .. } => {
            message(icons::WARNING, "protocol mismatch", theme.warning)
        }
        Connection::Connected => match &app.status {
            Some(s) => {
                let (icon, color) = match s.kind {
                    StatusKind::Plain => return Some((None, s.text.clone(), theme.text)),
                    StatusKind::Info => (icons::INFO, theme.accent),
                    StatusKind::Ok => (icons::SUCCESS, theme.ok),
                    StatusKind::Warning => (icons::WARNING, theme.warning),
                    StatusKind::Error => (icons::FAILURE, theme.error),
                };
                message(icon, &s.text, color)
            }
            None if app.stale_client => message(
                icons::WARNING,
                "this abc is older than the running abcd · quit and reopen abc",
                theme.warning,
            ),
            None => None,
        },
    }
}

/// Visual mode, as Vim names it, and how much is selected: lines, or
/// characters within one line.
fn visual_mode(app: &App) -> Option<String> {
    if app.context() != Context::Visual {
        return None;
    }
    let (from, to, by_line) = app.current_viewer()?.selected()?;
    let size = if by_line || from.line != to.line {
        crate::app::plural(to.line - from.line + 1, "line")
    } else {
        crate::app::plural(to.col - from.col + 1, "character")
    };
    let mode = if by_line { "VISUAL LINE" } else { "VISUAL" };
    Some(format!("-- {mode} -- · {size}"))
}

/// What's typed towards a command that hasn't run yet, as Vim's showcmd
/// shows it: a count, the start of a sequence, an `f` waiting for its character.
fn typed(app: &App) -> String {
    let mut parts = Vec::new();
    if let Some(count) = app.count {
        parts.push(count.to_string());
    }
    if !app.pending.is_empty() {
        parts.push(keys::Keys(app.pending.clone()).to_string());
    }
    if let Some(action) = app.finding {
        parts.push(key_label(app, Context::Viewer, action));
    }
    parts.join(" ")
}

fn status_line(buf: &mut Buffer, g: &Geometry, app: &App, theme: &Theme) {
    let counts = matches!(app.connection, Connection::Connected | Connection::Lost).then(|| {
        let worktrees: usize = app.repos.iter().map(|r| r.worktrees().len()).sum();
        let plural = |n: usize, word: &str| format!("{n} {word}{}", if n == 1 { "" } else { "s" });
        if app.repos.is_empty() {
            plural(0, "repo")
        } else {
            format!(
                "{} · {}",
                plural(app.repos.len(), "repo"),
                plural(worktrees, "worktree")
            )
        }
    });
    let mut right_edge = g.right;
    if let Some(counts) = counts {
        let counts = format!(" {counts} ");
        right_edge = g.right - counts.chars().count() as u16;
        buf.set_string(right_edge, g.bottom, counts, theme.dim());
    }
    let room = right_edge.saturating_sub(g.left + 1);
    if let Some(prompt) = &app.prompt {
        return prompt_line(buf, (g.left + 1, g.bottom), room as usize, prompt, theme);
    }
    if let Some((icon, text, color)) = status_message(app, theme) {
        let style = theme.style(color);
        let mut spans = vec![Span::raw(" ")];
        match icon {
            Some(icon) => spans.extend(label::labelled(icon, style, text, style, theme)),
            None => spans.push(Span::styled(text, style)),
        }
        spans.push(Span::raw(" "));
        buf.set_line(g.left + 1, g.bottom, &Line::from(spans), room);
    }
}

/// The open prompt, as Vim's command line shows it: its sign, what's typed,
/// and a cursor in it, scrolled to keep the cursor in `room`.
fn prompt_line(buf: &mut Buffer, (x, y): (u16, u16), room: usize, prompt: &Prompt, theme: &Theme) {
    let sign = match prompt.kind {
        PromptKind::Search => '/',
        PromptKind::Line => ':',
    };
    // " /", then the text, then the cursor's cell and a blank before the border.
    let scroll = prompt.input.visual_scroll(room.saturating_sub(4));
    let mut skipped = 0;
    let shown: String = prompt
        .input
        .value()
        .chars()
        .skip_while(|c| {
            let skip = skipped < scroll;
            if skip {
                skipped += crate::viewer::width(&[*c]);
            }
            skip
        })
        .collect();
    buf.set_stringn(x, y, format!(" {sign}{shown}  "), room, theme.text());
    let cursor = 2 + prompt.input.visual_cursor() - scroll;
    if cursor < room {
        buf[(x + cursor as u16, y)].set_style(Style::new().add_modifier(Modifier::REVERSED));
    }
}

/// Trims from the left, for paths: the end is the informative part.
fn ltrunc(s: &str, n: usize) -> String {
    let len = s.chars().count();
    if len <= n {
        s.to_string()
    } else {
        std::iter::once('…')
            .chain(s.chars().skip(len - n + 1))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gallery::{self, Screen};

    fn keybar(screen: Screen, state: &str) -> Vec<String> {
        let preset = gallery::presets(screen)
            .into_iter()
            .find(|p| p.name == state)
            .expect("a gallery state");
        keybar_items(&preset.app)
            .into_iter()
            .map(|(keys, label)| format!("{keys} {label}"))
            .collect()
    }

    #[test]
    fn a_viewer_with_no_text_offers_only_the_way_back() {
        for state in ["binary", "too-large", "loading", "error"] {
            assert_eq!(
                keybar(Screen::Viewer, state),
                ["esc explorer", "space commands", "? help", "q quit"],
                "{state}"
            );
        }
        assert!(keybar(Screen::Viewer, "highlighted").contains(&"/ n find".to_string()));
    }
}
