//! Drawing the switcher: the mode tabs, the query and its count, then a
//! fixed number of results, in a dialog over the faded screen. Worktrees use
//! the sidebar's icons and badge columns; matched characters are bold in the
//! accent colour.

use abc_core::model::{Head, Repo, Worktree};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use super::dialog::{self, Dialog, Row};
use super::label::{self, BADGE_GAP, blank, labelled, marker, no_icon, pad};
use super::sidebar::{self, Columns};
use crate::app::{App, head_name};
use crate::icons;
use crate::switcher::{Mode, Source, Switcher, Target, ago};
use crate::theme::Theme;

/// The switcher's width, and how far down the screen it sits.
const WIDTH: u16 = 96;
const TOP: u16 = 3;

pub fn draw(buf: &mut Buffer, area: Rect, app: &App, switcher: &Switcher, theme: &Theme) {
    let width = WIDTH.min(area.width.saturating_sub(4));
    let x = area.x + (area.width - width) / 2;
    let inner = width.saturating_sub(4) as usize;
    let mut body = message(switcher, theme).map_or_else(Vec::new, |m| vec![Row::new(m)]);
    let shown = switcher
        .hits
        .iter()
        .enumerate()
        .skip(switcher.offset)
        .take(switcher.rows);
    match switcher.mode {
        Mode::Worktrees => {
            let layout = WorktreeLayout::new(app, switcher, inner, theme);
            for (i, hit) in shown {
                let Target::Worktree(path) = &switcher.candidates()[hit.candidate].target else {
                    continue;
                };
                if let Some((repo, w)) = app.find(path) {
                    let row = layout.row(app, repo, w, &hit.matched, theme);
                    body.push(row.selected(i == switcher.selected));
                }
            }
        }
        Mode::Files => {
            for (i, hit) in shown {
                let path = &switcher.candidates()[hit.candidate].text;
                let (icon, color) = icons::file(path, theme);
                let mut left = vec![no_icon(theme)];
                left.push(Span::styled(icon.to_string(), theme.style(color)));
                left.push(blank(theme.icon_gap));
                left.extend(matched(
                    path,
                    0,
                    &hit.matched,
                    usize::MAX,
                    theme.text(),
                    theme,
                ));
                body.push(Row::new(left).selected(i == switcher.selected));
            }
        }
    }
    // The box keeps its size as results come and go.
    while body.len() < switcher.rows {
        body.push(Row::default());
    }
    let run = match switcher.mode {
        Mode::Worktrees => "switch",
        Mode::Files => "open",
    };
    let footer = vec![
        ("enter".into(), run),
        ("↑↓".into(), "move"),
        ("ctrl-u".into(), "clear"),
        ("esc".into(), "close"),
    ];
    let header = vec![tabs(switcher.mode, theme), query(switcher, inner, theme)];
    let dialog = Dialog {
        title: "Switch",
        header,
        body,
        footer,
    };
    dialog::draw(buf, (x, area.y + TOP), width, dialog, theme);
}

fn tabs<'a>(mode: Mode, theme: &Theme) -> Row<'a> {
    let tab = |name: &'a str, on: bool| {
        let style = if on {
            theme
                .accent()
                .add_modifier(Modifier::BOLD)
                .bg(theme.selection)
        } else {
            theme.dim()
        };
        Span::styled(name, style)
    };
    Row::new(vec![
        tab(" Worktrees ", mode == Mode::Worktrees),
        blank(2),
        tab(" Files ", mode == Mode::Files),
    ])
    .right(Span::styled("tab to change", theme.dim()))
}

/// "> query" with its cursor, and on the right how many match.
fn query<'a>(switcher: &'a Switcher, inner: usize, theme: &Theme) -> Row<'a> {
    let total = switcher.candidates().len();
    let count = match &switcher.source {
        Source::Ready(_) if switcher.input.value().is_empty() => total.to_string(),
        Source::Ready(_) => format!("{} of {total}", switcher.hits.len()),
        Source::Loading => "loading…".into(),
        Source::Failed(_) | Source::NoWorktree => String::new(),
    };
    dialog::query(&switcher.input, count, inner, theme)
}

/// What to say instead of, or before, the results.
fn message<'a>(switcher: &Switcher, theme: &Theme) -> Option<Line<'a>> {
    let query = switcher.input.value();
    let (what, plural) = match switcher.mode {
        Mode::Worktrees => ("worktrees", "worktrees"),
        Mode::Files => ("files", "files"),
    };
    let line = match &switcher.source {
        Source::Loading => labelled(
            icons::SYNC,
            theme.dim(),
            "Loading files…",
            theme.dim(),
            theme,
        ),
        Source::Failed(why) => label::badge(
            icons::FAILURE,
            format!("Can't list files: {why}"),
            theme.error,
            theme,
        ),
        Source::NoWorktree => vec![Span::styled(
            "Open a worktree to find its files.",
            theme.dim(),
        )],
        Source::Ready(c) if c.is_empty() => {
            vec![Span::styled(format!("No {plural} yet."), theme.dim())]
        }
        Source::Ready(_) if switcher.hits.is_empty() => vec![Span::styled(
            format!("No {what} match \"{query}\"."),
            theme.dim(),
        )],
        Source::Ready(_) => return None,
    };
    Some(Line::from(line))
}

