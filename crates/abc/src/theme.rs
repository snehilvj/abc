//! Colours by role. Screens never name a colour; they ask the theme for a role.

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
    /// Dark or light, for colours that come in two sets (file icons).
    pub dark: bool,
    /// Blank cells between an icon and its label. Terminals that draw Nerd Font
    /// glyphs into the next cell need 2; the rest look right with 1.
    pub icon_gap: usize,
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
            dark: true,
            icon_gap: 2,
        }
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
