//! The icon system. Every icon abc draws goes through these helpers, so icons
//! are spaced and coloured the same way on every screen:
//!
//! - An icon is one glyph from `icons`, then `theme.icon_gap` blank cells, then
//!   its label. Many terminals draw a Nerd Font glyph into the cell after it, so
//!   the gap is part of the look rather than a hard-coded space.
//! - A badge (PR, checks, git counts, messages) colours its icon and label alike.
//! - Metadata (repo, branch, commit, main checkout) has a dim or accent icon and
//!   a plain label.
//! - Neighbouring badges sit `BADGE_GAP` cells apart.

use std::borrow::Cow;

use ratatui::style::{Color, Style};
use ratatui::text::Span;

use crate::theme::Theme;

/// Cells between neighbouring badges.
pub const BADGE_GAP: usize = 2;

/// An icon, the gap, and its label.
pub fn labelled<'a>(
    icon: &'static str,
    icon_style: Style,
    label: impl Into<Cow<'a, str>>,
    label_style: Style,
    theme: &Theme,
) -> Vec<Span<'a>> {
    vec![
        Span::styled(icon, icon_style),
        blank(theme.icon_gap),
        Span::styled(label, label_style),
    ]
}

/// An icon and its label in one colour.
pub fn badge<'a>(
    icon: &'static str,
    label: impl Into<Cow<'a, str>>,
    color: Color,
    theme: &Theme,
) -> Vec<Span<'a>> {
    let style = theme.style(color);
    labelled(icon, style, label, style, theme)
}

/// An icon with no label, followed by the gap, for markers such as the current worktree.
pub fn marker<'a>(icon: &'static str, style: Style, theme: &Theme) -> Vec<Span<'a>> {
    vec![Span::styled(icon, style), blank(theme.icon_gap)]
}

/// Cells an icon and its gap take before a label.
pub fn icon_width(theme: &Theme) -> usize {
    1 + theme.icon_gap
}

/// Blank cells as wide as an icon and its gap: keeps labels aligned where a row has no icon.
pub fn no_icon(theme: &Theme) -> Span<'static> {
    blank(icon_width(theme))
}

pub fn blank(cells: usize) -> Span<'static> {
    Span::raw(" ".repeat(cells))
}

/// Total width of some spans, in cells.
pub fn width(spans: &[Span]) -> usize {
    spans.iter().map(Span::width).sum()
}

/// Pads `spans` with blanks to `cells` wide.
pub fn pad(mut spans: Vec<Span<'_>>, cells: usize) -> Vec<Span<'_>> {
    let fill = cells.saturating_sub(width(&spans));
    if fill > 0 {
        spans.push(blank(fill));
    }
    spans
}
