//! Sidebar rows: a line per repo, then two lines per worktree (or one, compact).

use abc_core::model::{Checks, GitStatus, Head, PrState, PullRequest, Repo, Worktree};
use ratatui::style::{Color, Modifier};
use ratatui::text::{Line, Span};

use crate::app::SidebarStyle;
use crate::icons::Icons;
use crate::theme::Theme;

pub fn repo_line<'a>(repo: &'a Repo, width: u16, theme: &Theme, icons: &Icons) -> Line<'a> {
    let mut left = vec![Span::raw(" ")];
    let name_style = if repo.is_missing() {
        theme.dim()
    } else {
        theme.bold()
    };
    if !icons.repo.is_empty() {
        let style = if repo.is_missing() {
            theme.dim()
        } else {
            theme.accent()
        };
        left.push(Span::styled(format!("{} ", icons.repo), style));
    }
    let right = if repo.is_missing() {
        missing(theme, icons)
    } else {
        Vec::new()
    };
    left.push(Span::styled(repo.name.as_str(), name_style));
    fit(left, right, width)
}

pub fn worktree_lines<'a>(
    w: &'a Worktree,
    current: bool,
    style: SidebarStyle,
    width: u16,
    theme: &Theme,
    icons: &Icons,
) -> Vec<Line<'a>> {
    let mut left = vec![
        Span::raw(" "),
        Span::styled(if current { "▸ " } else { "  " }, theme.accent()),
    ];
    if w.is_main && !icons.home.is_empty() {
        left.push(Span::styled(format!("{} ", icons.home), theme.dim()));
    }
    let name_style = if current {
        theme.accent().add_modifier(Modifier::BOLD)
    } else if w.missing {
        theme.dim()
    } else {
        theme.text()
    };
    left.push(Span::styled(w.name.as_str(), name_style));

    let mut right = if w.missing {
        missing(theme, icons)
    } else {
        pr_badge(w.pr.as_ref(), theme, icons)
    };
    if w.locked && !w.missing {
        let mut locked = vec![Span::styled(icons.lock, theme.dim())];
        if !right.is_empty() {
            locked.push(Span::raw("  "));
        }
        locked.append(&mut right);
        right = locked;
    }

    if style == SidebarStyle::Compact {
        if !w.missing {
            left.extend(git_spans(w.git.as_ref(), theme, icons));
        }
        return vec![fit(left, right, width)];
    }

    let mut second = vec![Span::raw("     ")];
    match &w.head {
        Head::Detached(sha) => {
            if !icons.commit.is_empty() {
                second.push(Span::styled(format!("{} ", icons.commit), theme.dim()));
            }
            second.push(Span::styled(format!("{sha} detached"), theme.dim()));
        }
        Head::Branch(branch) => {
            if !icons.branch.is_empty() {
                second.push(Span::styled(format!("{} ", icons.branch), theme.dim()));
            }
            second.push(Span::styled(branch.as_str(), theme.dim()));
            if !w.missing {
                second.extend(git_spans(w.git.as_ref(), theme, icons));
            }
        }
    }
    vec![fit(left, right, width), fit(second, Vec::new(), width)]
}

/// Changed files, ahead, behind, or `local` without an upstream.
pub fn git_spans<'a>(git: Option<&GitStatus>, theme: &Theme, icons: &Icons) -> Vec<Span<'a>> {
    let Some(git) = git else { return Vec::new() };
    let parts = [
        (git.changed, icons.changed, theme.git_changed),
        (git.ahead, icons.ahead, theme.git_ahead),
        (git.behind, icons.behind, theme.git_behind),
    ];
    let mut spans = Vec::new();
    for (count, icon, color) in parts {
        if count > 0 {
            spans.push(Span::raw(if spans.is_empty() { "  " } else { " " }));
            spans.push(Span::styled(format!("{icon}{count}"), theme.style(color)));
        }
    }
    if !git.upstream {
        spans.push(Span::raw("  "));
        spans.push(Span::styled("local", theme.dim()));
    }
    spans
}

pub fn pr_color(state: PrState, theme: &Theme) -> Color {
    match state {
        PrState::Open => theme.pr_open,
        PrState::Draft => theme.pr_draft,
        PrState::Merged => theme.pr_merged,
        PrState::Closed => theme.pr_closed,
    }
}

pub fn checks_color(checks: Checks, theme: &Theme) -> Color {
    match checks {
        Checks::Passing => theme.ok,
        Checks::Failing => theme.error,
        Checks::Pending => theme.warning,
    }
}

pub fn checks_icon(checks: Checks, icons: &Icons) -> &'static str {
    match checks {
        Checks::Passing => icons.checks_passing,
        Checks::Failing => icons.checks_failing,
        Checks::Pending => icons.checks_pending,
    }
}

