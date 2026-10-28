//! Every symbol abc draws: Nerd Font glyphs, one terminal cell each. abc needs
//! a Nerd Font in the terminal: a patched font such as FiraCode Nerd Font Mono,
//! or Symbols Nerd Font Mono as a fallback font. `ui::label` draws them.

pub const REPO: &str = "\u{f401}";
/// A repository's main checkout.
pub const HOME: &str = "\u{f46d}";
pub const BRANCH: &str = "\u{f418}";
/// A detached HEAD.
pub const COMMIT: &str = "\u{f417}";
/// The current worktree.
pub const CURRENT: &str = "\u{f44a}";

pub const PR_OPEN: &str = "\u{f407}";
pub const PR_DRAFT: &str = "\u{f4dd}";
pub const PR_MERGED: &str = "\u{f419}";
pub const PR_CLOSED: &str = "\u{f4dc}";
pub const CHECKS_PASSING: &str = "\u{f42e}";
pub const CHECKS_FAILING: &str = "\u{f467}";
pub const CHECKS_PENDING: &str = "\u{f43a}";

/// Uncommitted changes.
pub const CHANGED: &str = "\u{f444}";
pub const AHEAD: &str = "\u{f431}";
pub const BEHIND: &str = "\u{f433}";
pub const LOCK: &str = "\u{f023}";

/// Messages: something worked, failed, needs attention, or is just so.
pub const SUCCESS: &str = "\u{f4a4}";
pub const FAILURE: &str = "\u{f530}";
pub const WARNING: &str = "\u{f421}";
pub const INFO: &str = "\u{f449}";
/// Starting, connecting or restarting.
pub const SYNC: &str = "\u{f46a}";
pub const DISCONNECTED: &str = "\u{f529}";

/// A file whose type devicons doesn't know.
pub const FILE: &str = "\u{f4a5}";
pub const FOLDER: &str = "\u{e5ff}";
pub const FOLDER_OPEN: &str = "\u{e5fe}";

/// A file's icon and colour, by its name, from devicons (light or dark set).
///
/// devicons checks whether a name it doesn't know is a directory on disk. abc's
/// client never reads the disk, so the name goes behind a path that can't
/// exist: a NUL byte makes the check fail before any system call is made.
pub fn file(name: &str, theme: &crate::theme::Theme) -> (char, ratatui::style::Color) {
    let path = std::path::Path::new("\0").join(name);
    let set = if theme.dark {
        devicons::Theme::Dark
    } else {
        devicons::Theme::Light
    };
    let icon = devicons::icon_for_file(path.as_path(), &Some(set));
    // devicons answers an unknown type with a plain `*`; abc draws a file icon.
    if icon.icon == '*' {
        return (FILE.chars().next().expect("a glyph"), theme.dim);
    }
    let hex = u32::from_str_radix(icon.color.trim_start_matches('#'), 16).unwrap_or(0x7e8ea8);
    let color = ratatui::style::Color::Rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8);
    (icon.icon, color)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;

    #[test]
    fn file_icons_come_from_the_name_and_extension() {
        let dark = Theme::dark();
        assert_ne!(file("verify.ts", &dark).0, file("main.rs", &dark).0);
        assert_eq!(file("Dockerfile", &dark).0, file("dockerfile", &dark).0);
    }

    #[test]
    fn a_name_is_never_checked_against_the_disk() {
        // `src` is a real folder where the tests run; looked up by name it is
        // an unknown file, never devicons' folder icon.
        assert!(std::path::Path::new("src").is_dir());
        assert_ne!(file("src", &Theme::dark()).0, '\u{f115}');
    }

    #[test]
    fn light_and_dark_themes_get_their_own_colours() {
        let (_, dark) = file("README.md", &Theme::dark());
        let (_, light) = file("README.md", &Theme::light());
        assert_ne!(dark, light);
    }
}
