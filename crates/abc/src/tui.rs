//! `abc` itself: the frame, against abcd. A session task connects (starting
//! abcd if need be), subscribes, and turns abcd's answers and events into
//! the app's messages; the app's commands become requests. Keys and drawing
//! run on this thread, as in the gallery; talking to abcd runs on a small
//! runtime beside it.

use std::collections::HashMap;
use std::io::stdout;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use abc_core::client::{self, Client, StartError, Startup};
use abc_core::model::FileContents;
use abc_core::paths::Paths;
use abc_core::protocol::{
    self, Call, Done, Event as AbcdEvent, Files, Github, PROTOCOL, Repos, SearchDone, ServerStatus,
};
use abc_core::settings::{self, Settings};
use anyhow::{Context as _, Result};
use ratatui::crossterm::event::{self, DisableBracketedPaste, Event, KeyEventKind};
use ratatui::crossterm::execute;
use tokio::runtime::Runtime;

use crate::app::{App, Cmd, Msg, SidebarStyle, Status, StatusKind};
use crate::keys::Keymap;
use crate::layout::Widths;
use crate::theme::{Theme, ThemeChoice};
use crate::{clipboard, highlight, ui};

/// How long to wait between tries to reach abcd again.
const RECONNECT: Duration = Duration::from_secs(1);
/// How often to look for a changed settings file.
const SETTINGS_CHECK: Duration = Duration::from_secs(1);

/// What the session tells the loop.
enum Net {
    /// Connected and subscribed: the client to make requests with, where
    /// their answers go, and what to tell the app.
    Connected(Arc<Client>, Replies, Msg),
    /// The connection dropped: requests fail until the next `Connected`.
    Lost,
    Msg(Msg),
}

/// Searches running: abcd's request id for each, to the app's search id.
type Searches = Arc<Mutex<HashMap<u64, u64>>>;

/// Answers to requests, sent through the session so they reach the app
/// after the events abcd sent before them: a search's results before its end.
type Replies = tokio::sync::mpsc::UnboundedSender<Msg>;

