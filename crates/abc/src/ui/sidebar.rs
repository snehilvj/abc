//! The sidebar: repos and their worktrees, with git and PR status in columns
//! that line up from row to row. Icons go through `label`.

use std::path::Path;

use abc_core::model::{Checks, GitStatus, Head, PrState, PullRequest, Repo, Worktree};
use ratatui::style::{Color, Modifier};
use ratatui::text::{Line, Span};

use super::label::{self, BADGE_GAP, badge, blank, labelled, marker, no_icon, pad};
use crate::app::SidebarStyle;
use crate::icons;
use crate::theme::Theme;

/// How the sidebar is laid out.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    pub style: SidebarStyle,
    /// Blank rows between repos.
    pub repo_gap: u16,
    /// Blank rows between worktrees in the detailed style.
    pub worktree_gap: u16,
    pub width: u16,
}

/// A sidebar line, and the index (in `App::entries()` order) of the repo or
/// worktree it belongs to. Gap lines belong to nothing.
pub struct Row<'a> {
    pub entry: Option<usize>,
    pub line: Line<'a>,
}

/// The gap between two columns, if both are shown.
fn gap(a: usize, b: usize) -> usize {
    if a > 0 && b > 0 { BADGE_GAP } else { 0 }
}

/// Widths of the status columns in cells, measured over every worktree shown,
/// so each badge sits in the same column on every row.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Columns {
    lock: bool,
    /// Widest `#number`; 0 when no worktree has a PR.
    pr_number: usize,
    checks: bool,
    changed: usize,
    /// Ahead and behind badges, or `local`.
    upstream: usize,
}

/// Cells for the ahead/behind badges of one worktree.
fn upstream_width(git: &GitStatus, icon: usize) -> usize {
    if !git.upstream {
        return "local".len();
    }
    let count = |n: u32| if n > 0 { icon + n.to_string().len() } else { 0 };
    let (ahead, behind) = (count(git.ahead), count(git.behind));
    ahead + behind + gap(ahead, behind)
}

impl Columns {
    pub fn measure(repos: &[Repo], theme: &Theme) -> Self {
        let icon = label::icon_width(theme);
        let mut c = Self::default();
        for w in repos
            .iter()
            .flat_map(|r| r.worktrees())
            .filter(|w| !w.missing)
        {
            c.lock |= w.locked;
            if let Some(pr) = &w.pr {
                c.pr_number = c.pr_number.max(format!("#{}", pr.number).len());
                c.checks |=
                    pr.checks.is_some() && matches!(pr.state, PrState::Open | PrState::Draft);
            }
            if let Some(git) = &w.git {
                if git.changed > 0 {
                    c.changed = c.changed.max(icon + git.changed.to_string().len());
                }
                c.upstream = c.upstream.max(upstream_width(git, icon));
            }
        }
        c
    }
}

/// Changed files, then ahead/behind (or `local`), each in its own column.
pub(crate) fn status_cells<'a>(
    git: Option<&GitStatus>,
    c: &Columns,
    theme: &Theme,
) -> Vec<Span<'a>> {
    if c.changed + c.upstream == 0 {
        return Vec::new();
    }
    let blank_status = GitStatus {
        upstream: true,
        ..GitStatus::default()
    };
    let git = git.unwrap_or(&blank_status);
    let mut cells = Vec::new();
    if c.changed > 0 {
        let changed = if git.changed > 0 {
            badge(
                icons::changed(),
                git.changed.to_string(),
                theme.git_changed,
                theme,
            )
        } else {
            Vec::new()
        };
        cells.extend(pad(changed, c.changed));
    }
    if c.upstream > 0 {
        if c.changed > 0 {
            cells.push(blank(BADGE_GAP));
        }
        let mut upstream = Vec::new();
        if !git.upstream {
            upstream.push(Span::styled("local", theme.dim()));
        } else {
            if git.ahead > 0 {
                upstream.extend(badge(
                    icons::ahead(),
                    git.ahead.to_string(),
                    theme.git_ahead,
                    theme,
                ));
            }
            if git.behind > 0 {
                if git.ahead > 0 {
                    upstream.push(blank(BADGE_GAP));
                }
                upstream.extend(badge(
                    icons::behind(),
                    git.behind.to_string(),
                    theme.git_behind,
                    theme,
                ));
            }
        }
        cells.extend(pad(upstream, c.upstream));
    }
    cells
}

