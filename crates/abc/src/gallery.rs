//! `abc gallery <screen>`: a screen on fake data, in the real terminal, with no
//! server. Each screen has named states; `[` and `]` step through them.

use std::io::stdout;
use std::time::Duration;

use anyhow::{Context as _, Result, anyhow};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::SetTitle;
use ratatui::layout::Rect;
use ratatui::widgets::Paragraph;

use crate::app::{App, Connection, Entry, Focus, SidebarStyle, Status, StatusKind};
use crate::fake;
use crate::theme::{Theme, ThemeChoice};
use crate::ui;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Screen {
    Frame,
    /// Panics on the first key, to check the terminal comes back clean.
    #[value(hide = true)]
    Panic,
}

#[derive(Debug, clap::Args)]
pub struct Args {
    /// The screen to open.
    #[arg(value_enum)]
    pub screen: Screen,
    /// The state to start in. `[` and `]` step through the others.
    #[arg(long)]
    pub state: Option<String>,
    #[arg(long, value_enum, default_value_t)]
    pub theme: ThemeChoice,
}

/// A named starting point for a screen: the state the app is in, and anything
/// the gallery draws differently.
#[derive(Debug, Clone)]
pub struct Preset {
    pub name: &'static str,
    pub app: App,
    /// Draw as if the terminal were this size.
    pub size: Option<(u16, u16)>,
}

fn preset(name: &'static str, app: App) -> Preset {
    Preset {
        name,
        app,
        size: None,
    }
}

pub fn presets(screen: Screen) -> Vec<Preset> {
    match screen {
        Screen::Frame => frame(),
        Screen::Panic => Vec::new(),
    }
}

/// Connected to abcd, with `repos` and the usual worktree open.
fn connected(repos: Vec<abc_core::model::Repo>) -> App {
    let mut app = App::new("dev");
    app.repos = repos;
    app.connection = Connection::Connected;
    app.open(&fake::current());
    app
}

fn with_status(kind: StatusKind, text: &str) -> App {
    App {
        status: Some(Status::new(kind, text)),
        ..connected(fake::repos())
    }
}

fn cursor_on(app: &mut App, entry: Entry) {
    app.focus = Focus::Sidebar;
    app.sidebar_cursor = app
        .entries()
        .iter()
        .position(|e| *e == entry)
        .expect("entry exists");
}

pub fn frame() -> Vec<Preset> {
    let mut sidebar = connected(fake::repos_with_variants());
    cursor_on(&mut sidebar, Entry::Worktree(0, 1));

    let mut missing = connected(fake::repos_with_variants());
    cursor_on(&mut missing, Entry::Repo(2));

    let empty = App {
        connection: Connection::Connected,
        ..App::new("dev")
    };
    vec![
        preset("populated", connected(fake::repos())),
        preset("sidebar", sidebar),
        preset(
            "compact",
            App {
                sidebar_style: SidebarStyle::Compact,
                ..connected(fake::repos())
            },
        ),
        preset("empty", empty),
        preset("loading", App::new("dev")),
        preset(
            "error",
            App {
                connection: Connection::StartFailed,
                ..App::new("dev")
            },
        ),
        preset("missing", missing),
        preset(
            "added",
            with_status(StatusKind::Ok, "✓ added privue-agents"),
        ),
        preset(
            "gh-off",
            App {
                repos: fake::repos_without_prs(),
                ..with_status(
                    StatusKind::Warning,
                    "! gh is not logged in · PR status is off · run gh auth login",
                )
            },
        ),
        preset(
            "config-error",
            with_status(
                StatusKind::Error,
                "✗ config.toml line 14: unknown key sidebar.widht · keeping the last good settings",
            ),
        ),
        preset(
            "restarted",
            with_status(StatusKind::Info, "↻ abcd restarted · build 0.1.0+3f9a2c1e"),
        ),
        preset(
            "disconnected",
            App {
                connection: Connection::Lost,
                ..connected(fake::repos())
            },
        ),
        preset(
            "stale-client",
            App {
                stale_client: true,
                ..connected(fake::repos())
            },
        ),
        preset(
            "stale",
            App {
                connection: Connection::Incompatible { ours: 1, theirs: 2 },
                ..App::new("dev")
            },
        ),
        Preset {
            size: Some((64, 18)),
            ..preset("small", connected(fake::repos()))
        },
    ]
}

pub fn run(args: Args) -> Result<()> {
    if args.screen == Screen::Panic {
        return run_panic();
    }
    let presets = presets(args.screen);
    let names = || {
        presets
            .iter()
            .map(|p| p.name)
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut index = match &args.state {
        None => 0,
        Some(state) => presets
            .iter()
            .position(|p| p.name == state)
            .ok_or_else(|| anyhow!("no state {state:?}; the states are: {}", names()))?,
    };
    // Ask the terminal for its background before raw mode takes over the input.
    let theme = Theme::resolve(args.theme);
    let mut terminal = ratatui::try_init().context("setting up the terminal")?;
    let result = (|| -> Result<()> {
        let mut app = presets[index].app.clone();
        set_title(args.screen, presets[index].name);
        loop {
            let preset = &presets[index];
            terminal.draw(|frame| {
                let area = match preset.size {
                    Some((w, h)) => Rect::new(0, 0, w, h).clamp(frame.area()),
                    None => frame.area(),
                };
                ui::draw(frame, area, &app, &theme);
            })?;
            // Handle everything already typed before drawing again, so held keys never lag.
            let mut wait = true;
            while wait || event::poll(Duration::ZERO)? {
                wait = false;
                let Event::Key(key) = event::read()? else {
                    continue;
                };
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                let step = match key.code {
                    KeyCode::Char(']') => 1,
                    KeyCode::Char('[') => presets.len() - 1,
                    _ => {
                        app.handle_key(key);
                        continue;
                    }
                };
                index = (index + step) % presets.len();
                app = presets[index].app.clone();
                set_title(args.screen, presets[index].name);
            }
            if app.quit {
                return Ok(());
            }
        }
    })();
    ratatui::restore();
    result
}

fn set_title(screen: Screen, state: &str) {
    let screen = format!("{screen:?}").to_lowercase();
    // A terminal that ignores titles loses nothing.
    let _ = execute!(
        stdout(),
        SetTitle(format!("abc gallery {screen} · {state}"))
    );
}

fn run_panic() -> Result<()> {
    let mut terminal = ratatui::try_init().context("setting up the terminal")?;
    terminal.draw(|frame| {
        let text = "Press any key and abc panics. Your terminal should come back clean.";
        frame.render_widget(Paragraph::new(text), frame.area());
    })?;
    loop {
        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            panic!("abc gallery panic: checking that the terminal is restored");
        }
    }
}
