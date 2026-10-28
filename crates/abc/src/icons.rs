//! Every symbol abc draws: Nerd Font glyphs, one terminal cell each. abc needs
//! a Nerd Font in the terminal: a patched font such as FiraCode Nerd Font Mono,
//! or Symbols Nerd Font Mono as a fallback font. `ui::label` draws them.
//! `[icons.glyphs]` in the settings changes any of them, one by one.

use std::collections::BTreeMap;
use std::sync::RwLock;

/// The glyph table: written once here, it makes for each glyph a constant,
/// abc's own, and a function, the glyph drawn now, named as settings name it
/// in `[icons.glyphs]`; and `Glyphs`, all of them.
macro_rules! glyphs {
    ($($(#[$doc:meta])* $own:ident $name:ident = $glyph:literal,)*) => {
        $(
            $(#[$doc])*
            pub const $own: &str = $glyph;
        )*

        /// Every glyph abc draws besides file types', which devicons gives.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct Glyphs {
            $(pub $name: &'static str,)*
        }

        impl Glyphs {
            /// abc's own.
            pub const DEFAULT: Self = Self { $($name: $own,)* };
            /// Every glyph's name, as settings name it.
            pub const NAMES: &[&str] = &[$(stringify!($name)),*];

            fn slot(&mut self, name: &str) -> Option<&mut &'static str> {
                match name {
                    $(stringify!($name) => Some(&mut self.$name),)*
                    _ => None,
                }
            }
        }

        $(
            $(#[$doc])*
            pub fn $name() -> &'static str {
                GLYPHS.read().unwrap().$name
            }
        )*
    };
}

glyphs! {
    REPO repo = "\u{f401}",
    /// A repository's main checkout.
    HOME home = "\u{f46d}",
    BRANCH branch = "\u{f418}",
    /// A detached HEAD.
    COMMIT commit = "\u{f417}",
    /// The current worktree.
    CURRENT current = "\u{f44a}",
    PR_OPEN pr_open = "\u{f407}",
    PR_DRAFT pr_draft = "\u{f4dd}",
    PR_MERGED pr_merged = "\u{f419}",
    PR_CLOSED pr_closed = "\u{f4dc}",
    CHECKS_PASSING checks_passing = "\u{f42e}",
    CHECKS_FAILING checks_failing = "\u{f467}",
    CHECKS_PENDING checks_pending = "\u{f43a}",
    /// Uncommitted changes.
    CHANGED changed = "\u{f444}",
    AHEAD ahead = "\u{f431}",
    BEHIND behind = "\u{f433}",
    LOCK lock = "\u{f023}",
    /// Messages: something worked, failed, needs attention, or is just so.
    SUCCESS success = "\u{f4a4}",
    FAILURE failure = "\u{f530}",
    WARNING warning = "\u{f421}",
    INFO info = "\u{f449}",
    /// Starting, connecting or restarting.
    SYNC sync = "\u{f46a}",
    DISCONNECTED disconnected = "\u{f529}",
    /// A file whose type devicons doesn't know.
    FILE file = "\u{f4a5}",
    FOLDER folder = "\u{e5ff}",
    FOLDER_OPEN folder_open = "\u{e5fe}",
}

/// The glyphs drawn now: abc's own, with `[icons.glyphs]` over them. Shared
/// as the tab width is, rather than threaded through every drawing function.
static GLYPHS: RwLock<Glyphs> = RwLock::new(Glyphs::DEFAULT);

/// abc's glyphs with `overrides` (name to glyph) over them, or every mistake.
pub fn with_overrides(overrides: &BTreeMap<String, String>) -> Result<Glyphs, Vec<String>> {
    let mut glyphs = Glyphs::DEFAULT;
    let mut errors = Vec::new();
    for (name, glyph) in overrides {
        let Some(slot) = glyphs.slot(name) else {
            errors.push(format!(
                "icons.glyphs: {name:?} isn't an icon abc has: {}",
                Glyphs::NAMES.join(", ")
            ));
            continue;
        };
        if glyph.chars().count() != 1 {
            errors.push(format!(
                "icons.glyphs: {name} = {glyph:?}: one character, such as a Nerd Font glyph"
            ));
            continue;
        }
        // Set once per settings load: a few bytes each time is no matter.
        *slot = Box::leak(glyph.clone().into_boxed_str());
    }
    if errors.is_empty() {
        Ok(glyphs)
    } else {
        Err(errors)
    }
}

/// Draws `glyphs` from now on.
pub fn set(glyphs: Glyphs) {
    *GLYPHS.write().unwrap() = glyphs;
}

/// A file's icon and colour, by its name, from devicons (light or dark set).
///
/// devicons checks whether a name it doesn't know is a directory on disk. abc's
/// client never reads the disk, so the name goes behind a path that can't
/// exist: a NUL byte makes the check fail before any system call is made.
pub fn for_file(name: &str, theme: &crate::theme::Theme) -> (char, ratatui::style::Color) {
    let path = std::path::Path::new("\0").join(name);
    let set = if theme.dark {
        devicons::Theme::Dark
    } else {
        devicons::Theme::Light
    };
    let icon = devicons::icon_for_file(path.as_path(), &Some(set));
    // devicons answers an unknown type with a plain `*`; abc draws a file icon.
    if icon.icon == '*' {
        return (file().chars().next().expect("a glyph"), theme.dim);
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
    fn glyphs_are_overridden_one_by_one() {
        let overrides = BTreeMap::from([("branch".to_string(), "b".to_string())]);
        let glyphs = with_overrides(&overrides).unwrap();
        assert_eq!(glyphs.branch, "b");
        assert_eq!(glyphs.commit, Glyphs::DEFAULT.commit, "the rest are abc's");
        assert_eq!(with_overrides(&BTreeMap::new()), Ok(Glyphs::DEFAULT));
    }

    #[test]
    fn an_unknown_icon_or_a_long_glyph_says_so() {
        let overrides = BTreeMap::from([
            ("brnch".to_string(), "b".to_string()),
            ("lock".to_string(), "locked".to_string()),
        ]);
        let errors = with_overrides(&overrides).unwrap_err();
        assert_eq!(errors.len(), 2);
        assert!(errors[0].contains("\"brnch\" isn't an icon abc has: repo, home, branch"));
        assert!(errors[1].contains("lock = \"locked\": one character"));
    }

    #[test]
    fn file_icons_come_from_the_name_and_extension() {
        let dark = Theme::dark();
        assert_ne!(for_file("verify.ts", &dark).0, for_file("main.rs", &dark).0);
        assert_eq!(
            for_file("Dockerfile", &dark).0,
            for_file("dockerfile", &dark).0
        );
    }

    #[test]
    fn a_name_is_never_checked_against_the_disk() {
        // `src` is a real folder where the tests run; looked up by name it is
        // an unknown file, never devicons' folder icon.
        assert!(std::path::Path::new("src").is_dir());
        assert_ne!(for_file("src", &Theme::dark()).0, '\u{f115}');
    }

    #[test]
    fn light_and_dark_themes_get_their_own_colours() {
        let (_, dark) = for_file("README.md", &Theme::dark());
        let (_, light) = for_file("README.md", &Theme::light());
        assert_ne!(dark, light);
    }
}
