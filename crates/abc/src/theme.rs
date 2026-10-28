//! Colours by role. Screens never name a colour; they ask the theme for a role.
//! Besides the built-in themes, `themes/<name>.toml` beside `config.toml`
//! holds your own: a built-in one to start from, and the colours it changes.

use std::path::Path;
use std::time::Duration;

use ratatui::style::{Color, Modifier, Style};

/// The `theme` setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, clap::ValueEnum)]
pub enum ThemeChoice {
    /// Dark or light, from the terminal's background colour.
    #[default]
    Auto,
    Dark,
    Light,
    /// The terminal's own 16 colours.
    Ansi,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    pub text: Color,
    pub dim: Color,
    pub border: Color,
    pub accent: Color,
    pub selection: Color,
    /// The background of the viewer's line with the cursor.
    pub cursor_line: Color,
    /// The background of text a search found.
    pub search_match: Color,
    /// The background of the match the cursor is on.
    pub current_match: Color,
    /// Text behind a dialog, or on screen while disconnected.
    pub faded: Color,
    pub ok: Color,
    pub warning: Color,
    pub error: Color,
    pub git_changed: Color,
    pub git_ahead: Color,
    pub git_behind: Color,
    pub pr_open: Color,
    pub pr_draft: Color,
    pub pr_merged: Color,
    pub pr_closed: Color,
    pub folder: Color,
    /// Colours for highlighted code, by tree-sitter highlight kind.
    pub syntax: Syntax,
    /// Dark or light, for colours that come in two sets (file icons).
    pub dark: bool,
    /// Blank cells between an icon and its label. Terminals that draw Nerd Font
    /// glyphs into the next cell need 2; the rest look right with 1.
    pub icon_gap: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Syntax {
    pub keyword: Color,
    pub string: Color,
    pub comment: Color,
    pub r#type: Color,
    pub function: Color,
    /// Numbers, booleans and other constants.
    pub constant: Color,
    pub property: Color,
    /// Punctuation and operators.
    pub punctuation: Color,
}

/// Every colour a theme file can set, by table and key, and the field it sets.
macro_rules! roles {
    ($($table:literal . $key:literal => $($field:ident).+,)*) => {
        const ROLES: &[(&str, &str)] = &[$(($table, $key)),*];

        /// The colour `table.key` names in `theme`, if it's one.
        fn role<'a>(theme: &'a mut Theme, table: &str, key: &str) -> Option<&'a mut Color> {
            match (table, key) {
                $(($table, $key) => Some(&mut theme.$($field).+),)*
                _ => None,
            }
        }
    };
}

roles! {
    "ui"."text" => text,
    "ui"."dim" => dim,
    "ui"."border" => border,
    "ui"."accent" => accent,
    "ui"."selection" => selection,
    "ui"."cursor_line" => cursor_line,
    "ui"."search_match" => search_match,
    "ui"."current_match" => current_match,
    "ui"."faded" => faded,
    "ui"."folder" => folder,
    "ui"."ok" => ok,
    "ui"."warning" => warning,
    "ui"."error" => error,
    "git"."changed" => git_changed,
    "git"."ahead" => git_ahead,
    "git"."behind" => git_behind,
    "pr"."open" => pr_open,
    "pr"."draft" => pr_draft,
    "pr"."merged" => pr_merged,
    "pr"."closed" => pr_closed,
    "syntax"."keyword" => syntax.keyword,
    "syntax"."string" => syntax.string,
    "syntax"."comment" => syntax.comment,
    "syntax"."type" => syntax.r#type,
    "syntax"."function" => syntax.function,
    "syntax"."constant" => syntax.constant,
    "syntax"."property" => syntax.property,
    "syntax"."punctuation" => syntax.punctuation,
}

/// A colour as a theme file writes it: a name, 0 to 255, or `#rrggbb`.
fn color_name(color: Color) -> String {
    match color {
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        Color::Indexed(n) => n.to_string(),
        named => format!("{named:?}").to_lowercase(),
    }
}

