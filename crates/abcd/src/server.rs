//! Listening on the profile's socket and serving each connection: hello
//! first, then requests answered in turn.

use std::fs::{self, DirBuilder, File, TryLockError};
use std::os::unix::fs::DirBuilderExt;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use abc_core::paths::Paths;
use abc_core::protocol::{
    Call, Done, ErrorCode, Hello, HelloLine, PROTOCOL, Request, Response, ServerStatus, encode,
};
use anyhow::{Context as _, bail};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::{Notify, mpsc};
use tracing::{info, warn};

/// How often abcd checks its socket is still there.
const SOCKET_CHECK: Duration = Duration::from_secs(1);

/// What every connection shares.
struct Server {
    build: String,
    profile: String,
    /// Seconds since the Unix epoch.
    started: u64,
    clients: AtomicUsize,
    /// Notified when a client asks abcd to stop.
    stop: Notify,
}

/// Serves the profile until asked to stop, signalled, or its socket goes.
pub async fn run(paths: Paths, build: String) -> anyhow::Result<()> {
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
    match lock.try_lock() {
        Ok(()) => {}
        Err(TryLockError::WouldBlock) => bail!(
            "abcd is already running for profile {}",
            paths.profile.name()
        ),
        Err(TryLockError::Error(e)) => return Err(e).context("locking the lock file"),
    }
    let _log = crate::log::start(&paths)?;

    // Holding the lock, no other abcd owns a socket here: one left behind is stale.
    let socket = paths.socket();
    let _ = fs::remove_file(&socket);
    let listener = UnixListener::bind(&socket)
        .with_context(|| format!("listening on {}", socket.display()))?;
    info!(pid = std::process::id(), build, "abcd started");

    let server = Arc::new(Server {
        build,
        profile: paths.profile.name().into(),
        started: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
        clients: AtomicUsize::new(0),
        stop: Notify::new(),
    });
    let mut term = signal(SignalKind::terminate())?;
    let mut interrupt = signal(SignalKind::interrupt())?;
    // Handled, so closing the terminal that started abcd doesn't stop it.
    let mut hangup = signal(SignalKind::hangup())?;
    let mut check = tokio::time::interval(SOCKET_CHECK);
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
            protocol: PROTOCOL,
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
        Some(HelloLine { hello }) if hello.protocol == PROTOCOL => {}
        other => {
            info!("hanging up on a client that said {other:?}");
            return;
        }
    }
    let _client = Client::new(&server.clients);

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
        let _ = out.send(encode(&answer(&server, request)));
        if stop {
            // Let the answer go out before stopping.
            drop(out);
            let _ = writer.await;
            server.stop.notify_one();
            return;
        }
    }
    drop(out);
    let _ = writer.await;
}

fn answer(server: &Server, request: Request) -> Response {
    let id = request.id;
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
        Call::ServerShutdown => Response::ok(id, &Done {}),
        other => Response::error(
            id,
            ErrorCode::BadRequest,
            format!("this abcd can't answer {other:?} yet"),
        ),
    }
}
