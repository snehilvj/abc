//! The sidebar: repos and their worktrees, with git and PR status in columns
//! that line up from row to row.

use std::path::Path;

use abc_core::model::{Checks, GitStatus, Head, PrState, PullRequest, Repo, Worktree};
use ratatui::style::{Color, Modifier};
use ratatui::text::{Line, Span};

use crate::app::SidebarStyle;
use crate::icons::Icons;
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

/// Widths of the status columns, measured over every worktree shown, so each
/// glyph sits in the same column on every row.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Columns {
    lock: usize,
    pr_icon: usize,
    pr_number: usize,
    checks: usize,
    changed: usize,
    ahead: usize,
    behind: usize,
    local: bool,
}

impl Columns {
    pub fn measure(repos: &[Repo], icons: &Icons) -> Self {
        let mut c = Self::default();
        let width = |icon: &str, n: u32| icon.chars().count() + n.to_string().len();
        for w in repos.iter().flat_map(|r| r.worktrees()) {
            if w.missing {
                continue;
            }
            if w.locked {
                c.lock = icons.lock.chars().count();
            }
            if let Some(pr) = &w.pr {
                c.pr_icon = if icons.nerd { 2 } else { 0 };
                c.pr_number = c.pr_number.max(format!("#{}", pr.number).len());
                if pr.checks.is_some() && matches!(pr.state, PrState::Open | PrState::Draft) {
                    c.checks = 2;
                }
            }
            if let Some(git) = &w.git {
                if git.changed > 0 {
                    c.changed = c.changed.max(width(icons.changed, git.changed));
                }
                if git.ahead > 0 {
                    c.ahead = c.ahead.max(width(icons.ahead, git.ahead));
                }
                if git.behind > 0 {
                    c.behind = c.behind.max(width(icons.behind, git.behind));
                }
                c.local |= !git.upstream;
            }
        }
        c
    }

    /// Ahead and behind, or `local` in their place.
    fn upstream(&self) -> usize {
        let both = self.ahead + self.behind + usize::from(self.ahead > 0 && self.behind > 0);
        both.max(if self.local { "local".len() } else { 0 })
    }

    fn status(&self) -> usize {
        let upstream = self.upstream();
        self.changed + upstream + usize::from(self.changed > 0 && upstream > 0)
    }
}

fn pad(text: String, width: usize, style: ratatui::style::Style) -> Span<'static> {
    let fill = width.saturating_sub(text.chars().count());
    Span::styled(format!("{text}{}", " ".repeat(fill)), style)
}

/// Changed files, then ahead and behind, each in its own column.
fn status_cells(
    git: Option<&GitStatus>,
    c: &Columns,
    theme: &Theme,
    icons: &Icons,
) -> Vec<Span<'static>> {
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
            format!("{icon}{n}")
        } else {
            String::new()
        }
    };
    let mut cells = Vec::new();
    if c.changed > 0 {
        cells.push(pad(
            count(git.changed, icons.changed),
            c.changed,
            theme.style(theme.git_changed),
        ));
    }
    let upstream = c.upstream();
    if upstream > 0 {
        if c.changed > 0 {
            cells.push(Span::raw(" "));
        }
        if git.upstream {
            let mut used = 0;
            if c.ahead > 0 {
                cells.push(pad(
                    count(git.ahead, icons.ahead),
                    c.ahead,
                    theme.style(theme.git_ahead),
                ));
                used += c.ahead;
            }
            if c.behind > 0 {
                if c.ahead > 0 {
                    cells.push(Span::raw(" "));
                    used += 1;
                }
                cells.push(pad(
                    count(git.behind, icons.behind),
                    c.behind,
                    theme.style(theme.git_behind),
                ));
                used += c.behind;
            }
            cells.push(Span::raw(" ".repeat(upstream - used)));
        } else {
            cells.push(pad("local".into(), upstream, theme.dim()));
        }
    }
    cells
}

