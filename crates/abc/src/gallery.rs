//! `abc gallery <screen>`: a screen on fake data, in the real terminal, with no
//! server. Each screen has named states; `[` and `]` step through them.

use std::io::stdout;
use std::sync::mpsc;
use std::time::Duration;

use anyhow::{Context as _, Result, anyhow};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::SetTitle;
use ratatui::layout::Rect;
use ratatui::widgets::Paragraph;

use crate::app::{App, Cmd, Connection, Entry, Focus, Msg, SidebarStyle, Status, StatusKind};
use crate::clipboard;
use crate::explorer::{Explorer, Listing};
use crate::fake;
use crate::highlight;
use crate::motion::Pos;
use crate::theme::{Theme, ThemeChoice};
use crate::ui;
use crate::viewer::{Document, Viewer};

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Screen {
    Frame,
    Explorer,
    Viewer,
    /// Every icon and what it means, with a 1-cell and a 2-cell gap side by side.
    Icons,
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
        Screen::Explorer => explorer(),
        Screen::Viewer => viewer(),
        Screen::Icons | Screen::Panic => Vec::new(),
    }
}

/// Connected to abcd, with `repos` and the usual worktree open.
fn connected(repos: Vec<abc_core::model::Repo>) -> App {
    let mut app = App::new("dev");
    app.listings = fake::listings(&repos);
    app.repos = repos;
    app.connection = Connection::Connected;
    app.open(&fake::current());
    let verify = "packages/integrations/gst/verify.ts";
    app.viewers.insert(fake::current(), fake::viewer(verify));
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
        preset("added", with_status(StatusKind::Ok, "added privue-agents")),
        preset(
            "gh-off",
            App {
                repos: fake::repos_without_prs(),
                ..with_status(
                    StatusKind::Warning,
                    "gh is not logged in · PR status is off · run gh auth login",
                )
            },
        ),
        preset(
            "config-error",
            with_status(
                StatusKind::Error,
                "config.toml line 14: unknown key sidebar.widht · keeping the last good settings",
            ),
        ),
        preset(
            "restarted",
            with_status(StatusKind::Info, "abcd restarted · build 0.1.0+3f9a2c1e"),
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

/// The explorer's states, each on the populated frame with the explorer focused.
pub fn explorer() -> Vec<Preset> {
    let with = |listing: Listing| {
        let mut app = connected(fake::repos());
        app.listings.insert(fake::current(), listing);
        app
    };
    vec![
        preset("tree", connected(fake::repos())),
        preset("loading", with(Listing::Loading)),
        preset(
            "empty",
            with(Listing::Ready(Box::new(Explorer::new::<&str>(&[])))),
        ),
        preset("error", with(Listing::Failed("permission denied".into()))),
    ]
}

/// The viewer's states, each on the populated frame with the viewer focused.
pub fn viewer() -> Vec<Preset> {
    let open = |path: &str, extra: &[&str]| {
        let mut app = connected(fake::repos());
        app.focus = Focus::Viewer;
        let segments: Vec<String> = path.split('/').map(String::from).collect();
        if let Some(Listing::Ready(explorer)) = app.listings.get_mut(&fake::current()) {
            for folder in extra {
                explorer
                    .state
                    .borrow_mut()
                    .open(folder.split('/').map(String::from).collect());
            }
            explorer.reveal(&segments);
        }
        app.viewers.insert(fake::current(), fake::viewer(path));
        app
    };
    let verify = "packages/integrations/gst/verify.ts";
    let with_document = |document: Document| {
        let mut app = open(verify, &[]);
        let path = verify.split('/').map(String::from).collect();
        app.viewers
            .insert(fake::current(), Viewer::new(path, document));
        app
    };
    let mut pending = open(verify, &[]);
    if let Some(Viewer {
        document: Document::Text(text),
        ..
    }) = pending.viewers.get_mut(&fake::current())
    {
        text.spans = None;
    }
    let mut none = connected(fake::repos());
    none.viewers.clear();
    if let Some(Listing::Ready(explorer)) = none.listings.get_mut(&fake::current()) {
        explorer.open = None;
    }
    let mut large = open("generated/rows.rs", &[]);
    if let Some(viewer) = large.viewers.get_mut(&fake::current()) {
        viewer.go(Pos::new(usize::MAX, 0));
    }
    let client = "packages/integrations/gst/client.ts";
    // verify.ts with a selection from one place to another.
    let select = |from: Pos, to: Pos, by_line: bool| {
        let mut app = open(verify, &[]);
        if let Some(viewer) = app.viewers.get_mut(&fake::current()) {
            viewer.go(from);
            viewer.visual(by_line);
            viewer.go(to);
        }
        app
    };
    // The cursor near the end of a line longer than the viewer is wide.
    let mut sideways = open(client, &[]);
    if let Some(viewer) = sideways.viewers.get_mut(&fake::current()) {
        viewer.go(Pos::new(10, 52));
    }
    vec![
        preset("highlighted", open(verify, &[])),
        preset("pending", pending),
        preset("none", none),
        preset("loading", with_document(Document::Loading)),
        preset("plain", open("README.md", &[])),
        preset("binary", open("docs/logo.png", &[])),
        preset("too-large", open("data/dump.sql", &[])),
        preset(
            "error",
            with_document(Document::Failed("permission denied".into())),
        ),
        preset("large", large),
        preset("sideways", sideways),
        preset("visual", select(Pos::new(3, 4), Pos::new(6, 9), false)),
        preset("visual-lines", select(Pos::new(3, 0), Pos::new(5, 0), true)),
        preset("searching", typed(open(client, &[]), "/gst")),
        preset("search", typed(open(client, &[]), "/gstin\nn")),
        preset("go-to-line", typed(open(verify, &[]), ":42")),
    ]
}

/// `app` after typing `keys` in it, one key per character; `\n` is enter.
fn typed(mut app: App, keys: &str) -> App {
    for c in keys.chars() {
        let code = match c {
            '\n' => KeyCode::Enter,
            c => KeyCode::Char(c),
        };
        app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
    }
    app
}

/// Carries out the app's commands from fake data, as abcd will: files are read
/// at once; highlighting runs on a thread and comes back as a message.
fn serve(app: &mut App, tx: &mpsc::Sender<Msg>) {
    while !app.outbox.is_empty() {
        for cmd in std::mem::take(&mut app.outbox) {
            match cmd {
                Cmd::Copy(text) => {
                    // On this thread: OSC 52 goes to the terminal between frames.
                    if let Err(why) = clipboard::copy(&text, &mut stdout()) {
                        app.apply(Msg::CopyFailed(why));
                    }
                }
                Cmd::ReadFile { worktree, path } => {
                    let contents = fake::read(&path);
                    app.apply(Msg::FileRead {
                        worktree,
                        path,
                        contents,
                    });
                }
                Cmd::Highlight {
                    worktree,
                    path,
                    source,
                    lang,
                } => {
                    let tx = tx.clone();
                    std::thread::spawn(move || {
                        let spans = highlight::highlight(&source, lang);
                        let _ = tx.send(Msg::Highlighted {
                            worktree,
                            path,
                            spans,
                        });
                    });
                }
            }
        }
    }
}

pub fn run(args: Args) -> Result<()> {
    match args.screen {
        Screen::Panic => return run_panic(),
        Screen::Icons => return run_catalog(args.state.as_deref(), Theme::resolve(args.theme)),
        Screen::Frame | Screen::Explorer | Screen::Viewer => {}
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
        let (tx, rx) = mpsc::channel();
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
            // Wait for a key or a finished job; then handle everything waiting
            // before drawing again, so held keys never lag.
            loop {
                if let Ok(msg) = rx.try_recv() {
                    app.apply(msg);
                    break;
                }
                if event::poll(Duration::from_millis(30))? {
                    break;
                }
            }
            while let Ok(msg) = rx.try_recv() {
                app.apply(msg);
            }
            while event::poll(Duration::ZERO)? {
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
                        serve(&mut app, &tx);
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

/// The icon catalogue's states: the icon system, and file types.
pub const ICON_STATES: [&str; 2] = ["system", "files"];

fn run_catalog(state: Option<&str>, theme: Theme) -> Result<()> {
    let mut index = match state {
        None => 0,
        Some(state) => ICON_STATES
            .iter()
            .position(|s| *s == state)
            .ok_or_else(|| {
                anyhow!(
                    "no state {state:?}; the states are: {}",
                    ICON_STATES.join(", ")
                )
            })?,
    };
    let mut terminal = ratatui::try_init().context("setting up the terminal")?;
    let result = (|| -> Result<()> {
        loop {
            set_title(Screen::Icons, ICON_STATES[index]);
            terminal.draw(|frame| {
                let area = frame.area();
                match ICON_STATES[index] {
                    "files" => ui::catalog::draw_files(frame.buffer_mut(), area, &theme),
                    _ => ui::catalog::draw(frame.buffer_mut(), area, &theme),
                }
            })?;
            let Event::Key(key) = event::read()? else {
                continue;
            };
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                KeyCode::Char(']') => index = (index + 1) % ICON_STATES.len(),
                KeyCode::Char('[') => index = (index + ICON_STATES.len() - 1) % ICON_STATES.len(),
                _ => {}
            }
        }
    })();
    ratatui::restore();
    result
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