/// PR icon and number in the state's colour, then the checks glyph while it's open.
pub fn pr_badge<'a>(pr: Option<&PullRequest>, theme: &Theme, icons: &Icons) -> Vec<Span<'a>> {
    let Some(pr) = pr else { return Vec::new() };
    let style = theme.style(pr_color(pr.state, theme));
    let icon = match pr.state {
        PrState::Open => icons.pr_open,
        PrState::Draft => icons.pr_draft,
        PrState::Merged => icons.pr_merged,
        PrState::Closed => icons.pr_closed,
    };
    let mut spans = Vec::new();
    if !icon.is_empty() {
        spans.push(Span::styled(format!("{icon} "), style));
    }
    spans.push(Span::styled(format!("#{}", pr.number), style));
    if let (Some(checks), PrState::Open | PrState::Draft) = (pr.checks, pr.state) {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            checks_icon(checks, icons),
            theme.style(checks_color(checks, theme)),
        ));
    }
    spans
}

fn missing<'a>(theme: &Theme, icons: &Icons) -> Vec<Span<'a>> {
    let mut spans = Vec::new();
    if !icons.alert.is_empty() {
        spans.push(Span::styled(
            format!("{} ", icons.alert),
            theme.style(theme.warning),
        ));
    }
    spans.push(Span::styled("missing", theme.dim()));
    spans
}

/// Lays `left` and `right` out across `width`, right-aligning `right` one
/// column in from the edge. The last left span (the name) gives way first.
fn fit<'a>(mut left: Vec<Span<'a>>, right: Vec<Span<'a>>, width: u16) -> Line<'a> {
    let width = width as usize;
    let right_width: usize = right.iter().map(Span::width).sum();
    let room = width.saturating_sub(right_width + if right.is_empty() { 1 } else { 2 });
    let mut used = 0;
    let mut kept = Vec::new();
    for span in left.drain(..) {
        let w = span.width();
        if used + w <= room {
            used += w;
            kept.push(span);
        } else {
            let text: String = span
                .content
                .chars()
                .take(room.saturating_sub(used + 1))
                .collect();
            used += text.chars().count() + 1;
            kept.push(Span::styled(format!("{text}…"), span.style));
            break;
        }
    }
    if !right.is_empty() {
        kept.push(Span::raw(
            " ".repeat(width.saturating_sub(used + right_width + 1)),
        ));
        kept.extend(right);
        kept.push(Span::raw(" "));
    }
    Line::from(kept)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake;

    fn text(line: &Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn a_worktree_takes_two_lines_with_its_pr_on_the_right() {
        let repos = fake::repos();
        let w = &repos[0].worktrees()[0];
        let lines = worktree_lines(
            w,
            true,
            SidebarStyle::Detailed,
            34,
            &Theme::dark(),
            &Icons::UNICODE,
        );
        assert_eq!(
            text(&lines[0]),
            format!(" ▸ gst-check{}#142 ✓ ", " ".repeat(15))
        );
        assert_eq!(text(&lines[1]), "     feat/gst-check  ●3 ⇡2");
        assert_eq!(lines[0].width(), 34);
    }

    #[test]
    fn compact_puts_git_status_after_the_name() {
        let repos = fake::repos();
        let w = &repos[1].worktrees()[1];
        let lines = worktree_lines(
            w,
            false,
            SidebarStyle::Compact,
            34,
            &Theme::dark(),
            &Icons::UNICODE,
        );
        assert_eq!(lines.len(), 1);
        assert_eq!(text(&lines[0]), "   ocr-retry  ●7  local");
    }

    #[test]
    fn a_long_name_gives_way_to_the_pr_badge() {
        let mut w = fake::repos()[0].worktrees()[0].clone();
        w.name = "a-very-long-worktree-folder-name".into();
        let lines = worktree_lines(
            &w,
            false,
            SidebarStyle::Detailed,
            34,
            &Theme::dark(),
            &Icons::UNICODE,
        );
        assert_eq!(text(&lines[0]), "   a-very-long-worktree-f… #142 ✓ ");
        assert_eq!(lines[0].width(), 34);
    }

    #[test]
    fn a_missing_repo_says_so_on_the_right() {
        let repos = fake::repos_with_variants();
        let line = repo_line(&repos[2], 34, &Theme::dark(), &Icons::UNICODE);
        assert_eq!(
            text(&line),
            format!(" old-scraper{}missing ", " ".repeat(14))
        );
        assert_eq!(line.width(), 34);
    }

    #[test]
    fn merged_and_closed_prs_have_no_checks_glyph() {
        let repos = fake::repos_with_variants();
        let merged = pr_badge(
            repos[0].worktrees()[2].pr.as_ref(),
            &Theme::dark(),
            &Icons::UNICODE,
        );
        assert_eq!(
            merged
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>(),
            "#141"
        );
    }
}