pub fn run(paths: Paths) -> Result<()> {
    let (settings, broken) = match crate::cli::check_settings(&paths.config_file()) {
        Ok(settings) => (settings, None),
        Err(errors) => (Settings::defaults(), errors.into_iter().next()),
    };
    let mut app = App::new(paths.profile.name());
    // Ask the terminal for its background before raw mode takes over the input.
    let mut theme = Theme::resolve(theme_choice(&settings.theme, &mut app));
    apply(&settings, &mut app, &mut theme);
    if let Some(why) = broken {
        let text = format!("{} · using the defaults", settings::one_line(&why));
        app.status = Some(Status::new(StatusKind::Error, text));
    }
    let mut watch = settings::Watch::new(&paths.config_file());
    let mut watched = Instant::now();

    // abc's own log, beside abcd's; abcd makes the folder private when it starts.
    std::fs::create_dir_all(&paths.state_dir).context("creating the state folder")?;
    let _log = abc_core::log::start(&paths.client_log(), &settings.server.log)
        .context("starting the log")?;
    tracing::info!(build = abc_core::build::BUILD_ID, "abc started");

    let runtime = Runtime::new().context("starting the runtime")?;
    let abcd = crate::cli::sibling()?;
    let (tx, rx) = mpsc::channel::<Net>();
    let searches: Searches = Arc::default();
    runtime.spawn(session(
        paths.clone(),
        abcd.clone(),
        tx.clone(),
        searches.clone(),
    ));

    let mut terminal = ratatui::try_init().context("setting up the terminal")?;
    crate::gallery::paste_in_one_piece();
    let result = (|| -> Result<()> {
        let mut client: Option<(Arc<Client>, Replies)> = None;
        let mut visited: Option<PathBuf> = None;
        loop {
            terminal.draw(|frame| {
                let area = frame.area();
                ui::draw(frame, area, &app, &theme);
            })?;
            // Wait for a key or news from abcd; then handle everything
            // waiting before drawing again, so held keys never lag.
            loop {
                if let Ok(net) = rx.try_recv() {
                    receive(net, &mut app, &mut client);
                    break;
                }
                if app.tick(Instant::now()) || !app.outbox.is_empty() {
                    break;
                }
                if watched.elapsed() >= SETTINGS_CHECK {
                    watched = Instant::now();
                    if watch.changed() {
                        reload(&paths, &mut app, &mut theme);
                        break;
                    }
                }
                if event::poll(Duration::from_millis(30))? {
                    break;
                }
            }
            while let Ok(net) = rx.try_recv() {
                receive(net, &mut app, &mut client);
            }
            while event::poll(Duration::ZERO)? {
                match event::read()? {
                    Event::Key(key) if key.kind == KeyEventKind::Press => app.handle_key(key),
                    Event::Paste(text) => app.paste(&text),
                    _ => {}
                }
            }
            for cmd in std::mem::take(&mut app.outbox) {
                match cmd {
                    Cmd::Connect => {
                        runtime.spawn(session(
                            paths.clone(),
                            abcd.clone(),
                            tx.clone(),
                            searches.clone(),
                        ));
                    }
                    // On this thread: OSC 52 goes to the terminal between frames.
                    Cmd::Copy(text) => {
                        if let Err(why) = clipboard::copy(&text, &mut stdout()) {
                            app.apply(Msg::CopyFailed(why));
                        }
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
                            let msg = Msg::Highlighted {
                                worktree,
                                path,
                                spans,
                            };
                            let _ = tx.send(Net::Msg(msg));
                        });
                    }
                    cmd => match &client {
                        Some((client, replies)) => {
                            let ask = ask(cmd, client.clone(), replies.clone(), searches.clone());
                            runtime.spawn(ask);
                        }
                        None => {
                            if let Some(msg) = unanswered(cmd) {
                                app.apply(msg);
                            }
                        }
                    },
                }
            }
            // abcd remembers when each worktree was opened, for "2h ago" and tab.
            if app.current != visited
                && let Some((client, _)) = &client
            {
                visited = app.current.clone();
                if let Some(worktree) = visited.clone() {
                    let client = client.clone();
                    runtime.spawn(async move {
                        let _ = client.call::<Done>(Call::WorktreeVisit { worktree }).await;
                    });
                }
            }
            if app.quit {
                return Ok(());
            }
        }
    })();
    let _ = execute!(stdout(), DisableBracketedPaste);
    ratatui::restore();
    // abcd keeps running; the runtime's tasks end here.
    runtime.shutdown_background();
    result
}

fn receive(net: Net, app: &mut App, client: &mut Option<(Arc<Client>, Replies)>) {
    match net {
        Net::Connected(connected, replies, msg) => {
            if let Msg::Connected { startup, .. } = &msg {
                tracing::info!(abcd = connected.server.build, ?startup, "connected");
            }
            *client = Some((connected, replies));
            app.apply(msg);
        }
        Net::Lost => {
            tracing::warn!("lost abcd; reconnecting");
            *client = None;
            app.apply(Msg::ConnectionLost);
        }
        Net::Msg(msg) => {
            match &msg {
                Msg::StartFailed(why) => tracing::error!("abcd didn't start: {why}"),
                Msg::Incompatible { ours, theirs } => {
                    tracing::error!(ours, theirs, "abcd speaks another protocol");
                }
                Msg::AddFailed { why } | Msg::RemoveFailed { why } => tracing::warn!("{why}"),
                Msg::FilesListed {
                    worktree,
                    files: Err(why),
                } => {
                    tracing::warn!("listing {}: {why}", worktree.display());
                }
                _ => {}
            }
            app.apply(msg);
        }
    }
}

/// Applies the settings file again after it changed, or says why it can't.
fn reload(paths: &Paths, app: &mut App, theme: &mut Theme) {
    match crate::cli::check_settings(&paths.config_file()) {
        Ok(settings) => {
            // "auto" asked the terminal at startup; it can't be asked again in raw mode.
            if settings.theme != "auto" {
                *theme = Theme::resolve(theme_choice(&settings.theme, app));
            }
            apply(&settings, app, theme);
            app.status = Some(Status::new(StatusKind::Info, "settings reloaded"));
        }
        Err(errors) => {
            let why = errors.into_iter().next().unwrap_or_default();
            let text = format!(
                "{} · keeping the last good settings",
                settings::one_line(&why)
            );
            app.status = Some(Status::new(StatusKind::Error, text));
        }
    }
}

