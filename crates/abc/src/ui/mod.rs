//! Drawing. Everything here is a pure function of the app state, theme and icons.

pub mod sidebar;

use abc_core::model::{Checks, Head, PrState, PullRequest, Review};
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, StatefulWidget};

use crate::app::{App, Connection, Entry, Focus, StatusKind};
use crate::icons::Icons;
use crate::keys::{self, Action, Context};
use crate::layout::{self, Columns};
use crate::theme::Theme;

pub fn draw(frame: &mut Frame, area: Rect, app: &App, theme: &Theme, icons: &Icons) {
    draw_buffer(frame.buffer_mut(), area, app, theme, icons);
}

pub fn draw_buffer(buf: &mut Buffer, area: Rect, app: &App, theme: &Theme, icons: &Icons) {
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
    chrome(buf, &g, app, theme, icons);
    sidebar(buf, &g, app, theme, icons);
    match notice(app) {
        Some(lines) => centred(buf, &g, &lines, theme),
        None => panes(buf, &g, theme),
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

    /// First and last content rows (between the pane headers and the separator).
    fn content_rows(&self) -> (u16, u16) {
        (self.top + 3, self.bottom - 3)
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
fn chrome(buf: &mut Buffer, g: &Geometry, app: &App, theme: &Theme, icons: &Icons) {
    let border = theme.border();
    hline(buf, g.top, g.left, g.right, border);
    hline(buf, g.bottom, g.left, g.right, border);
    vline(buf, g.left, g.top, g.bottom, border);
    vline(buf, g.right, g.top, g.bottom, border);
    put(buf, g.left, g.top, "┌", border);
    put(buf, g.right, g.top, "┐", border);
    put(buf, g.left, g.bottom, "└", border);
    put(buf, g.right, g.bottom, "┘", border);

    // Sidebar divider, tab bar, and the line under it.
    vline(buf, g.d1, g.top + 1, g.bottom - 3, border);
    buf.set_string(
        g.d1 + 2,
        g.top + 1,
        "Code",
        theme.bold().add_modifier(Modifier::UNDERLINED),
    );
    hline(buf, g.top + 2, g.d1 + 1, g.right - 1, border);
    put(buf, g.d1, g.top + 2, "├", border);
    put(buf, g.right, g.top + 2, "┤", border);

    // The separator above the key bar.
    hline(buf, g.bottom - 2, g.left + 1, g.right - 1, border);
    put(buf, g.left, g.bottom - 2, "├", border);
    put(buf, g.right, g.bottom - 2, "┤", border);
    put(buf, g.d1, g.bottom - 2, "┴", border);

    if g.split {
        put(buf, g.d2, g.top + 2, "┬", border);
        vline(buf, g.d2, g.top + 3, g.bottom - 3, border);
        put(buf, g.d2, g.bottom - 2, "┴", border);
        if let Some((_, w)) = app.current_worktree() {
            let room = g.cols.explorer.saturating_sub(3) as usize;
            let path = ltrunc(&w.path.display().to_string(), room);
            buf.set_string(
                g.d1 + 2,
                g.top + 2,
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
    let title = title(app, theme, icons);
    buf.set_line(
        g.left + 1,
        g.top,
        &title,
        profile_x.saturating_sub(g.left + 2),
    );
}

fn title<'a>(app: &'a App, theme: &Theme, icons: &Icons) -> Line<'a> {
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
            if !icons.branch.is_empty() {
                spans.push(Span::styled(format!("{} ", icons.branch), theme.dim()));
            }
            spans.push(Span::styled(branch.as_str(), theme.text()));
        }
        if let Some(pr) = &w.pr {
            spans.push(dot());
            let quiet = PullRequest {
                checks: None,
                ..pr.clone()
            };
            spans.extend(sidebar::pr_badge(Some(&quiet), theme, icons));
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
                let style = theme.style(sidebar::checks_color(checks, theme));
                spans.extend([
                    dot(),
                    Span::styled(
                        format!("{} {words}", sidebar::checks_icon(checks, icons)),
                        style,
                    ),
                ]);
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

fn sidebar(buf: &mut Buffer, g: &Geometry, app: &App, theme: &Theme, icons: &Icons) {
    let focused = app.focus == Focus::Sidebar || app.current_worktree().is_none();
    buf.set_string(
        g.left + 2,
        g.top + 1,
        "WORKTREES",
        theme.header(focused).add_modifier(Modifier::BOLD),
    );
    let area = Rect::new(
        g.left + 1,
        g.top + 2,
        g.cols.sidebar,
        g.bottom - 2 - (g.top + 2),
    );
    let width = g.cols.sidebar;
    let note = |text: &'static str| {
        let line = Line::from(vec![Span::raw(" "), Span::styled(text, theme.dim())]);
        Some(vec![line])
    };
    let message = match app.connection {
        Connection::Starting => note("Loading…"),
        Connection::StartFailed | Connection::Incompatible { .. } => Some(Vec::new()),
        _ if app.repos.is_empty() => note("No repositories yet."),
        _ => None,
    };
    if let Some(lines) = message {
        for (i, line) in lines.iter().enumerate() {
            buf.set_line(area.x, area.y + 1 + i as u16, line, width);
        }
        return;
    }
    let current = app.current.as_deref();
    let items: Vec<ListItem> = app
        .entries()
        .into_iter()
        .map(|entry| match entry {
            Entry::Repo(r) => ListItem::new(sidebar::repo_line(&app.repos[r], width, theme, icons)),
            Entry::Worktree(r, w) => {
                let w = &app.repos[r].worktrees()[w];
                let lines = sidebar::worktree_lines(
                    w,
                    current == Some(w.path.as_path()),
                    app.sidebar_style,
                    width,
                    theme,
                    icons,
                );
                ListItem::new(lines)
            }
        })
        .collect();
    let highlight = if focused {
        Style::new().bg(theme.selection)
    } else {
        Style::new()
    };
    let mut state = ListState::default().with_selected(Some(app.sidebar_cursor));
    StatefulWidget::render(
        List::new(items).highlight_style(highlight),
        area,
        buf,
        &mut state,
    );
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

/// A message that replaces the explorer and viewer, as centred lines.
fn notice(app: &App) -> Option<Vec<(String, Role)>> {
    let blank = || (String::new(), Role::Text);
    let lines = match &app.connection {
        Connection::Starting => vec![],
        Connection::StartFailed => vec![
            ("✗ abcd did not start within 3 seconds".into(), Role::Error),
            blank(),
            ("Run  abc logs  in a shell to see why.".into(), Role::Text),
            blank(),
            (
                keys_hint(app, &[(Action::Retry, "retry"), (Action::Quit, "quit")]),
                Role::Dim,
            ),
        ],
        Connection::Incompatible { ours, theirs } => vec![
            (
                "! This abc is too old for the running abcd".into(),
                Role::Warning,
            ),
            blank(),
            (
                format!("abc speaks protocol {ours}, abcd speaks protocol {theirs}."),
                Role::Text,
            ),
            (
                "Quit and reopen abc to use the new build.".into(),
                Role::Text,
            ),
            blank(),
            (keys_hint(app, &[(Action::Quit, "quit")]), Role::Dim),
        ],
        _ if app.repos.is_empty() => vec![
            ("No repositories yet".into(), Role::Bold),
            blank(),
            (
                format!(
                    "Press {} to add one, or run",
                    key_label(app, Context::Global, Action::AddRepo)
                ),
                Role::Text,
            ),
            ("abc repo add <path>".into(), Role::Accent),
            ("in a shell.".into(), Role::Text),
        ],
        _ => match missing_repo_at_cursor(app) {
            Some(name) => {
                let repo = app.repos.iter().find(|r| r.name == name)?;
                vec![
                    (format!("✗ {name} is missing"), Role::Error),
                    blank(),
                    (
                        format!("{} no longer exists.", repo.path.display()),
                        Role::Text,
                    ),
                    blank(),
                    (
                        format!(
                            "{} removes it from abc.",
                            key_label(app, Context::Global, Action::RemoveRepo)
                        ),
                        Role::Dim,
                    ),
                ]
            }
            None if app.current_worktree().is_none() => {
                vec![(
                    "Pick a worktree in the sidebar and press enter.".into(),
                    Role::Dim,
                )]
            }
            None => return None,
        },
    };
    Some(lines)
}

fn centred(buf: &mut Buffer, g: &Geometry, lines: &[(String, Role)], theme: &Theme) {
    let (y0, y1) = g.content_rows();
    let height = y1 - y0 + 1;
    let width = g.right - g.d1 - 1;
    let top = y0 + height.saturating_sub(lines.len() as u16) / 2;
    for (i, (text, role)) in lines.iter().enumerate() {
        let x = g.d1 + 1 + width.saturating_sub(text.chars().count() as u16) / 2;
        buf.set_string(x, top + i as u16, text, role.style(theme));
    }
}

/// The explorer and viewer. Both arrive with their own screens; until then the
/// panes say so.
fn panes(buf: &mut Buffer, g: &Geometry, theme: &Theme) {
    let (y0, _) = g.content_rows();
    buf.set_string(g.d1 + 2, y0, "The file explorer is the", theme.dim());
    buf.set_string(g.d1 + 2, y0 + 1, "next screen to build.", theme.dim());
    buf.set_string(g.d2 + 2, y0, "No file open.", theme.dim());
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
        Context::Viewer => vec![
            (two(Down, Up), "scroll"),
            (two(HalfPageDown, HalfPageUp), "half page"),
            (two(PageDown, PageUp), "page"),
            (two(First, Last), "top/end"),
            (global(FocusLeft), "explorer"),
        ],
    };
    items.push((leader, "commands"));
    if ctx != Context::Viewer {
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

fn status_line(buf: &mut Buffer, g: &Geometry, app: &App, theme: &Theme) {
    let (text, style) = if !app.pending.is_empty() {
        (
            format!("{} …", keys::Keys(app.pending.clone())),
            theme.text(),
        )
    } else {
        match &app.connection {
            Connection::Starting => ("◌ starting abcd…".to_string(), theme.accent()),
            Connection::StartFailed => {
                ("✗ abcd did not start".to_string(), theme.style(theme.error))
            }
            Connection::Lost => (
                "✗ lost abcd · reconnecting…".to_string(),
                theme.style(theme.error),
            ),
            Connection::Incompatible { .. } => (
                "! protocol mismatch".to_string(),
                theme.style(theme.warning),
            ),
            Connection::Connected if app.stale_client && app.status.is_none() => (
                "! this abc is older than the running abcd · quit and reopen abc".to_string(),
                theme.style(theme.warning),
            ),
            Connection::Connected => match &app.status {
                Some(s) => {
                    let color = match s.kind {
                        StatusKind::Info => theme.accent,
                        StatusKind::Ok => theme.ok,
                        StatusKind::Warning => theme.warning,
                        StatusKind::Error => theme.error,
                    };
                    (s.text.clone(), theme.style(color))
                }
                None => (String::new(), theme.text()),
            },
        }
    };
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
    if !text.is_empty() {
        let room = right_edge.saturating_sub(g.left + 2) as usize;
        buf.set_string(
            g.left + 1,
            g.bottom,
            format!(" {} ", trunc(&text, room.saturating_sub(2))),
            style,
        );
    }
}

fn trunc(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        s.chars().take(n.saturating_sub(1)).chain(['…']).collect()
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