/// Padlock, PR icon, number (right-aligned) and checks glyph, each in its own column.
fn pr_cells(w: &Worktree, c: &Columns, theme: &Theme, icons: &Icons) -> Vec<Span<'static>> {
    let mut cells = Vec::new();
    if c.lock > 0 {
        let lock = if w.locked { icons.lock } else { "" };
        cells.push(pad(lock.into(), c.lock, theme.dim()));
        if c.pr_number > 0 {
            cells.push(Span::raw(" "));
        }
    }
    if c.pr_number == 0 {
        return cells;
    }
    let Some(pr) = &w.pr else {
        cells.push(Span::raw(" ".repeat(c.pr_icon + c.pr_number + c.checks)));
        return cells;
    };
    let style = theme.style(pr_color(pr.state, theme));
    if c.pr_icon > 0 {
        cells.push(pad(pr_icon(pr.state, icons).into(), c.pr_icon, style));
    }
    cells.push(Span::styled(
        format!("{:>width$}", format!("#{}", pr.number), width = c.pr_number),
        style,
    ));
    if c.checks > 0 {
        match (pr.checks, pr.state) {
            (Some(checks), PrState::Open | PrState::Draft) => {
                cells.push(Span::raw(" "));
                cells.push(Span::styled(
                    checks_icon(checks, icons),
                    theme.style(checks_color(checks, theme)),
                ));
            }
            _ => cells.push(Span::raw("  ")),
        }
    }
    cells
}

fn missing(theme: &Theme, icons: &Icons) -> Vec<Span<'static>> {
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

fn repo_line<'a>(repo: &'a Repo, width: u16, theme: &Theme, icons: &Icons) -> Line<'a> {
    let missing_repo = repo.is_missing();
    let mut left = vec![Span::raw("  ")];
    if !icons.repo.is_empty() {
        left.push(Span::styled(
            format!("{} ", icons.repo),
            if missing_repo {
                theme.dim()
            } else {
                theme.accent()
            },
        ));
    }
    left.push(Span::styled(
        repo.name.as_str(),
        if missing_repo {
            theme.dim()
        } else {
            theme.bold()
        },
    ));
    fit(
        left,
        if missing_repo {
            missing(theme, icons)
        } else {
            Vec::new()
        },
        width,
    )
}

fn worktree_lines<'a>(
    w: &'a Worktree,
    current: bool,
    c: &Columns,
    o: &Options,
    theme: &Theme,
    icons: &Icons,
) -> Vec<Line<'a>> {
    let mut left = vec![
        Span::raw("  "),
        Span::styled(if current { "▸ " } else { "  " }, theme.accent()),
    ];
    if w.is_main && !icons.home.is_empty() {
        left.push(Span::styled(format!("{} ", icons.home), theme.dim()));
    }
    let name_style = match (current, w.missing) {
        (true, _) => theme.accent().add_modifier(Modifier::BOLD),
        (false, true) => theme.dim(),
        (false, false) => theme.text(),
    };
    left.push(Span::styled(w.name.as_str(), name_style));

    if o.style == SidebarStyle::Compact {
        let right = if w.missing {
            missing(theme, icons)
        } else {
            let mut right = status_cells(w.git.as_ref(), c, theme, icons);
            let pr = pr_cells(w, c, theme, icons);
            if !right.is_empty() && !pr.is_empty() {
                right.push(Span::raw("  "));
            }
            right.extend(pr);
            right
        };
        return vec![fit(left, right, o.width)];
    }

    let right = if w.missing {
        missing(theme, icons)
    } else {
        pr_cells(w, c, theme, icons)
    };
    let mut second = vec![Span::raw("      ")];
    let (icon, head) = match &w.head {
        Head::Detached(sha) => (icons.commit, format!("{sha} detached")),
        Head::Branch(branch) => (icons.branch, branch.clone()),
    };
    if !icon.is_empty() {
        second.push(Span::styled(format!("{icon} "), theme.dim()));
    }
    second.push(Span::styled(head, theme.dim()));
    let status = if w.missing {
        Vec::new()
    } else {
        status_cells(w.git.as_ref(), c, theme, icons)
    };
    vec![fit(left, right, o.width), fit(second, status, o.width)]
}