/// The settings that change how abc looks and which keys do what.
fn apply(settings: &Settings, app: &mut App, theme: &mut Theme) {
    app.widths = Widths {
        sidebar: settings.layout.sidebar_width,
        explorer: settings.layout.explorer_width,
        viewer_min: settings.layout.viewer_min_width,
        explorer_min: settings.layout.explorer_min_width,
    };
    app.sidebar_style = match settings.sidebar.style {
        settings::SidebarStyle::Detailed => SidebarStyle::Detailed,
        settings::SidebarStyle::Compact => SidebarStyle::Compact,
    };
    app.repo_gap = settings.sidebar.repo_gap;
    app.worktree_gap = settings.sidebar.worktree_gap;
    theme.icon_gap = usize::from(settings.icons.gap);
    app.switcher_rows = settings.switcher.rows.max(1);
    app.find.rows = settings.find.rows.max(1);
    app.find.max_lines = settings.find.max_lines;
    app.find.delay = Duration::from_millis(settings.find.delay_ms);
    app.check_delay = Duration::from_millis(settings.add_repo.check_delay_ms);
    app.which_key_delay = Duration::from_millis(settings.keys.which_key.delay_ms);
    crate::viewer::set_tab_width(settings.viewer.tab_width);
    // Checked already: a mistake there means every setting is at its default.
    app.keymap = Keymap::from_settings(&settings.keys).unwrap_or_else(|_| Keymap::defaults());
}

/// The `theme` setting as a choice; one abc doesn't know says so and goes by
/// the terminal.
fn theme_choice(theme: &str, app: &mut App) -> ThemeChoice {
    match theme {
        "auto" => ThemeChoice::Auto,
        "dark" => ThemeChoice::Dark,
        "light" => ThemeChoice::Light,
        "ansi" => ThemeChoice::Ansi,
        other => {
            let text = format!("theme {other:?} isn't one abc has · using auto");
            app.status = Some(Status::new(StatusKind::Warning, text));
            ThemeChoice::Auto
        }
    }
}

/// The connection to abcd for as long as abc runs: started at first, then
/// subscribed, sending the repositories and every change, and reconnecting
/// when it drops.
async fn session(paths: Paths, abcd: PathBuf, tx: mpsc::Sender<Net>, searches: Searches) {
    let me = client::hello();
    let send = |msg: Msg| tx.send(Net::Msg(msg)).is_ok();
    let (mut client, startup) = match client::start(&paths, &abcd, &me).await {
        Ok(started) => started,
        Err(StartError::Incompatible { ours, theirs }) => {
            send(Msg::Incompatible { ours, theirs });
            return;
        }
        Err(StartError::StartFailed(why)) => {
            send(Msg::StartFailed(why));
            return;
        }
    };
    let mut startup = Some(startup);
    let (replies, mut answered) = tokio::sync::mpsc::unbounded_channel::<Msg>();
    // Forwards an answer, letting go of a finished search's id.
    let answer = |msg: Msg| {
        if let Msg::SearchDone { id, .. } | Msg::SearchFailed { id, .. } = &msg {
            searches.lock().unwrap().retain(|_, search| search != id);
        }
        send(msg)
    };
    loop {
        let client_now = Arc::new(client);
        if let Some(mut events) = client_now.events()
            && let Ok(repos) = subscribe(&client_now).await
        {
            let startup = startup.take().unwrap_or(Startup::Ready { started: false });
            let connected = Msg::Connected { repos, startup };
            let net = Net::Connected(client_now.clone(), replies.clone(), connected);
            if tx.send(net).is_err() {
                return;
            }
            if let Ok(status) = client_now.call::<ServerStatus>(Call::ServerStatus).await
                && let Github::Off { why } = status.github
            {
                send(Msg::GithubOff(why));
            }
            loop {
                // Events first: those abcd sent before an answer are queued before it.
                let msg = tokio::select! {
                    biased;
                    event = events.recv() => match event {
                        Some(AbcdEvent::ReposChanged { repos }) => Msg::ReposChanged(repos),
                        Some(AbcdEvent::SearchFound { search, files }) => {
                            let Some(id) = searches.lock().unwrap().get(&search).copied() else {
                                continue;
                            };
                            Msg::Found { id, files }
                        }
                        // abc doesn't ask to watch yet.
                        Some(AbcdEvent::FilesChanged { .. } | AbcdEvent::FileChanged { .. }) => {
                            continue;
                        }
                        None => break,
                    },
                    Some(msg) = answered.recv() => msg,
                };
                if !answer(msg) {
                    return;
                }
            }
        }
        drop(client_now);
        if tx.send(Net::Lost).is_err() {
            return;
        }
        // Back to the abcd running, or one started; never a restart. Answers
        // to what was asked before the drop still come through meanwhile.
        client = loop {
            tokio::select! {
                Some(msg) = answered.recv() => {
                    if !answer(msg) {
                        return;
                    }
                    continue;
                }
                () = tokio::time::sleep(RECONNECT) => {}
            }
            match client::reconnect(&paths, &abcd, &me).await {
                Ok(back) if back.server.protocol == PROTOCOL => break back,
                Ok(back) => {
                    let theirs = back.server.protocol;
                    send(Msg::Incompatible {
                        ours: PROTOCOL,
                        theirs,
                    });
                    return;
                }
                Err(_) => {}
            }
        };
    }
}

