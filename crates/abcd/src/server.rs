//! Listening on the profile's socket and serving each connection: hello
//! first, then requests answered in turn.

use std::fs::{self, DirBuilder, File, TryLockError};
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use abc_core::paths::Paths;
use abc_core::protocol::{
    Call, Done, ErrorCode, Event, Failure, Files, Hello, HelloLine, Repos, Request, Response,
    SearchDone, ServerStatus, encode,
};
use abc_core::settings::Settings;
use anyhow::{Context as _, bail};
use serde::Serialize;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::{Notify, broadcast, mpsc};
use tracing::{info, warn};

use crate::files::{self, Unreadable};
use crate::state::State;

/// How often abcd checks its socket is still there.
const SOCKET_CHECK: Duration = Duration::from_secs(1);
/// How long abcd waits for another abcd of its profile to let go of the lock.
const LOCK_WAIT: Duration = Duration::from_millis(500);

/// What every connection shares.
struct Server {
    build: String,
    protocol: u32,
    profile: String,
    /// Seconds since the Unix epoch.
    started: u64,
    clients: AtomicUsize,
    /// Notified when a client asks abcd to stop.
    stop: Notify,
    state: State,
}

impl Server {
    /// Files larger than this aren't read or searched (`viewer.max_file_size_mb`).
    fn file_limit(&self) -> u64 {
        self.state.settings().viewer.max_file_size_mb * 1024 * 1024
    }
}

/// Serves the profile until asked to stop, signalled, or its socket goes.
pub async fn run(paths: Paths, build: String, protocol: u32) -> anyhow::Result<()> {
    DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&paths.state_dir)
        .with_context(|| format!("creating {}", paths.state_dir.display()))?;
    let lock = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(paths.lock())
        .context("opening the lock file")?;
    // A restart starts the new abcd as the old one exits: give it a moment.
    let deadline = Instant::now() + LOCK_WAIT;
    loop {
        match lock.try_lock() {
            Ok(()) => break,
            Err(TryLockError::WouldBlock) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(TryLockError::WouldBlock) => bail!(
                "abcd is already running for profile {}",
                paths.profile.name()
            ),
            Err(TryLockError::Error(e)) => return Err(e).context("locking the lock file"),
        }
    }
    // A broken settings file mustn't stop abcd: it runs on the defaults and says why.
    let (settings, broken) = match Settings::load(&paths.config_file()) {
        Ok(settings) => (settings, None),
        Err(e) => (
            Settings::from_file(Path::new("")).expect("the defaults load"),
            Some(e),
        ),
    };
    let log = crate::log::start(&paths, &settings.server.log)?;
    if let Some(e) = broken {
        warn!("{}; using the defaults", Settings::describe(&e));
    }

    // Holding the lock, no other abcd owns a socket here: one left behind is stale.
    let socket = paths.socket();
    let _ = fs::remove_file(&socket);
    let listener = UnixListener::bind(&socket)
        .with_context(|| format!("listening on {}", socket.display()))?;
    info!(pid = std::process::id(), build, "abcd started");
    let home = std::env::var_os("HOME").map_or_else(|| PathBuf::from("/"), PathBuf::from);
    let state = State::load(paths.state_file(), home, &settings).await?;

    let server = Arc::new(Server {
        build,
        protocol,
        profile: paths.profile.name().into(),
        started: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
        clients: AtomicUsize::new(0),
        stop: Notify::new(),
        state,
    });
    let mut term = signal(SignalKind::terminate())?;
    let mut interrupt = signal(SignalKind::interrupt())?;
    // Handled, so closing the terminal that started abcd doesn't stop it.
    let mut hangup = signal(SignalKind::hangup())?;
    let mut check = tokio::time::interval(SOCKET_CHECK);
    let mut watch = abc_core::settings::Watch::new(&paths.config_file());
    // Worktrees made or removed outside abc, and repositories that went.
    let mut rescan =
        tokio::time::interval(Duration::from_secs(settings.git.status_interval.max(1)));
    let why = loop {
        tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok((stream, _)) => {
                    tokio::spawn(serve(stream, server.clone()));
                }
                Err(e) => warn!("accepting a connection: {e}"),
            },
            _ = server.stop.notified() => break "asked to",
            _ = term.recv() => break "SIGTERM",
            _ = interrupt.recv() => break "SIGINT",
            _ = hangup.recv() => info!("ignoring SIGHUP"),
            _ = check.tick() => {
                if !socket.exists() {
                    break "its socket was removed";
                }
                if watch.changed() {
                    match Settings::load(&paths.config_file()) {
                        Ok(settings) => {
                            log.set_filter(&settings.server.log);
                            let every = settings.git.status_interval.max(1);
                            if every != server.state.settings().git.status_interval.max(1) {
                                rescan = tokio::time::interval(Duration::from_secs(every));
                            }
                            server.state.set_settings(settings).await;
                            info!("settings reloaded");
                        }
                        Err(e) => warn!(
                            "{}; keeping the last good settings",
                            Settings::describe(&e)
                        ),
                    }
                }
            },
            // Off the accept loop: gh can take a while.
            _ = rescan.tick() => if server.state.subscribers.load(Ordering::SeqCst) > 0 {
                let server = server.clone();
                tokio::spawn(async move {
                    server.state.rescan().await;
                    server.state.fetch_prs_if_due().await;
                });
            },
        }
    };
    info!("abcd stopping: {why}");
    remove_socket(&socket);
    Ok(())
}

