//! The sidebar: repos and their worktrees, with git and PR status in columns
//! that line up from row to row.

use std::path::Path;

use abc_core::model::{Checks, GitStatus, Head, PrState, PullRequest, Repo, Worktree};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

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

/// Columns between two status columns. Nerd icons draw a little wider than
/// their cell, so an icon and its count get a space too: `● 3  ↑ 2`.
const GAP: usize = 2;

/// The gap between two columns, if both are shown.
fn gap(a: usize, b: usize) -> usize {
    if a > 0 && b > 0 { GAP } else { 0 }
}

/// Widths of the status columns, measured over every worktree shown, so each
/// glyph sits in the same column on every row.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Columns {
    lock: bool,
    pr: bool,
    pr_number: usize,
    checks: bool,
    changed: usize,
    ahead: usize,
    behind: usize,
    local: bool,
}

impl Columns {
    pub fn measure(repos: &[Repo]) -> Self {
        let mut c = Self::default();
        // Every icon is one cell, then a space, then the count.
        let width = |n: u32| 2 + n.to_string().len();
        for w in repos.iter().flat_map(|r| r.worktrees()) {
            if w.missing {
                continue;
            }
            c.lock |= w.locked;
            if let Some(pr) = &w.pr {
                c.pr = true;
                c.pr_number = c.pr_number.max(format!("#{}", pr.number).len());
                c.checks |=
                    pr.checks.is_some() && matches!(pr.state, PrState::Open | PrState::Draft);
            }
            if let Some(git) = &w.git {
                if git.changed > 0 {
                    c.changed = c.changed.max(width(git.changed));
                }
                if git.ahead > 0 {
                    c.ahead = c.ahead.max(width(git.ahead));
                }
                if git.behind > 0 {
                    c.behind = c.behind.max(width(git.behind));
                }
                c.local |= !git.upstream;
            }
        }
        c
    }

    /// Ahead and behind, or `local` in their place.
    fn upstream(&self) -> usize {
        let both = self.ahead + self.behind + gap(self.ahead, self.behind);
        both.max(if self.local { "local".len() } else { 0 })
    }

    fn status(&self) -> usize {
        let upstream = self.upstream();
        self.changed + upstream + gap(self.changed, upstream)
    }
}

fn pad(text: String, width: usize, style: Style) -> Span<'static> {
    let fill = width.saturating_sub(text.chars().count());
    Span::styled(format!("{text}{}", " ".repeat(fill)), style)
}

/// Changed files, then ahead and behind, each in its own column.
fn status_cells(git: Option<&GitStatus>, c: &Columns, theme: &Theme) -> Vec<Span<'static>> {
    if c.status() == 0 {
        return Vec::new();
    }
    let blank = GitStatus {
        upstream: true,
        ..GitStatus::default()
    };
    let git = git.unwrap_or(&blank);
    let count = |n: u32, icon: &str| {
        if n > 0 {
            format!("{icon} {n}")
        } else {
            String::new()
        }
    };
    let mut cells = Vec::new();
    if c.changed > 0 {
        cells.push(pad(
            count(git.changed, icons::CHANGED),
            c.changed,
            theme.style(theme.git_changed),
        ));
    }
    let upstream = c.upstream();
    if upstream == 0 {
        return cells;
    }
    if c.changed > 0 {
        cells.push(Span::raw(" ".repeat(GAP)));
    }
    if !git.upstream {
        cells.push(pad("local".into(), upstream, theme.dim()));
        return cells;
    }
    let mut used = 0;
    if c.ahead > 0 {
        cells.push(pad(
            count(git.ahead, icons::AHEAD),
            c.ahead,
            theme.style(theme.git_ahead),
        ));
        used += c.ahead;
    }
    if c.behind > 0 {
        if c.ahead > 0 {
            cells.push(Span::raw(" ".repeat(GAP)));
            used += GAP;
        }
        cells.push(pad(
            count(git.behind, icons::BEHIND),
            c.behind,
            theme.style(theme.git_behind),
        ));
        used += c.behind;
    }
    cells.push(Span::raw(" ".repeat(upstream - used)));
    cells
}