/// Subscribes, then lists: every change after the list arrives as an event.
async fn subscribe(client: &Client) -> Result<Vec<abc_core::model::Repo>, protocol::Failure> {
    client.call::<Done>(Call::Subscribe).await?;
    Ok(client.call::<Repos>(Call::RepoList).await?.repos)
}

/// Asks abcd what `cmd` needs, and sends the answer back as a message.
async fn ask(cmd: Cmd, client: Arc<Client>, replies: Replies, searches: Searches) {
    let why = |failure: protocol::Failure| failure.message;
    let msg = match cmd {
        Cmd::ReadFile { worktree, path } => {
            let call = Call::FileRead {
                worktree: worktree.clone(),
                path: path.clone(),
            };
            let contents = client.call::<FileContents>(call).await.map_err(why);
            Msg::FileRead {
                worktree,
                path,
                contents,
            }
        }
        Cmd::ListFiles { worktree } => {
            let call = Call::WorktreeFiles {
                worktree: worktree.clone(),
            };
            let files = client
                .call::<Files>(call)
                .await
                .map(|f| f.files)
                .map_err(why);
            Msg::FilesListed { worktree, files }
        }
        Cmd::Search {
            worktree,
            id,
            query,
            max_lines,
        } => {
            let request = client.reserve();
            searches.lock().unwrap().insert(request, id);
            let call = Call::Search {
                worktree,
                query,
                max_lines,
            };
            // The session lets go of the id once this answer has passed.
            match client.call_as::<SearchDone>(request, call).await {
                Ok(done) => Msg::SearchDone {
                    id,
                    capped: done.capped,
                },
                Err(failure) => Msg::SearchFailed {
                    id,
                    why: failure.message,
                },
            }
        }
        Cmd::CheckRepo { path, name } => {
            let call = Call::RepoCheck {
                path: path.clone(),
                name: name.clone(),
            };
            let check = client.call(call).await.map_err(why);
            Msg::RepoChecked { path, name, check }
        }
        Cmd::AddRepo { path, name } => {
            let call = Call::RepoAdd {
                path,
                name: Some(name),
            };
            match client.call::<protocol::Added>(call).await {
                Ok(added) => Msg::RepoAdded {
                    name: added.name,
                    repos: added.repos,
                },
                Err(failure) => Msg::AddFailed {
                    why: failure.message,
                },
            }
        }
        Cmd::RemoveRepo { name } => {
            let call = Call::RepoRemove { name: name.clone() };
            match client.call::<Repos>(call).await {
                Ok(repos) => Msg::RepoRemoved {
                    name,
                    repos: repos.repos,
                },
                Err(failure) => Msg::RemoveFailed {
                    why: failure.message,
                },
            }
        }
        // Handled on the loop's thread.
        Cmd::Connect | Cmd::Copy(_) | Cmd::Highlight { .. } => return,
    };
    let _ = replies.send(msg);
}

