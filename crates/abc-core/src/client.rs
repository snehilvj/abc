//! Talking to abcd: connecting, starting it when it isn't running, and what
//! to do when it's another build.
//!
//! At startup, if abcd's protocol or build differ from ours, abc asks its
//! sibling `abcd --print-build-id`. If that differs from the running abcd,
//! abcd is stale: abc restarts it, once. If it matches, abc is the stale one:
//! it carries on with a warning when the protocol matches, and asks to be
//! reopened when it doesn't. A client reconnecting mid-session never restarts
//! abcd, so two builds never take turns stopping each other.

use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde::de::DeserializeOwned;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::net::unix::OwnedWriteHalf;
use tokio::sync::{Mutex, mpsc, oneshot};

use crate::build::BUILD_ID;
use crate::paths::Paths;
use crate::protocol::{
    Call, ErrorCode, Event, Failure, Hello, HelloLine, PROTOCOL, Request, Response, ServerLine,
    encode,
};

/// How long a started abcd has to answer.
const START_TIMEOUT: Duration = Duration::from_secs(3);
/// How often to try connecting while it starts or stops.
const RETRY: Duration = Duration::from_millis(20);

type Pending = Arc<std::sync::Mutex<HashMap<u64, oneshot::Sender<Response>>>>;

/// A connection to abcd that has said hello. Requests can be made from
/// several tasks at once; events arrive on [`Client::events`].
pub struct Client {
    /// abcd's hello.
    pub server: Hello,
    writer: Mutex<OwnedWriteHalf>,
    next: AtomicU64,
    pending: Pending,
    events: std::sync::Mutex<Option<mpsc::UnboundedReceiver<Event>>>,
}

impl Client {
    /// Connects to the abcd listening at `socket`, saying `me` as hello.
    pub async fn connect(socket: &Path, me: &Hello) -> io::Result<Self> {
        let stream = UnixStream::connect(socket).await?;
        let (read, mut writer) = stream.into_split();
        let hello = HelloLine { hello: me.clone() };
        writer.write_all(encode(&hello).as_bytes()).await?;
        let mut lines = BufReader::new(read).lines();
        let line = lines
            .next_line()
            .await?
            .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "abcd hung up"))?;
        let server: HelloLine = serde_json::from_str(&line).map_err(|e| {
            io::Error::new(io::ErrorKind::InvalidData, format!("abcd's hello: {e}"))
        })?;

        let pending: Pending = Arc::default();
        let (events, received) = mpsc::unbounded_channel();
        let waiting = pending.clone();
        tokio::spawn(async move {
            while let Ok(Some(line)) = lines.next_line().await {
                match serde_json::from_str(&line) {
                    Ok(ServerLine::Response(response)) => {
                        let asked = waiting.lock().unwrap().remove(&response.id);
                        if let Some(asked) = asked {
                            let _ = asked.send(response);
                        }
                    }
                    Ok(ServerLine::Event(event)) => {
                        let _ = events.send(event);
                    }
                    Err(_) => {}
                }
            }
            // Hung up: whoever is still waiting hears so as their senders drop.
            waiting.lock().unwrap().clear();
        });
        Ok(Self {
            server: server.hello,
            writer: Mutex::new(writer),
            next: AtomicU64::new(1),
            pending,
            events: std::sync::Mutex::new(Some(received)),
        })
    }

    /// Asks abcd, and waits for its answer as `T`.
    pub async fn call<T: DeserializeOwned>(&self, call: Call) -> Result<T, Failure> {
        self.call_as(self.reserve(), call).await
    }

    /// An id for a request not made yet, to know it before its events arrive.
    pub fn reserve(&self) -> u64 {
        self.next.fetch_add(1, Ordering::SeqCst)
    }

    /// Asks abcd under `id`, from [`Client::reserve`], and waits for its answer.
    pub async fn call_as<T: DeserializeOwned>(&self, id: u64, call: Call) -> Result<T, Failure> {
        let (answer, answered) = oneshot::channel();
        self.pending.lock().unwrap().insert(id, answer);
        let line = encode(&Request { id, call });
        if let Err(e) = self.writer.lock().await.write_all(line.as_bytes()).await {
            self.pending.lock().unwrap().remove(&id);
            return Err(hung_up(e.to_string()));
        }
        match answered.await {
            Ok(response) => response.into_result(),
            Err(_) => Err(hung_up("abcd hung up".into())),
        }
    }

    /// Events abcd pushes, until it hangs up; the first caller gets them.
    pub fn events(&self) -> Option<mpsc::UnboundedReceiver<Event>> {
        self.events.lock().unwrap().take()
    }

    /// Whether abcd is this very build.
    fn matches(&self, me: &Hello) -> bool {
        self.server.protocol == me.protocol && self.server.build == me.build
    }
}

fn hung_up(message: String) -> Failure {
    Failure {
        code: ErrorCode::Io,
        message,
    }
}

/// This build's hello.
pub fn hello() -> Hello {
    Hello {
        protocol: PROTOCOL,
        build: BUILD_ID.into(),
        pid: None,
        profile: None,
    }
}