/// Padlock, the PR badge (number right-aligned), then the checks icon, each in its own column.
pub(crate) fn pr_cells<'a>(w: &Worktree, c: &Columns, theme: &Theme) -> Vec<Span<'a>> {
    let mut cells = Vec::new();
    if c.lock {
        cells.push(Span::styled(
            if w.locked { icons::lock() } else { " " },
            theme.dim(),
        ));
        if c.pr_number > 0 {
            cells.push(blank(BADGE_GAP));
        }
    }
    if c.pr_number == 0 {
        return cells;
    }
    let checks_width = if c.checks { BADGE_GAP + 1 } else { 0 };
    let Some(pr) = &w.pr else {
        cells.push(blank(label::icon_width(theme) + c.pr_number + checks_width));
        return cells;
    };
    let number = format!("{:>width$}", format!("#{}", pr.number), width = c.pr_number);
    cells.extend(badge(
        pr_icon(pr.state),
        number,
        pr_color(pr.state, theme),
        theme,
    ));
    if c.checks {
        cells.push(blank(BADGE_GAP));
        match (pr.checks, pr.state) {
            (Some(checks), PrState::Open | PrState::Draft) => {
                cells.push(Span::styled(
                    checks_icon(checks),
                    theme.style(checks_color(checks, theme)),
                ));
            }
            _ => cells.push(blank(1)),
        }
    }
    cells
}

fn missing<'a>(theme: &Theme) -> Vec<Span<'a>> {
    badge(icons::warning(), "missing", theme.warning, theme)
}

fn repo_line<'a>(repo: &'a Repo, width: u16, theme: &Theme) -> Line<'a> {
    let (icon, name, right) = if repo.is_missing() {
        (theme.dim(), theme.dim(), missing(theme))
    } else {
        (theme.accent(), theme.bold(), Vec::new())
    };
    let mut left = vec![blank(2)];
    left.extend(labelled(
        icons::repo(),
        icon,
        repo.name.as_str(),
        name,
        theme,
    ));
    fit(left, right, width)
}

fn worktree_lines<'a>(
    w: &'a Worktree,
    current: bool,
    c: &Columns,
    o: &Options,
    theme: &Theme,
) -> Vec<Line<'a>> {
    // The marker column holds the current-worktree icon; names line up after it.
    let mut left = vec![blank(2)];
    if current {
        left.extend(marker(icons::current(), theme.accent(), theme));
    } else {
        left.push(no_icon(theme));
    }
    let name_style = match (current, w.missing) {
        (true, _) => theme.accent().add_modifier(Modifier::BOLD),
        (false, true) => theme.dim(),
        (false, false) => theme.text(),
    };
    if w.is_main {
        left.extend(labelled(
            icons::home(),
            theme.dim(),
            w.name.as_str(),
            name_style,
            theme,
        ));
    } else {
        left.push(Span::styled(w.name.as_str(), name_style));
    }

    if o.style == SidebarStyle::Compact {
        let right = if w.missing {
            missing(theme)
        } else {
            let mut right = status_cells(w.git.as_ref(), c, theme);
            let pr = pr_cells(w, c, theme);
            if !right.is_empty() && !pr.is_empty() {
                right.push(blank(BADGE_GAP));
            }
            right.extend(pr);
            right
        };
        return vec![fit(left, right, o.width)];
    }

    let right = if w.missing {
        missing(theme)
    } else {
        pr_cells(w, c, theme)
    };
    let (icon, head) = match &w.head {
        Head::Detached(sha) => (icons::commit(), format!("{sha} detached")),
        Head::Branch(branch) => (icons::branch(), branch.clone()),
    };
    // The branch line starts under the worktree's name.
    let mut second = vec![blank(2), no_icon(theme)];
    second.extend(labelled(icon, theme.dim(), head, theme.dim(), theme));
    let status = if w.missing {
        Vec::new()
    } else {
        status_cells(w.git.as_ref(), c, theme)
    };
    vec![fit(left, right, o.width), fit(second, status, o.width)]
}

/// Every sidebar line, with blank rows between repos and between worktrees.
pub fn rows<'a>(
    repos: &'a [Repo],
    current: Option<&Path>,
    o: &Options,
    theme: &Theme,
) -> Vec<Row<'a>> {
    let c = Columns::measure(repos, theme);
    let gap = |rows: &mut Vec<Row<'a>>, n: u16| {
        rows.extend((0..n).map(|_| Row {
            entry: None,
            line: Line::default(),
        }))
    };
    let mut rows = Vec::new();
    let mut entry = 0;
    for (r, repo) in repos.iter().enumerate() {
        if r > 0 {
            gap(&mut rows, o.repo_gap);
        }
        rows.push(Row {
            entry: Some(entry),
            line: repo_line(repo, o.width, theme),
        });
        entry += 1;
        for (i, w) in repo.worktrees().iter().enumerate() {
            if i > 0 && o.style == SidebarStyle::Detailed {
                gap(&mut rows, o.worktree_gap);
            }
            let is_current = current == Some(w.path.as_path());
            for line in worktree_lines(w, is_current, &c, o, theme) {
                rows.push(Row {
                    entry: Some(entry),
                    line,
                });
            }
            entry += 1;
        }
    }
    rows
}

