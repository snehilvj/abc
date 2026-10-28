//! abcd as clients meet it: its own process, one per profile, over its real
//! socket.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use abc_core::build::BUILD_ID;
use abc_core::protocol::{
    Call, Done, Failure, Hello, HelloLine, PROTOCOL, Request, ServerLine, ServerStatus, encode,
};
use serde::de::DeserializeOwned;

/// A home for one test, under /tmp: macOS caps socket paths at 104 bytes.
fn home() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("abcd-")
        .tempdir_in("/tmp")
        .unwrap()
}

/// abcd, killed when the test ends however it ends.
struct Abcd(Child);

impl Drop for Abcd {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

impl Abcd {
    fn start(home: &Path, profile: &str, args: &[&str]) -> Self {
        let child = Command::new(env!("CARGO_BIN_EXE_abcd"))
            .args(["--profile", profile])
            .args(args)
            .env("HOME", home)
            .env("XDG_STATE_HOME", home.join("state"))
            .env("XDG_CONFIG_HOME", home.join("config"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        Self(child)
    }

    fn pid(&self) -> u32 {
        self.0.id()
    }

    /// Waits for abcd to exit, and says how.
    fn exit(&mut self) -> std::process::ExitStatus {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = self.0.try_wait().unwrap() {
                return status;
            }
            assert!(Instant::now() < deadline, "abcd didn't exit");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn stderr(&mut self) -> String {
        let mut text = String::new();
        if let Some(mut err) = self.0.stderr.take() {
            std::io::Read::read_to_string(&mut err, &mut text).unwrap();
        }
        text
    }
}

fn state_dir(home: &Path, profile: &str) -> PathBuf {
    home.join("state/abc").join(profile)
}

fn socket(home: &Path, profile: &str) -> PathBuf {
    state_dir(home, profile).join("abcd.sock")
}

/// A connection that has said hello.
struct Conn {
    reader: BufReader<UnixStream>,
    writer: UnixStream,
    next: u64,
    server: Hello,
}

/// Connects once abcd listens, saying hello as `protocol`.
fn connect_as(socket: &Path, protocol: u32) -> Conn {
    let deadline = Instant::now() + Duration::from_secs(5);
    let stream = loop {
        match UnixStream::connect(socket) {
            Ok(stream) => break stream,
            Err(_) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            Err(e) => panic!("abcd never listened at {}: {e}", socket.display()),
        }
    };
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut writer = stream.try_clone().unwrap();
    let hello = HelloLine {
        hello: Hello {
            protocol,
            build: BUILD_ID.into(),
            pid: None,
            profile: None,
        },
    };
    writer.write_all(encode(&hello).as_bytes()).unwrap();
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let server: HelloLine = serde_json::from_str(&line).unwrap();
    Conn {
        reader,
        writer,
        next: 1,
        server: server.hello,
    }
}

fn connect(socket: &Path) -> Conn {
    connect_as(socket, PROTOCOL)
}

impl Conn {
    fn call<T: DeserializeOwned>(&mut self, call: Call) -> Result<T, Failure> {
        let id = self.next;
        self.next += 1;
        let request = encode(&Request { id, call });
        self.writer.write_all(request.as_bytes()).unwrap();
        loop {
            let mut line = String::new();
            assert!(
                self.reader.read_line(&mut line).unwrap() > 0,
                "abcd hung up"
            );
            if let ServerLine::Response(response) = serde_json::from_str(&line).unwrap() {
                assert_eq!(response.id, id);
                return response.into_result();
            }
        }
    }

    /// Whether abcd has closed the connection.
    fn closed(&mut self) -> bool {
        let mut line = String::new();
        matches!(self.reader.read_line(&mut line), Ok(0))
    }
}

fn kill(signal: &str, pid: u32) {
    let status = Command::new("kill")
        .args([&format!("-{signal}"), &pid.to_string()])
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
fn says_hello_answers_status_and_shuts_down_when_asked() {
    let home = home();
    let mut abcd = Abcd::start(home.path(), "test", &[]);
    let mut conn = connect(&socket(home.path(), "test"));
    assert_eq!(
        conn.server,
        Hello {
            protocol: PROTOCOL,
            build: BUILD_ID.into(),
            pid: Some(abcd.pid()),
            profile: Some("test".into()),
        }
    );
    let status: ServerStatus = conn.call(Call::ServerStatus).unwrap();
    assert_eq!((status.pid, status.clients), (abcd.pid(), 1));
    assert_eq!(status.profile, "test");
    let _: Done = conn.call(Call::ServerShutdown).unwrap();
    assert!(abcd.exit().success());
    assert!(!socket(home.path(), "test").exists(), "its socket is gone");
}

#[test]
fn one_abcd_per_profile() {
    let home = home();
    let _first = Abcd::start(home.path(), "test", &[]);
    connect(&socket(home.path(), "test"));
    let mut second = Abcd::start(home.path(), "test", &[]);
    assert!(!second.exit().success());
    assert!(second.stderr().contains("already running"));
    let _other = Abcd::start(home.path(), "other", &[]);
    connect(&socket(home.path(), "other"));
}

#[test]
fn keeps_its_state_folder_private() {
    let home = home();
    let _abcd = Abcd::start(home.path(), "test", &[]);
    connect(&socket(home.path(), "test"));
    let mode = std::fs::metadata(state_dir(home.path(), "test"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o700);
}

#[test]
fn replaces_a_socket_left_by_a_crash() {
    let home = home();
    std::fs::create_dir_all(state_dir(home.path(), "test")).unwrap();
    std::fs::write(socket(home.path(), "test"), "").unwrap();
    let _abcd = Abcd::start(home.path(), "test", &[]);
    let mut conn = connect(&socket(home.path(), "test"));
    let _: ServerStatus = conn.call(Call::ServerStatus).unwrap();
}

#[test]
fn exits_when_its_socket_is_removed() {
    let home = home();
    let mut abcd = Abcd::start(home.path(), "test", &[]);
    connect(&socket(home.path(), "test"));
    std::fs::remove_file(socket(home.path(), "test")).unwrap();
    assert!(abcd.exit().success());
}

#[test]
fn ignores_sighup_and_stops_cleanly_on_sigterm() {
    let home = home();
    let mut abcd = Abcd::start(home.path(), "test", &[]);
    let mut conn = connect(&socket(home.path(), "test"));
    kill("HUP", abcd.pid());
    let _: ServerStatus = conn.call(Call::ServerStatus).unwrap();
    kill("TERM", abcd.pid());
    assert!(abcd.exit().success());
    assert!(!socket(home.path(), "test").exists());
}

#[test]
fn hangs_up_on_a_client_of_another_protocol() {
    let home = home();
    let _abcd = Abcd::start(home.path(), "test", &[]);
    let mut conn = connect_as(&socket(home.path(), "test"), PROTOCOL + 1);
    assert_eq!(conn.server.protocol, PROTOCOL, "it still says who it is");
    assert!(conn.closed());
}

#[test]
fn reports_its_build_id() {
    let out = Command::new(env!("CARGO_BIN_EXE_abcd"))
        .arg("--print-build-id")
        .output()
        .unwrap();
    assert_eq!(String::from_utf8(out.stdout).unwrap().trim(), BUILD_ID);
    let home = home();
    let _abcd = Abcd::start(home.path(), "test", &["--build-id", "0.0.0+feedface"]);
    let conn = connect(&socket(home.path(), "test"));
    assert_eq!(conn.server.build, "0.0.0+feedface");
}