/// `text` from its character `start` in the matched text, the matched ones
/// bold in the accent colour, cut with "…" to `width` cells and padded to it.
fn matched<'a>(
    text: &str,
    start: usize,
    matched: &[u32],
    width: usize,
    style: Style,
    theme: &Theme,
) -> Vec<Span<'a>> {
    let hit = theme.accent().add_modifier(Modifier::BOLD);
    let chars: Vec<char> = text.chars().collect();
    let cut = chars.len() > width;
    let keep = if cut {
        width.saturating_sub(1)
    } else {
        chars.len()
    };
    let mut spans: Vec<Span> = Vec::new();
    for (i, c) in chars.iter().take(keep).enumerate() {
        let s = if matched.binary_search(&((start + i) as u32)).is_ok() {
            hit
        } else {
            style
        };
        match spans.last_mut() {
            Some(last) if last.style == s => last.content.to_mut().push(*c),
            _ => spans.push(Span::styled(c.to_string(), s)),
        }
    }
    if cut {
        spans.push(Span::styled("…", theme.dim()));
    }
    if width == usize::MAX {
        spans
    } else {
        pad(spans, width)
    }
}

/// Column widths for worktree rows, measured over every worktree listed; the
/// branch gives way when the row would be too wide.
struct WorktreeLayout {
    repo: usize,
    name: usize,
    branch: usize,
    columns: Columns,
}

impl WorktreeLayout {
    fn new(app: &App, switcher: &Switcher, inner: usize, theme: &Theme) -> Self {
        let listed: Vec<(&Repo, &Worktree)> = switcher
            .candidates()
            .iter()
            .filter_map(|c| match &c.target {
                Target::Worktree(path) => app.find(path),
                Target::File(_) => None,
            })
            .collect();
        let widest = |f: &dyn Fn(&Repo, &Worktree) -> usize| {
            listed.iter().map(|(r, w)| f(r, w)).max().unwrap_or(0)
        };
        let repo = widest(&|r, _| r.name.chars().count());
        let name = widest(&|_, w| w.name.chars().count());
        let branch = widest(&|_, w| head_name(&w.head).chars().count());
        let when = widest(&|_, w| ago(w.last_opened, app.now).chars().count());
        let columns = Columns::measure(&app.repos, theme);
        let icon = label::icon_width(theme);
        // Badge columns are padded, so any one row measures them all.
        let badges = listed.first().map_or(0, |(_, w)| {
            label::width(&sidebar::status_cells(w.git.as_ref(), &columns, theme))
                + BADGE_GAP
                + label::width(&sidebar::pr_cells(w, &columns, theme))
        });
        // Current marker, repo, name, branch icon, then a blank before the badges and the time.
        let fixed = icon + (repo + 1) + (name + 1) + icon + 1 + badges + 1 + when;
        Self {
            repo,
            name,
            branch: branch.min(inner.saturating_sub(fixed)),
            columns,
        }
    }

    fn row<'a>(
        &self,
        app: &App,
        repo: &Repo,
        w: &Worktree,
        matched_at: &[u32],
        theme: &Theme,
    ) -> Row<'a> {
        let current = app.current.as_deref() == Some(w.path.as_path());
        let mut left = if current {
            marker(icons::CURRENT, theme.accent(), theme)
        } else {
            vec![no_icon(theme)]
        };
        let name_style = if current { theme.bold() } else { theme.text() };
        // The matched text is "repo name branch".
        let repo_len = repo.name.chars().count();
        let name_at = repo_len + 1;
        let branch_at = name_at + w.name.chars().count() + 1;
        left.extend(matched(
            &repo.name,
            0,
            matched_at,
            self.repo,
            theme.dim(),
            theme,
        ));
        left.push(blank(1));
        left.extend(matched(
            &w.name, name_at, matched_at, self.name, name_style, theme,
        ));
        left.push(blank(1));
        let icon = match w.head {
            Head::Branch(_) => icons::BRANCH,
            Head::Detached(_) => icons::COMMIT,
        };
        left.extend(marker(icon, theme.dim(), theme));
        let branch = head_name(&w.head);
        left.extend(matched(
            branch,
            branch_at,
            matched_at,
            self.branch,
            theme.text(),
            theme,
        ));
        left.push(blank(1));
        left.extend(sidebar::status_cells(w.git.as_ref(), &self.columns, theme));
        left.push(blank(BADGE_GAP));
        left.extend(sidebar::pr_cells(w, &self.columns, theme));
        Row::new(left).right(Span::styled(ago(w.last_opened, app.now), theme.dim()))
    }
}