/// Every sidebar line, with blank rows between repos and between worktrees.
pub fn rows<'a>(
    repos: &'a [Repo],
    current: Option<&Path>,
    o: &Options,
    theme: &Theme,
    icons: &Icons,
) -> Vec<Row<'a>> {
    let c = Columns::measure(repos, icons);
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
            line: repo_line(repo, o.width, theme, icons),
        });
        entry += 1;
        for (i, w) in repo.worktrees().iter().enumerate() {
            if i > 0 && o.style == SidebarStyle::Detailed {
                gap(&mut rows, o.worktree_gap);
            }
            let is_current = current == Some(w.path.as_path());
            for line in worktree_lines(w, is_current, &c, o, theme, icons) {
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

pub fn pr_icon(state: PrState, icons: &Icons) -> &'static str {
    match state {
        PrState::Open => icons.pr_open,
        PrState::Draft => icons.pr_draft,
        PrState::Merged => icons.pr_merged,
        PrState::Closed => icons.pr_closed,
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

/// PR icon and number in the state's colour, for the title bar.
pub fn pr_badge<'a>(pr: &PullRequest, theme: &Theme, icons: &Icons) -> Vec<Span<'a>> {
    let style = theme.style(pr_color(pr.state, theme));
    let icon = pr_icon(pr.state, icons);
    let mut spans = Vec::new();
    if !icon.is_empty() {
        spans.push(Span::styled(format!("{icon} "), style));
    }
    spans.push(Span::styled(format!("#{}", pr.number), style));
    spans
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

    fn texts(repos: &[Repo], style: SidebarStyle, icons: &Icons) -> Vec<String> {
        let o = Options {
            style,
            repo_gap: 2,
            worktree_gap: 1,
            width: 38,
        };
        rows(repos, Some(&fake::current()), &o, &Theme::dark(), icons)
            .iter()
            .map(|r| r.line.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect()
    }

    /// The character column of `needle` in each line that has it.
    fn columns_of(lines: &[String], needle: char) -> Vec<usize> {
        lines
            .iter()
            .filter_map(|l| l.chars().position(|c| c == needle))
            .collect()
    }

    #[test]
    fn git_status_glyphs_line_up_in_columns() {
        for icons in [Icons::UNICODE, Icons::NERD] {
            let lines = texts(&fake::repos(), SidebarStyle::Detailed, &icons);
            for glyph in [icons.changed, icons.ahead, icons.behind] {
                let cols = columns_of(&lines, glyph.chars().next().unwrap());
                assert!(cols.len() >= 2, "{glyph} appears on several rows");
                assert!(
                    cols.windows(2).all(|w| w[0] == w[1]),
                    "{glyph} columns differ: {cols:?}"
                );
            }
        }
    }

    #[test]
    fn pr_numbers_right_align_and_checks_share_a_column() {
        let lines = texts(&fake::repos(), SidebarStyle::Detailed, &Icons::UNICODE);
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
        let checks = columns_of(&lines, '✓')
            .into_iter()
            .chain(columns_of(&lines, '✗'))
            .collect::<Vec<_>>();
        assert!(checks.windows(2).all(|w| w[0] == w[1]));
    }

    #[test]
    fn repos_are_two_rows_apart_and_worktrees_one() {
        let lines = texts(&fake::repos(), SidebarStyle::Detailed, &Icons::UNICODE);
        assert_eq!(&lines[0], "  privue-platform");
        assert!(lines[1].starts_with("  ▸ gst-check"));
        assert!(lines[2].starts_with("      feat/gst-check"));
        assert_eq!(lines[3], "");
        assert!(lines[4].starts_with("    gst-rounding"));
        let ecourts = lines
            .iter()
            .position(|l| l.starts_with("  ecourts"))
            .unwrap();
        assert_eq!(
            (&lines[ecourts - 2], &lines[ecourts - 1]),
            (&String::new(), &String::new())
        );
    }

    #[test]
    fn every_line_fills_the_sidebar_or_ends_early() {
        for line in texts(
            &fake::repos_with_variants(),
            SidebarStyle::Detailed,
            &Icons::NERD,
        ) {
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
        let lines = texts(&repos, SidebarStyle::Detailed, &Icons::UNICODE);
        assert_eq!(lines[1].chars().count(), 38);
        assert!(lines[1].contains("…"));
        assert!(lines[1].ends_with("#142 ✓ "));
    }

    #[test]
    fn a_missing_repo_says_so_on_the_right() {
        let lines = texts(
            &fake::repos_with_variants(),
            SidebarStyle::Detailed,
            &Icons::UNICODE,
        );
        let line = lines.iter().find(|l| l.contains("old-scraper")).unwrap();
        assert!(line.ends_with("missing "), "{line:?}");
        assert_eq!(line.chars().count(), 38);
    }

    #[test]
    fn merged_and_closed_prs_show_no_checks_glyph() {
        let lines = texts(
            &fake::repos_with_variants(),
            SidebarStyle::Detailed,
            &Icons::UNICODE,
        );
        for number in ["#141", "#97"] {
            let line = lines.iter().find(|l| l.contains(number)).unwrap();
            assert!(line.trim_end().ends_with(number), "{line:?}");
        }
    }

    #[test]
    fn compact_is_one_line_per_worktree_without_worktree_gaps() {
        let lines = texts(&fake::repos(), SidebarStyle::Compact, &Icons::UNICODE);
        // 2 repos + 7 worktrees + 2 blank rows between the repos.
        assert_eq!(lines.len(), 11);
        assert!(lines[1].starts_with("  ▸ gst-check"));
        assert!(lines[1].contains("●3"));
    }
}