/// How connecting at startup went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Startup {
    /// abcd is this build. `started` if it wasn't running and abc started it.
    Ready { started: bool },
    /// abcd was an older or newer build than the abcd beside abc, so abc
    /// restarted it.
    Restarted { from: String },
    /// abc is an older or newer build than abcd but speaks its protocol: it
    /// works, but should be reopened.
    StaleClient,
}

#[derive(Debug, thiserror::Error)]
pub enum StartError {
    #[error("abcd didn't start: {0}")]
    StartFailed(String),
    /// abc should be reopened to use the running abcd.
    #[error("abc speaks protocol {ours}, abcd speaks protocol {theirs}")]
    Incompatible { ours: u32, theirs: u32 },
}

/// Connects at startup as `me`, starting `abcd` if it isn't running and
/// restarting it, once, if it's stale.
pub async fn start(
    paths: &Paths,
    abcd: &Path,
    me: &Hello,
) -> Result<(Client, Startup), StartError> {
    let socket = paths.socket();
    let (client, started) = match Client::connect(&socket, me).await {
        Ok(client) => (client, false),
        Err(_) => (launch(paths, abcd, me).await?, true),
    };
    if client.matches(me) {
        return Ok((client, Startup::Ready { started }));
    }
    let sibling = sibling_build(abcd)?;
    if sibling != client.server.build {
        let from = client.server.build.clone();
        stop(&client.server, &socket).await;
        drop(client);
        let client = launch(paths, abcd, me).await?;
        return judge(client, me, Some(from));
    }
    judge(client, me, None)
}

/// The protocol must match. Then abcd is either this build, after a
/// restart, or abc is the stale one.
fn judge(
    client: Client,
    me: &Hello,
    restarted_from: Option<String>,
) -> Result<(Client, Startup), StartError> {
    if client.server.protocol != me.protocol {
        return Err(StartError::Incompatible {
            ours: me.protocol,
            theirs: client.server.protocol,
        });
    }
    let startup = match restarted_from {
        Some(from) if client.server.build == me.build => Startup::Restarted { from },
        _ => Startup::StaleClient,
    };
    Ok((client, startup))
}

/// What `abcd --print-build-id` says.
fn sibling_build(abcd: &Path) -> Result<String, StartError> {
    let out = Command::new(abcd)
        .arg("--print-build-id")
        .output()
        .map_err(|e| StartError::StartFailed(format!("running {}: {e}", abcd.display())))?;
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Starts `abcd` for the profile, detached in its own process group so it
/// outlives abc, and connects once it answers.
async fn launch(paths: &Paths, abcd: &Path, me: &Hello) -> Result<Client, StartError> {
    use std::os::unix::process::CommandExt;
    Command::new(abcd)
        .args(["--profile", paths.profile.name()])
        // The same folders as abc, however abc found them.
        .env("XDG_STATE_HOME", paths.state_home())
        .env("XDG_CONFIG_HOME", paths.config_home())
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| StartError::StartFailed(format!("running {}: {e}", abcd.display())))
        // Reaped when it exits, so a long-lived abc collects no zombie.
        .map(|mut child| std::thread::spawn(move || child.wait()))?;
    let socket = paths.socket();
    let deadline = tokio::time::Instant::now() + START_TIMEOUT;
    loop {
        match Client::connect(&socket, me).await {
            Ok(client) => return Ok(client),
            Err(_) if tokio::time::Instant::now() < deadline => tokio::time::sleep(RETRY).await,
            Err(e) => {
                let log = paths.server_log();
                return Err(StartError::StartFailed(format!(
                    "no answer in {}s ({e}); see {}",
                    START_TIMEOUT.as_secs(),
                    log.display()
                )));
            }
        }
    }
}

/// Stops the abcd that said `hello` and waits until its socket is gone. A
/// SIGTERM to the pid in its hello, which never changes shape, so this works
/// on an abcd of any protocol.
async fn stop(hello: &Hello, socket: &Path) {
    if let Some(pid) = hello.pid {
        let _ = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .stderr(Stdio::null())
            .status();
    }
    let deadline = tokio::time::Instant::now() + START_TIMEOUT;
    while socket.exists() && tokio::time::Instant::now() < deadline {
        tokio::time::sleep(RETRY).await;
    }
}

/// Connects again after the connection dropped: to the abcd running, or one
/// started if none is. Never restarts one, so two builds never take turns
/// stopping each other.
pub async fn reconnect(paths: &Paths, abcd: &Path, me: &Hello) -> Result<Client, StartError> {
    match Client::connect(&paths.socket(), me).await {
        Ok(client) => Ok(client),
        Err(_) => launch(paths, abcd, me).await,
    }
}

/// Stops the profile's abcd if it's running, and says which process it was.
pub async fn stop_running(paths: &Paths) -> Option<u32> {
    let socket = paths.socket();
    let client = Client::connect(&socket, &hello()).await.ok()?;
    stop(&client.server, &socket).await;
    client.server.pid
}

/// Stops the profile's abcd if it's running, then starts `abcd`.
pub async fn restart(paths: &Paths, abcd: &Path, me: &Hello) -> Result<Client, StartError> {
    stop_running(paths).await;
    launch(paths, abcd, me).await
}
