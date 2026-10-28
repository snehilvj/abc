//! Listening on the profile's socket and serving each connection: hello
//! first, then requests answered in turn.

use std::fs::{self, DirBuilder, File, TryLockError};
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use abc_core::paths::Paths;
use abc_core::protocol::{
    Call, Done, ErrorCode, Failure, Hello, HelloLine, Repos, Request, Response, ServerStatus,
    encode,
};
use anyhow::{Context as _, bail};
use serde::Serialize;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::{Notify, broadcast, mpsc};
use tracing::{info, warn};

use crate::state::State;

/// How often abcd checks its socket is still there.
const SOCKET_CHECK: Duration = Duration::from_secs(1);
/// How long abcd waits for another abcd of its profile to let go of the lock.
const LOCK_WAIT: Duration = Duration::from_millis(500);
/// How often abcd rescans repositories while anyone is subscribed (`git.status_interval`).
const RESCAN: Duration = Duration::from_secs(5);

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
    let _log = crate::log::start(&paths)?;

    // Holding the lock, no other abcd owns a socket here: one left behind is stale.
    let socket = paths.socket();
    let _ = fs::remove_file(&socket);
    let listener = UnixListener::bind(&socket)
        .with_context(|| format!("listening on {}", socket.display()))?;
    info!(pid = std::process::id(), build, "abcd started");
    let home = std::env::var_os("HOME").map_or_else(|| PathBuf::from("/"), PathBuf::from);
    let state = State::load(paths.state_file(), home).await?;

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
    let mut rescan = tokio::time::interval(RESCAN);
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
            _ = check.tick() => if !socket.exists() {
                break "its socket was removed";
            },
            // Worktrees made or removed outside abc, and repositories that went.
            _ = rescan.tick() => if server.state.subscribers.load(Ordering::SeqCst) > 0 {
                server.state.rescan().await;
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
    // The subscription holds a sender too: end it so the writer can finish.
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
        Call::ServerStatus => Response::ok(
            id,
            &ServerStatus {
                pid: std::process::id(),
                build: server.build.clone(),
                profile: server.profile.clone(),
                started: server.started,
                clients: server.clients.load(Ordering::SeqCst),
            },
        ),
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
        other => Response::error(
            id,
            ErrorCode::BadRequest,
            format!("this abcd can't answer {other:?} yet"),
        ),
    }
}

/// Seconds since the Unix epoch.
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}
