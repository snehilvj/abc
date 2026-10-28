//! Column widths for the frame at a given terminal width.

/// The smallest terminal abc draws into; anything smaller shows a size notice.
pub const MIN_WIDTH: u16 = 80;
pub const MIN_HEIGHT: u16 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Widths {
    pub sidebar: u16,
    pub explorer: u16,
    pub viewer_min: u16,
    pub explorer_min: u16,
}

impl Default for Widths {
    fn default() -> Self {
        Self {
            sidebar: 34,
            explorer: 28,
            viewer_min: 40,
            explorer_min: 20,
        }
    }
}

/// Inner widths of the three panes, not counting borders or dividers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Columns {
    pub sidebar: u16,
    pub explorer: u16,
    pub viewer: u16,
}

/// Splits a terminal `width` into panes: two outer borders and two dividers
/// take 4 columns. The viewer shrinks first, down to `viewer_min`; then the
/// explorer, down to `explorer_min`; after that the viewer takes what's left.
pub fn columns(width: u16, w: Widths) -> Columns {
    let inner = width.saturating_sub(4);
    let sidebar = w.sidebar.min(inner);
    let rest = inner - sidebar;
    let explorer = if rest >= w.explorer + w.viewer_min {
        w.explorer
    } else {
        rest.saturating_sub(w.viewer_min)
            .max(w.explorer_min)
            .min(rest)
    };
    Columns {
        sidebar,
        explorer,
        viewer: rest - explorer,
    }
}

pub fn too_small(width: u16, height: u16) -> bool {
    width < MIN_WIDTH || height < MIN_HEIGHT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_width_uses_the_configured_widths() {
        let c = columns(120, Widths::default());
        assert_eq!(
            c,
            Columns {
                sidebar: 34,
                explorer: 28,
                viewer: 54
            }
        );
    }

    #[test]
    fn the_viewer_shrinks_first() {
        // 34 + 28 + 40 + 4 = 106: the narrowest width that keeps the explorer whole.
        assert_eq!(
            columns(106, Widths::default()),
            Columns {
                sidebar: 34,
                explorer: 28,
                viewer: 40
            }
        );
    }

    #[test]
    fn then_the_explorer_shrinks_to_its_minimum() {
        assert_eq!(
            columns(100, Widths::default()),
            Columns {
                sidebar: 34,
                explorer: 22,
                viewer: 40
            }
        );
        assert_eq!(
            columns(98, Widths::default()),
            Columns {
                sidebar: 34,
                explorer: 20,
                viewer: 40
            }
        );
    }

    #[test]
    fn then_the_viewer_takes_what_is_left() {
        assert_eq!(
            columns(80, Widths::default()),
            Columns {
                sidebar: 34,
                explorer: 20,
                viewer: 22
            }
        );
    }

    #[test]
    fn below_80_by_20_is_too_small() {
        assert!(too_small(79, 40));
        assert!(too_small(120, 19));
        assert!(!too_small(80, 20));
    }
}