const fn rgb(hex: u32) -> Color {
    Color::Rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

impl Theme {
    pub fn dark() -> Self {
        Self {
            text: rgb(0xd7dde3),
            dim: rgb(0x6f7b86),
            border: rgb(0x333e48),
            accent: rgb(0x4fc1d1),
            selection: rgb(0x1f3a44),
            cursor_line: rgb(0x20272e),
            search_match: rgb(0x433c1f),
            current_match: rgb(0x7a5a14),
            faded: rgb(0x343d45),
            ok: rgb(0x8cc56a),
            warning: rgb(0xe2b85a),
            error: rgb(0xec716b),
            git_changed: rgb(0xe2b85a),
            git_ahead: rgb(0x4fc1d1),
            git_behind: rgb(0xc788df),
            pr_open: rgb(0x8cc56a),
            pr_draft: rgb(0x7c8894),
            pr_merged: rgb(0xb18cf0),
            pr_closed: rgb(0xec716b),
            folder: rgb(0x6ea9f0),
            syntax: Syntax {
                keyword: rgb(0xc788df),
                string: rgb(0x9ccc75),
                comment: rgb(0x64707b),
                r#type: rgb(0xe6c27a),
                function: rgb(0x6ea9f0),
                constant: rgb(0xf0a35e),
                property: rgb(0xd7dde3),
                punctuation: rgb(0x8a949e),
            },
            dark: true,
            icon_gap: 2,
        }
    }

    pub fn light() -> Self {
        Self {
            text: rgb(0x1f2429),
            dim: rgb(0x77828c),
            border: rgb(0xcdd5da),
            accent: rgb(0x0a7f8f),
            selection: rgb(0xdcecef),
            cursor_line: rgb(0xeef2f4),
            search_match: rgb(0xfaeab0),
            current_match: rgb(0xf3c35c),
            faded: rgb(0xcdd3d7),
            ok: rgb(0x2a7531),
            warning: rgb(0x9a6400),
            error: rgb(0xc0362c),
            git_changed: rgb(0x9a6400),
            git_ahead: rgb(0x0a7f8f),
            git_behind: rgb(0x8a3da6),
            pr_open: rgb(0x2a7531),
            pr_draft: rgb(0x77828c),
            pr_merged: rgb(0x7546c4),
            pr_closed: rgb(0xc0362c),
            folder: rgb(0x1f63c4),
            syntax: Syntax {
                keyword: rgb(0x8a3da6),
                string: rgb(0x2a7531),
                comment: rgb(0x8a939b),
                r#type: rgb(0x8a5a00),
                function: rgb(0x1f63c4),
                constant: rgb(0xb5520b),
                property: rgb(0x1f2429),
                punctuation: rgb(0x6b757e),
            },
            dark: false,
            icon_gap: 2,
        }
    }

    pub fn ansi() -> Self {
        Self {
            text: Color::Reset,
            dim: Color::DarkGray,
            border: Color::DarkGray,
            accent: Color::Cyan,
            selection: Color::DarkGray,
            cursor_line: Color::Reset,
            search_match: Color::DarkGray,
            current_match: Color::Yellow,
            faded: Color::DarkGray,
            ok: Color::Green,
            warning: Color::Yellow,
            error: Color::Red,
            git_changed: Color::Yellow,
            git_ahead: Color::Cyan,
            git_behind: Color::Magenta,
            pr_open: Color::Green,
            pr_draft: Color::DarkGray,
            pr_merged: Color::Magenta,
            pr_closed: Color::Red,
            folder: Color::Blue,
            syntax: Syntax {
                keyword: Color::Magenta,
                string: Color::Green,
                comment: Color::DarkGray,
                r#type: Color::Yellow,
                function: Color::Blue,
                constant: Color::Cyan,
                property: Color::Reset,
                punctuation: Color::DarkGray,
            },
            dark: true,
            icon_gap: 2,
        }
    }

    /// A built-in theme by name.
    pub fn built_in(name: &str) -> Option<Self> {
        match name {
            "dark" => Some(Self::dark()),
            "light" => Some(Self::light()),
            "ansi" => Some(Self::ansi()),
            _ => None,
        }
    }

    /// A built-in theme, or yours in `config_dir/themes/<name>.toml`.
    pub fn named(name: &str, config_dir: &Path) -> Result<Self, String> {
        if let Some(theme) = Self::built_in(name) {
            return Ok(theme);
        }
        let safe = !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        if !safe {
            return Err(format!(
                "{name:?} isn't a theme name: letters, digits, - and _"
            ));
        }
        let file = config_dir.join("themes").join(format!("{name}.toml"));
        let text = std::fs::read_to_string(&file).map_err(|_| {
            format!(
                "no theme {name:?}: not dark, light or ansi, and no {}",
                file.display()
            )
        })?;
        Self::from_toml(&text).map_err(|why| format!("{}: {why}", file.display()))
    }

    /// A theme file: `inherit` names the built-in theme it starts from (dark
    /// unless it says), and `[ui]`, `[git]`, `[pr]` and `[syntax]` the colours
    /// it changes. Every mistake is listed.
    pub fn from_toml(text: &str) -> Result<Self, String> {
        let table: toml::Table = text.parse().map_err(|e: toml::de::Error| e.to_string())?;
        let mut errors = Vec::new();
        let inherit = match table.get("inherit") {
            None => "dark",
            Some(toml::Value::String(name)) => name.as_str(),
            Some(other) => {
                errors.push(format!("inherit: {other} isn't a theme name"));
                "dark"
            }
        };
        let mut theme = Self::built_in(inherit).unwrap_or_else(|| {
            errors.push(format!(
                "inherit: {inherit:?} isn't a built-in theme: dark, light or ansi"
            ));
            Self::dark()
        });
        for (table_name, value) in &table {
            if table_name == "inherit" {
                continue;
            }
            let toml::Value::Table(colours) = value else {
                errors.push(format!("{table_name}: a table such as [ui]"));
                continue;
            };
            for (key, value) in colours {
                let Some(slot) = role(&mut theme, table_name, key) else {
                    errors.push(format!("{table_name}.{key} isn't a colour abc has"));
                    continue;
                };
                match value.as_str().map(str::parse::<Color>) {
                    Some(Ok(color)) => *slot = color,
                    _ => errors.push(format!("{table_name}.{key}: {value} isn't a colour")),
                }
            }
        }
        if errors.is_empty() {
            Ok(theme)
        } else {
            Err(errors.join("; "))
        }
    }

    /// The theme as a file, every colour written out: to copy to
    /// `themes/<name>.toml` and change.
    pub fn to_toml(&self) -> String {
        let mut theme = self.clone();
        let mut out = String::from(
            "# A theme for abc. Copy it to themes/<name>.toml beside config.toml, set\n\
             # theme = \"<name>\", and change what you like; delete what you don't.\n\
             # Colours: a name such as \"cyan\", 0 to 255, or \"#rrggbb\".\n",
        );
        let inherit = if self.dark { "dark" } else { "light" };
        out.push_str(&format!("inherit = \"{inherit}\"\n"));
        let mut table = "";
        for (t, key) in ROLES {
            if *t != table {
                table = t;
                out.push_str(&format!("\n[{t}]\n"));
            }
            let color = *role(&mut theme, t, key).expect("every role has a field");
            out.push_str(&format!("{key} = \"{}\"\n", color_name(color)));
        }
        out
    }

    /// Resolves `auto` by asking the terminal for its background. Call before
    /// entering raw mode. A terminal that doesn't answer gets the dark theme.
    pub fn resolve(choice: ThemeChoice) -> Self {
        match choice {
            ThemeChoice::Dark => Self::dark(),
            ThemeChoice::Light => Self::light(),
            ThemeChoice::Ansi => Self::ansi(),
            ThemeChoice::Auto => {
                let mut options = terminal_colorsaurus::QueryOptions::default();
                options.timeout = Duration::from_millis(100);
                match terminal_colorsaurus::theme_mode(options) {
                    Ok(terminal_colorsaurus::ThemeMode::Light) => Self::light(),
                    _ => Self::dark(),
                }
            }
        }
    }

    pub fn style(&self, color: Color) -> Style {
        Style::new().fg(color)
    }

    pub fn text(&self) -> Style {
        self.style(self.text)
    }

    pub fn dim(&self) -> Style {
        self.style(self.dim)
    }

    pub fn border(&self) -> Style {
        self.style(self.border)
    }

    pub fn accent(&self) -> Style {
        self.style(self.accent)
    }

    pub fn bold(&self) -> Style {
        self.text().add_modifier(Modifier::BOLD)
    }

    /// A pane header: accent when its pane has focus, dim otherwise.
    pub fn header(&self, focused: bool) -> Style {
        if focused {
            self.accent().add_modifier(Modifier::BOLD)
        } else {
            self.dim()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_theme_file_inherits_and_overrides_by_role() {
        let file = "inherit = \"light\"\n\
                    [ui]\naccent = \"#ff8800\"\n\
                    [git]\nahead = \"cyan\"\n\
                    [syntax]\ntype = \"12\"\n";
        let theme = Theme::from_toml(file).unwrap();
        assert_eq!(theme.accent, Color::Rgb(0xff, 0x88, 0x00));
        assert_eq!(theme.git_ahead, Color::Cyan);
        assert_eq!(theme.syntax.r#type, Color::Indexed(12));
        assert_eq!(theme.text, Theme::light().text, "the rest is light's");
        assert!(!theme.dark);
        assert_eq!(
            Theme::from_toml("").unwrap(),
            Theme::dark(),
            "dark by default"
        );
    }

    #[test]
    fn mistakes_in_a_theme_file_say_where() {
        let errors = Theme::from_toml("inherit = \"solar\"\n[ui]\naccent = \"#zz\"\n").unwrap_err();
        assert!(errors.contains("inherit: \"solar\""), "{errors}");
        let errors = Theme::from_toml("[ui]\naccent = \"#zz\"\nfolder = \"nope\"\n").unwrap_err();
        assert!(
            errors.contains("ui.accent: \"#zz\" isn't a colour"),
            "{errors}"
        );
        assert!(errors.contains("ui.folder"), "every mistake: {errors}");
        let typo = Theme::from_toml("[ui]\naccnet = \"red\"\n").unwrap_err();
        assert!(typo.contains("accnet"), "{typo}");
    }

    #[test]
    fn a_theme_written_out_reads_back_the_same() {
        for theme in [Theme::dark(), Theme::light(), Theme::ansi()] {
            let file = theme.to_toml();
            assert_eq!(Theme::from_toml(&file).unwrap(), theme, "{file}");
        }
        assert!(Theme::dark().to_toml().contains("accent = \"#4fc1d1\""));
    }

    #[test]
    fn themes_are_found_by_name() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(Theme::named("light", dir.path()), Ok(Theme::light()));
        std::fs::create_dir_all(dir.path().join("themes")).unwrap();
        std::fs::write(
            dir.path().join("themes/mine.toml"),
            "[ui]\naccent = \"red\"\n",
        )
        .unwrap();
        assert_eq!(Theme::named("mine", dir.path()).unwrap().accent, Color::Red);
        let missing = Theme::named("nope", dir.path()).unwrap_err();
        assert!(missing.contains("themes/nope.toml"), "{missing}");
    }
}