fn remove_socket(socket: &Path) {
    if let Err(e) = fs::remove_file(socket)
        && e.kind() != std::io::ErrorKind::NotFound
    {
        warn!("removing {}: {e}", socket.display());
    }
}

/// Counts a connection while it's open.
struct Client<'a>(&'a AtomicUsize);

impl<'a> Client<'a> {
    fn new(clients: &'a AtomicUsize) -> Self {
        clients.fetch_add(1, Ordering::SeqCst);
        Self(clients)
    }
}

impl Drop for Client<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// One connection: hellos both ways, then requests until it closes. A client
/// of another protocol gets abcd's hello, so it can tell, and nothing more.
async fn serve(stream: UnixStream, server: Arc<Server>) {
    let (read, mut write) = stream.into_split();
    let mut lines = BufReader::new(read).lines();
    let hello = HelloLine {
        hello: Hello {
            protocol: server.protocol,
            build: server.build.clone(),
            pid: Some(std::process::id()),
            profile: Some(server.profile.clone()),
        },
    };
    if write.write_all(encode(&hello).as_bytes()).await.is_err() {
        return;
    }
    let theirs = match lines.next_line().await {
        Ok(Some(line)) => serde_json::from_str::<HelloLine>(&line).ok(),
        _ => return,
    };
    match theirs {
        Some(HelloLine { hello }) if hello.protocol == server.protocol => {}
        other => {
            info!("hanging up on a client that said {other:?}");
            return;
        }
    }
    let _client = Client::new(&server.clients);
    let mut subscription: Option<Subscription> = None;
    // The running search's cancel flag: a new search stops the last.
    let mut searching: Option<Arc<AtomicBool>> = None;

    // Responses, and later events, go out through one writer.
    let (out, mut outgoing) = mpsc::unbounded_channel::<String>();
    let writer = tokio::spawn(async move {
        while let Some(line) = outgoing.recv().await {
            if write.write_all(line.as_bytes()).await.is_err() {
                break;
            }
        }
    });
    while let Ok(Some(line)) = lines.next_line().await {
        let request: Request = match serde_json::from_str(&line) {
            Ok(request) => request,
            Err(e) => {
                // Answer under its id if it has one, so the client isn't left waiting.
                let id = serde_json::from_str::<serde_json::Value>(&line)
                    .ok()
                    .and_then(|v| v.get("id")?.as_u64());
                if let Some(id) = id {
                    let _ = out.send(encode(&Response::error(
                        id,
                        ErrorCode::BadRequest,
                        e.to_string(),
                    )));
                }
                continue;
            }
        };
        if let Call::Search {
            worktree,
            query,
            max_lines,
        } = request.call
        {
            let cancel = Arc::new(AtomicBool::new(false));
            if let Some(last) = searching.replace(cancel.clone()) {
                last.store(true, Ordering::Relaxed);
            }
            let search = Search {
                id: request.id,
                worktree,
                query,
                max_lines,
            };
            tokio::spawn(search.run(server.clone(), cancel, out.clone()));
            continue;
        }
        let stop = request.call == Call::ServerShutdown;
        if request.call == Call::Subscribe && subscription.is_none() {
            subscription = Some(Subscription::start(&server.state, out.clone()));
        }
        let _ = out.send(encode(&answer(&server, request).await));
        if stop {
            // Let the answer go out before stopping.
            drop((subscription, out));
            let _ = writer.await;
            server.stop.notify_one();
            return;
        }
    }
    // The subscription and searches hold senders too: end them so the writer can finish.
    if let Some(search) = searching {
        search.store(true, Ordering::Relaxed);
    }
    drop((subscription, out));
    let _ = writer.await;
}

/// Forwards change events to one connection while it's open.
struct Subscription<'a> {
    subscribers: &'a AtomicUsize,
    forward: tokio::task::JoinHandle<()>,
}

