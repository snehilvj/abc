//! `abc gallery icons`: every icon abc uses, with what it means, drawn with a
//! 1-cell and a 2-cell gap side by side, plus real sidebar rows in each. It is
//! the reference for the icon system and the way to choose `icon_gap` for a
//! terminal.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};
use ratatui::text::{Line, Span};

use super::label::{badge, labelled};
use super::sidebar;
use crate::app::SidebarStyle;
use crate::fake;
use crate::icons;
use crate::theme::Theme;

/// What each icon means, as its label in the catalogue, and its colour role.
fn vocabulary(theme: &Theme) -> Vec<(&'static str, &'static str, Option<Color>)> {
    vec![
        (icons::REPO, "repository", None),
        (icons::HOME, "main checkout", None),
        (icons::CURRENT, "current worktree", None),
        (icons::BRANCH, "branch", None),
        (icons::COMMIT, "detached HEAD", None),
        (icons::PR_OPEN, "#142 open", Some(theme.pr_open)),
        (icons::PR_DRAFT, "#88 draft", Some(theme.pr_draft)),
        (icons::PR_MERGED, "#141 merged", Some(theme.pr_merged)),
        (icons::PR_CLOSED, "#97 closed", Some(theme.pr_closed)),
        (icons::CHECKS_PASSING, "checks passing", Some(theme.ok)),
        (icons::CHECKS_FAILING, "checks failing", Some(theme.error)),
        (icons::CHECKS_PENDING, "checks running", Some(theme.warning)),
        (icons::CHANGED, "3 changed files", Some(theme.git_changed)),
        (icons::AHEAD, "2 ahead", Some(theme.git_ahead)),
        (icons::BEHIND, "1 behind", Some(theme.git_behind)),
        (icons::LOCK, "locked", None),
        (icons::WARNING, "missing", Some(theme.warning)),
        (icons::SUCCESS, "added privue-agents", Some(theme.ok)),
        (icons::FAILURE, "abcd did not start", Some(theme.error)),
        (icons::INFO, "abcd restarted", Some(theme.accent)),
        (icons::SYNC, "starting abcd…", Some(theme.accent)),
        (icons::DISCONNECTED, "lost abcd", Some(theme.error)),
    ]
}

pub fn draw(buf: &mut Buffer, area: Rect, theme: &Theme) {
    let half = area.width / 2;
    let top = area.y + 1;
    buf.set_string(
        area.x + 2,
        top,
        "ICONS AND LABELS",
        theme.accent().add_modifier(Modifier::BOLD),
    );
    buf.set_string(
        area.x + 2,
        top + 1,
        "Pick the column where every icon sits clear of its label, then press q.",
        theme.dim(),
    );
    for (i, (gap, name)) in [
        (1, "A  one cell after each icon"),
        (2, "B  two cells after each icon"),
    ]
    .into_iter()
    .enumerate()
    {
        let theme = Theme {
            icon_gap: gap,
            ..theme.clone()
        };
        let x = area.x + 2 + i as u16 * half;
        column(
            buf,
            Rect::new(
                x,
                top + 3,
                half.saturating_sub(4),
                area.height.saturating_sub(5),
            ),
            name,
            &theme,
        );
    }
}

fn column(buf: &mut Buffer, area: Rect, name: &str, theme: &Theme) {
    let mut y = area.y;
    buf.set_string(area.x, y, name, theme.bold());
    y += 2;
    let repos = fake::repos_with_variants();
    let options = sidebar::Options {
        style: SidebarStyle::Detailed,
        repo_gap: 1,
        worktree_gap: 0,
        width: 38,
    };
    for row in sidebar::rows(&repos[..1], Some(&fake::current()), &options, theme)
        .iter()
        .take(9)
    {
        buf.set_line(area.x.saturating_sub(1), y, &row.line, 38);
        y += 1;
    }
    y += 1;
    for (icon, meaning, color) in vocabulary(theme) {
        if y >= area.bottom() {
            break;
        }
        let spans = match color {
            Some(color) => badge(icon, meaning, color, theme),
            None => labelled(icon, theme.accent(), meaning, theme.text(), theme),
        };
        let mut line = vec![Span::raw("  ")];
        line.extend(spans);
        buf.set_line(area.x, y, &Line::from(line), area.width);
        y += 1;
    }
}