pub fn pr_color(state: PrState, theme: &Theme) -> Color {
    match state {
        PrState::Open => theme.pr_open,
        PrState::Draft => theme.pr_draft,
        PrState::Merged => theme.pr_merged,
        PrState::Closed => theme.pr_closed,
    }
}

pub fn pr_icon(state: PrState) -> &'static str {
    match state {
        PrState::Open => icons::pr_open(),
        PrState::Draft => icons::pr_draft(),
        PrState::Merged => icons::pr_merged(),
        PrState::Closed => icons::pr_closed(),
    }
}

pub fn checks_color(checks: Checks, theme: &Theme) -> Color {
    match checks {
        Checks::Passing => theme.ok,
        Checks::Failing => theme.error,
        Checks::Pending => theme.warning,
    }
}

pub fn checks_icon(checks: Checks) -> &'static str {
    match checks {
        Checks::Passing => icons::checks_passing(),
        Checks::Failing => icons::checks_failing(),
        Checks::Pending => icons::checks_pending(),
    }
}

/// The PR badge for the title bar: icon and `#number` in the state's colour.
pub fn pr_badge<'a>(pr: &PullRequest, theme: &Theme) -> Vec<Span<'a>> {
    badge(
        pr_icon(pr.state),
        format!("#{}", pr.number),
        pr_color(pr.state, theme),
        theme,
    )
}