impl<'a> Subscription<'a> {
    fn start(state: &'a State, out: mpsc::UnboundedSender<String>) -> Self {
        state.subscribers.fetch_add(1, Ordering::SeqCst);
        let mut events = state.events.subscribe();
        let forward = tokio::spawn(async move {
            loop {
                match events.recv().await {
                    Ok(event) => {
                        if out.send(encode(&event)).is_err() {
                            break;
                        }
                    }
                    // Events are whole snapshots: the next one makes up for any missed.
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });
        Self {
            subscribers: &state.subscribers,
            forward,
        }
    }
}

impl Drop for Subscription<'_> {
    fn drop(&mut self) {
        self.forward.abort();
        self.subscribers.fetch_sub(1, Ordering::SeqCst);
    }
}

/// A result, or the failure, as the response to request `id`.
fn reply<T: Serialize>(id: u64, result: Result<T, Failure>) -> Response {
    match result {
        Ok(result) => Response::ok(id, &result),
        Err(failure) => Response::error(id, failure.code, failure.message),
    }
}

async fn answer(server: &Server, request: Request) -> Response {
    let id = request.id;
    let state = &server.state;
    match request.call {
        Call::ServerStatus => {
            let (repos, worktrees, github) = state.summary().await;
            Response::ok(
                id,
                &ServerStatus {
                    pid: std::process::id(),
                    build: server.build.clone(),
                    profile: server.profile.clone(),
                    started: server.started,
                    clients: server.clients.load(Ordering::SeqCst),
                    repos,
                    worktrees,
                    github,
                },
            )
        }
        Call::ServerShutdown | Call::Subscribe => Response::ok(id, &Done {}),
        Call::RepoList => Response::ok(
            id,
            &Repos {
                repos: state.rescan().await,
            },
        ),
        Call::RepoCheck { path, name } => reply(id, state.check(&path, &name).await),
        Call::RepoAdd { path, name } => reply(id, state.add(&path, name).await),
        Call::RepoRemove { name } => reply(id, state.remove(&name).await),
        Call::WorktreeVisit { worktree } => reply(id, state.visit(&worktree, now()).await),
        Call::WorktreeFiles { worktree } => {
            if !state.knows(&worktree).await {
                return unknown_worktree(id, &worktree);
            }
            let files = tokio::task::spawn_blocking(move || files::list(&worktree)).await;
            Response::ok(
                id,
                &Files {
                    files: files.unwrap_or_default(),
                },
            )
        }
        Call::FileRead { worktree, path } => {
            if !state.knows(&worktree).await {
                return unknown_worktree(id, &worktree);
            }
            let limit = server.file_limit();
            let shown = path.clone();
            let read =
                tokio::task::spawn_blocking(move || files::read(&worktree, &path, limit)).await;
            match read {
                Ok(Ok(contents)) => Response::ok(id, &contents),
                Ok(Err(Unreadable::Outside)) => Response::error(
                    id,
                    ErrorCode::BadRequest,
                    format!("{shown} is outside the worktree"),
                ),
                Ok(Err(Unreadable::Io(why))) => Response::error(id, ErrorCode::Io, why),
                Err(e) => Response::error(id, ErrorCode::Io, e.to_string()),
            }
        }
        // Run apart, in `serve`, so a search never holds up other requests.
        Call::Search { .. } => Response::error(id, ErrorCode::BadRequest, "search runs apart"),
    }
}

fn unknown_worktree(id: u64, worktree: &Path) -> Response {
    Response::error(
        id,
        ErrorCode::NotFound,
        format!("no worktree at {}", worktree.display()),
    )
}

/// A search one connection asked for.
struct Search {
    id: u64,
    worktree: PathBuf,
    query: String,
    max_lines: usize,
}

impl Search {
    /// Finds the query in the worktree's files, sending what it finds as
    /// `search.found` events, a file at a time, then the response.
    async fn run(
        self,
        server: Arc<Server>,
        cancel: Arc<AtomicBool>,
        out: mpsc::UnboundedSender<String>,
    ) {
        let id = self.id;
        if !server.state.knows(&self.worktree).await {
            let _ = out.send(encode(&unknown_worktree(id, &self.worktree)));
            return;
        }
        let limit = server.file_limit();
        let events = out.clone();
        let capped = tokio::task::spawn_blocking(move || {
            let files = files::list(&self.worktree);
            files::search(
                &self.worktree,
                &files,
                &self.query,
                self.max_lines,
                limit,
                &cancel,
                |file| {
                    let found = Event::SearchFound {
                        search: id,
                        files: vec![file],
                    };
                    let _ = events.send(encode(&found));
                },
            )
        })
        .await
        .unwrap_or(false);
        let _ = out.send(encode(&Response::ok(id, &SearchDone { capped })));
    }
}

/// Seconds since the Unix epoch.
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}