/// The answer to `cmd` while abcd isn't connected: a failure, said plainly.
fn unanswered(cmd: Cmd) -> Option<Msg> {
    let why = || "abcd isn't connected".to_string();
    Some(match cmd {
        Cmd::ReadFile { worktree, path } => Msg::FileRead {
            worktree,
            path,
            contents: Err(why()),
        },
        Cmd::ListFiles { worktree } => Msg::FilesListed {
            worktree,
            files: Err(why()),
        },
        Cmd::Search { id, .. } => Msg::SearchFailed { id, why: why() },
        Cmd::CheckRepo { path, name } => Msg::RepoChecked {
            path,
            name,
            check: Err(why()),
        },
        Cmd::AddRepo { .. } => Msg::AddFailed { why: why() },
        Cmd::RemoveRepo { .. } => Msg::RemoveFailed { why: why() },
        // Handled on the loop's thread.
        Cmd::Connect | Cmd::Copy(_) | Cmd::Highlight { .. } => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_reach_the_app() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("config.toml");
        std::fs::write(
            &file,
            "[layout]\nsidebar_width = 30\n[sidebar]\nstyle = \"compact\"\n\
             [switcher]\nrows = 6\n[find]\nrows = 8\nmax_lines = 50\ndelay_ms = 40\n\
             [add_repo]\ncheck_delay_ms = 90\n[keys.which_key]\ndelay_ms = 120\n\
             [keys.global]\n\"space x\" = \"quit\"\n",
        )
        .unwrap();
        let settings = Settings::from_file(&file).unwrap();
        let mut app = App::new("dev");
        let mut theme = Theme::dark();
        apply(&settings, &mut app, &mut theme);
        assert_eq!(app.widths.sidebar, 30);
        assert_eq!(app.sidebar_style, SidebarStyle::Compact);
        assert_eq!(
            (app.switcher_rows, app.find.rows, app.find.max_lines),
            (6, 8, 50)
        );
        assert_eq!(app.find.delay, Duration::from_millis(40));
        assert_eq!(app.check_delay, Duration::from_millis(90));
        assert_eq!(app.which_key_delay, Duration::from_millis(120));
        assert!(
            app.keymap
                .keys_for(crate::keys::Context::Global, crate::keys::Action::Quit)
                .is_some()
        );
        let mut pending = Vec::new();
        let space = crate::keys::from_event(ratatui::crossterm::event::KeyEvent::from(
            ratatui::crossterm::event::KeyCode::Char(' '),
        ));
        let x = crate::keys::from_event(ratatui::crossterm::event::KeyEvent::from(
            ratatui::crossterm::event::KeyCode::Char('x'),
        ));
        app.keymap
            .feed(crate::keys::Context::Global, &mut pending, space);
        assert_eq!(
            app.keymap
                .feed(crate::keys::Context::Global, &mut pending, x),
            crate::keys::Feed::Action(crate::keys::Action::Quit)
        );
    }

    #[test]
    fn a_changed_settings_file_applies_or_says_why_not() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().to_path_buf();
        let paths = Paths::new(abc_core::paths::Profile::new("dev").unwrap(), move |var| {
            (var == "HOME").then(|| home.clone().into_os_string())
        })
        .unwrap();
        std::fs::create_dir_all(&paths.config_dir).unwrap();
        let mut app = App::new("dev");
        let mut theme = Theme::dark();
        std::fs::write(
            paths.config_file(),
            "theme = \"light\"\n[layout]\nsidebar_width = 30\n",
        )
        .unwrap();
        reload(&paths, &mut app, &mut theme);
        assert_eq!(app.status.as_ref().unwrap().text, "settings reloaded");
        assert_eq!((app.widths.sidebar, &theme), (30, &Theme::light()));

        std::fs::write(paths.config_file(), "[layout]\nsidebar_widht = 20\n").unwrap();
        reload(&paths, &mut app, &mut theme);
        assert_eq!(
            app.status.unwrap().text,
            "config.toml line 2: unknown field `sidebar_widht` · keeping the last good settings"
        );
        assert_eq!(app.widths.sidebar, 30, "the last good ones");
    }

    #[test]
    fn an_unknown_theme_says_so() {
        let mut app = App::new("dev");
        assert_eq!(theme_choice("light", &mut app), ThemeChoice::Light);
        assert_eq!(theme_choice("solarized", &mut app), ThemeChoice::Auto);
        assert!(app.status.unwrap().text.contains("solarized"));
    }
}