/// Lays `left` and `right` out across `width`, with `right` ending one column
/// in from the edge. The last left span (a name) gives way first.
fn fit<'a>(left: Vec<Span<'a>>, right: Vec<Span<'a>>, width: u16) -> Line<'a> {
    let width = width as usize;
    let right_width = label::width(&right);
    let room = width.saturating_sub(right_width + if right.is_empty() { 1 } else { 2 });
    let mut used = 0;
    let mut kept = Vec::new();
    for span in left {
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
        kept.push(blank(width.saturating_sub(used + right_width + 1)));
        kept.extend(right);
        kept.push(blank(1));
    }
    Line::from(kept)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake;
    use crate::icons::*;

    fn texts(repos: &[Repo], style: SidebarStyle) -> Vec<String> {
        let o = Options {
            style,
            repo_gap: 1,
            worktree_gap: 0,
            width: 38,
        };
        rows(repos, Some(&fake::current()), &o, &Theme::dark())
            .iter()
            .map(|r| r.line.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect()
    }

    fn is_icon(c: char) -> bool {
        ('\u{e000}'..='\u{f8ff}').contains(&c)
    }

    /// The character columns of any of `glyphs`, one per line that has one.
    fn columns_of(lines: &[String], glyphs: &[&str]) -> Vec<usize> {
        let glyphs: Vec<char> = glyphs.iter().map(|g| g.chars().next().unwrap()).collect();
        lines
            .iter()
            .filter_map(|l| l.chars().position(|c| glyphs.contains(&c)))
            .collect()
    }

    fn same(cols: &[usize]) -> bool {
        cols.windows(2).all(|w| w[0] == w[1])
    }

    #[test]
    fn no_icon_touches_its_label() {
        let gap = Theme::dark().icon_gap;
        for style in [SidebarStyle::Detailed, SidebarStyle::Compact] {
            for line in texts(&fake::repos_with_variants(), style) {
                let chars: Vec<char> = line.chars().collect();
                for (i, c) in chars.iter().enumerate().filter(|(_, c)| is_icon(**c)) {
                    let after = &chars[i + 1..(i + 1 + gap).min(chars.len())];
                    assert!(
                        after.iter().all(|c| *c == ' '),
                        "{c:?} touches its label in {line:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn status_badges_line_up_in_columns() {
        let lines = texts(&fake::repos(), SidebarStyle::Detailed);
        let changed = columns_of(&lines, &[CHANGED]);
        assert!(changed.len() >= 2 && same(&changed), "changed: {changed:?}");
        // Ahead and behind share one column, like the different PR icons do.
        let upstream = columns_of(&lines, &[AHEAD, BEHIND]);
        assert!(
            upstream.len() >= 3 && same(&upstream),
            "upstream: {upstream:?}"
        );
        let prs = columns_of(&lines, &[PR_OPEN, PR_DRAFT, PR_MERGED]);
        assert_eq!(prs.len(), 4);
        assert!(same(&prs), "pr icons: {prs:?}");
    }

    #[test]
    fn pr_numbers_right_align_and_checks_share_a_column() {
        let lines = texts(&fake::repos(), SidebarStyle::Detailed);
        let ends: Vec<usize> = lines
            .iter()
            .filter_map(|l| {
                let chars: Vec<char> = l.chars().collect();
                let hash = chars.iter().position(|c| *c == '#')?;
                Some(
                    hash + 1
                        + chars[hash + 1..]
                            .iter()
                            .take_while(|c| c.is_ascii_digit())
                            .count(),
                )
            })
            .collect();
        assert_eq!(ends.len(), 4);
        assert!(same(&ends), "number ends differ: {ends:?}");
        let checks = columns_of(&lines, &[CHECKS_PASSING, CHECKS_FAILING, CHECKS_PENDING]);
        assert_eq!(checks.len(), 3);
        assert!(same(&checks), "checks columns differ: {checks:?}");
    }

    #[test]
    fn repos_are_one_row_apart_and_worktrees_follow_each_other() {
        let lines = texts(&fake::repos(), SidebarStyle::Detailed);
        assert_eq!(lines[0], format!("  {REPO}  privue-platform"));
        assert!(lines[1].starts_with(&format!("  {CURRENT}  gst-check")));
        // The branch starts under the name, with its own icon.
        assert!(lines[2].starts_with(&format!("     {BRANCH}  feat/gst-check")));
        assert!(lines[3].starts_with("     gst-rounding"));
        let ecourts = lines
            .iter()
            .position(|l| l.starts_with(&format!("  {REPO}  ecourts")))
            .unwrap();
        assert_eq!(lines[ecourts - 1], "");
        assert!(!lines[ecourts - 2].is_empty());
    }

    #[test]
    fn every_line_fits_the_sidebar() {
        for line in texts(&fake::repos_with_variants(), SidebarStyle::Detailed) {
            assert!(
                line.chars().count() <= 38,
                "{line:?} is wider than the sidebar"
            );
        }
    }

    #[test]
    fn branch_names_are_not_cut_off() {
        let lines = texts(&fake::repos(), SidebarStyle::Detailed);
        for branch in [
            "feat/gst-check",
            "fix/gst-rounding",
            "fix/ledger-round",
            "feat/gst-lookup",
            "feat/ocr-retry",
        ] {
            assert!(
                lines.iter().any(|l| l.contains(branch)),
                "{branch} is cut off"
            );
        }
    }

    #[test]
    fn a_long_name_gives_way_to_the_pr_column() {
        let mut repos = fake::repos();
        if let abc_core::model::RepoState::Present { worktrees } = &mut repos[0].state {
            worktrees[0].name = "a-very-long-worktree-folder-name".into();
        }
        let lines = texts(&repos, SidebarStyle::Detailed);
        assert_eq!(lines[1].chars().count(), 38);
        assert!(lines[1].contains('…'));
        assert!(lines[1].ends_with(&format!("#142  {CHECKS_PASSING} ")));
    }

    #[test]
    fn a_missing_repo_says_so_on_the_right() {
        let lines = texts(&fake::repos_with_variants(), SidebarStyle::Detailed);
        let line = lines.iter().find(|l| l.contains("old-scraper")).unwrap();
        assert!(line.ends_with(&format!("{WARNING}  missing ")), "{line:?}");
        assert_eq!(line.chars().count(), 38);
    }

    #[test]
    fn merged_and_closed_prs_show_no_checks_icon() {
        let lines = texts(&fake::repos_with_variants(), SidebarStyle::Detailed);
        for number in ["#141", "#97"] {
            let line = lines.iter().find(|l| l.contains(number)).unwrap();
            assert!(line.trim_end().ends_with(number), "{line:?}");
        }
    }

    #[test]
    fn compact_is_one_line_per_worktree_without_worktree_gaps() {
        let lines = texts(&fake::repos(), SidebarStyle::Compact);
        // 2 repos + 7 worktrees + 1 blank row between the repos.
        assert_eq!(lines.len(), 10);
        assert!(lines[1].starts_with(&format!("  {CURRENT}  gst-")));
    }
}