/// Padlock, PR icon, number (right-aligned) and checks glyph, each in its own column.
fn pr_cells(w: &Worktree, c: &Columns, theme: &Theme) -> Vec<Span<'static>> {
    let mut cells = Vec::new();
    if c.lock {
        cells.push(Span::styled(
            if w.locked { icons::LOCK } else { " " },
            theme.dim(),
        ));
        if c.pr {
            cells.push(Span::raw(" "));
        }
    }
    if !c.pr {
        return cells;
    }
    let checks_width = if c.checks { 2 } else { 0 };
    let Some(pr) = &w.pr else {
        cells.push(Span::raw(" ".repeat(2 + c.pr_number + checks_width)));
        return cells;
    };
    let style = theme.style(pr_color(pr.state, theme));
    cells.push(Span::styled(format!("{} ", pr_icon(pr.state)), style));
    cells.push(Span::styled(
        format!("{:>width$}", format!("#{}", pr.number), width = c.pr_number),
        style,
    ));
    if c.checks {
        match (pr.checks, pr.state) {
            (Some(checks), PrState::Open | PrState::Draft) => {
                cells.push(Span::raw(" "));
                cells.push(Span::styled(
                    checks_icon(checks),
                    theme.style(checks_color(checks, theme)),
                ));
            }
            _ => cells.push(Span::raw("  ")),
        }
    }
    cells
}

fn missing(theme: &Theme) -> Vec<Span<'static>> {
    vec![
        Span::styled(format!("{} ", icons::ALERT), theme.style(theme.warning)),
        Span::styled("missing", theme.dim()),
    ]
}

fn repo_line<'a>(repo: &'a Repo, width: u16, theme: &Theme) -> Line<'a> {
    let (icon, name, right) = if repo.is_missing() {
        (theme.dim(), theme.dim(), missing(theme))
    } else {
        (theme.accent(), theme.bold(), Vec::new())
    };
    let left = vec![
        Span::raw("  "),
        Span::styled(format!("{} ", icons::REPO), icon),
        Span::styled(repo.name.as_str(), name),
    ];
    fit(left, right, width)
}

fn worktree_lines<'a>(
    w: &'a Worktree,
    current: bool,
    c: &Columns,
    o: &Options,
    theme: &Theme,
) -> Vec<Line<'a>> {
    let mut left = vec![
        Span::raw("  "),
        Span::styled(if current { "▸ " } else { "  " }, theme.accent()),
    ];
    if w.is_main {
        left.push(Span::styled(format!("{} ", icons::HOME), theme.dim()));
    }
    let name_style = match (current, w.missing) {
        (true, _) => theme.accent().add_modifier(Modifier::BOLD),
        (false, true) => theme.dim(),
        (false, false) => theme.text(),
    };
    left.push(Span::styled(w.name.as_str(), name_style));

    if o.style == SidebarStyle::Compact {
        let right = if w.missing {
            missing(theme)
        } else {
            let mut right = status_cells(w.git.as_ref(), c, theme);
            let pr = pr_cells(w, c, theme);
            if !right.is_empty() && !pr.is_empty() {
                right.push(Span::raw("  "));
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
        Head::Detached(sha) => (icons::COMMIT, format!("{sha} detached")),
        Head::Branch(branch) => (icons::BRANCH, branch.clone()),
    };
    // The branch sits under the worktree's name, after its icon.
    let second = vec![
        Span::raw("    "),
        Span::styled(format!("{icon} "), theme.dim()),
        Span::styled(head, theme.dim()),
    ];
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
    let c = Columns::measure(repos);
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
        PrState::Open => icons::PR_OPEN,
        PrState::Draft => icons::PR_DRAFT,
        PrState::Merged => icons::PR_MERGED,
        PrState::Closed => icons::PR_CLOSED,
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
        Checks::Passing => icons::CHECKS_PASSING,
        Checks::Failing => icons::CHECKS_FAILING,
        Checks::Pending => icons::CHECKS_PENDING,
    }
}

/// PR icon and number in the state's colour, for the title bar.
pub fn pr_badge<'a>(pr: &PullRequest, theme: &Theme) -> Vec<Span<'a>> {
    let style = theme.style(pr_color(pr.state, theme));
    vec![
        Span::styled(format!("{} ", pr_icon(pr.state)), style),
        Span::styled(format!("#{}", pr.number), style),
    ]
}

/// Lays `left` and `right` out across `width`, with `right` ending one column
/// in from the edge. The last left span (a name) gives way first.
fn fit<'a>(left: Vec<Span<'a>>, right: Vec<Span<'a>>, width: u16) -> Line<'a> {
    let width = width as usize;
    let right_width: usize = right.iter().map(Span::width).sum();
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

    /// The character column of `glyph` in each line that has it.
    fn columns_of(lines: &[String], glyph: &str) -> Vec<usize> {
        let glyph = glyph.chars().next().unwrap();
        lines
            .iter()
            .filter_map(|l| l.chars().position(|c| c == glyph))
            .collect()
    }

    #[test]
    fn git_status_glyphs_line_up_in_columns() {
        let lines = texts(&fake::repos(), SidebarStyle::Detailed);
        for glyph in [CHANGED, AHEAD, BEHIND] {
            let cols = columns_of(&lines, glyph);
            assert!(cols.len() >= 2, "{glyph} appears on several rows");
            assert!(
                cols.windows(2).all(|w| w[0] == w[1]),
                "{glyph} columns differ: {cols:?}"
            );
        }
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
        assert!(
            ends.windows(2).all(|w| w[0] == w[1]),
            "number ends differ: {ends:?}"
        );
        let checks: Vec<usize> = [CHECKS_PASSING, CHECKS_FAILING, CHECKS_PENDING]
            .iter()
            .flat_map(|g| columns_of(&lines, g))
            .collect();
        assert_eq!(checks.len(), 3);
        assert!(
            checks.windows(2).all(|w| w[0] == w[1]),
            "checks columns differ: {checks:?}"
        );
    }

    #[test]
    fn repos_are_one_row_apart_and_worktrees_follow_each_other() {
        let lines = texts(&fake::repos(), SidebarStyle::Detailed);
        assert_eq!(lines[0], format!("  {REPO} privue-platform"));
        assert!(lines[1].starts_with("  ▸ gst-check"));
        assert!(lines[2].starts_with(&format!("    {BRANCH} feat/gst-check")));
        assert!(lines[3].starts_with("    gst-rounding"));
        let ecourts = lines
            .iter()
            .position(|l| l.starts_with(&format!("  {REPO} ecourts")))
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
    fn a_long_name_gives_way_to_the_pr_column() {
        let mut repos = fake::repos();
        if let abc_core::model::RepoState::Present { worktrees } = &mut repos[0].state {
            worktrees[0].name = "a-very-long-worktree-folder-name".into();
        }
        let lines = texts(&repos, SidebarStyle::Detailed);
        assert_eq!(lines[1].chars().count(), 38);
        assert!(lines[1].contains('…'));
        assert!(lines[1].ends_with(&format!("#142 {CHECKS_PASSING} ")));
    }

    #[test]
    fn a_missing_repo_says_so_on_the_right() {
        let lines = texts(&fake::repos_with_variants(), SidebarStyle::Detailed);
        let line = lines.iter().find(|l| l.contains("old-scraper")).unwrap();
        assert!(line.ends_with(&format!("{ALERT} missing ")), "{line:?}");
        assert_eq!(line.chars().count(), 38);
    }

    #[test]
    fn merged_and_closed_prs_show_no_checks_glyph() {
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
        assert!(lines[1].starts_with("  ▸ gst-check"));
        assert!(lines[1].contains(&format!("{CHANGED} 3")));
    }
}
