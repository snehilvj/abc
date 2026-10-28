//! The commands besides the TUI: `abc server …`, `abc config …` and `abc
//! logs`. Output is plain text without icons, so it pipes and greps cleanly.
//! `server status`, `server stop`, `config` and `logs` never start abcd.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use abc_core::build::BUILD_ID;
use abc_core::client::{self, Client};
use abc_core::paths::Paths;
use abc_core::protocol::{Call, ServerStatus};
use abc_core::settings::Settings;
use anyhow::Context as _;

#[derive(Debug, clap::Subcommand)]
pub enum Server {
    /// Whether abcd is running, and which build. Never starts it.
    Status,
    /// Stop abcd if it's running, then start the one beside abc.
    Restart,
    /// Stop abcd. Never starts it.
    Stop,
}

#[derive(Debug, clap::Subcommand)]
pub enum Config {
    /// Where this profile's settings file is.
    Path,
    /// Check the settings file, keys included.
    Check,
    /// Print every option with its docs and default, as a settings file to start from.
    Template,
}

#[derive(Debug, clap::Args)]
pub struct Logs {
    /// abc's own log instead of abcd's.
    #[arg(long)]
    pub client: bool,
    /// How many of the last lines to show.
    #[arg(short = 'n', long, default_value_t = 50)]
    pub lines: usize,
    /// Keep showing lines as they're written.
    #[arg(short, long)]
    pub follow: bool,
}

/// How often `logs --follow` looks for new lines.
const FOLLOW: Duration = Duration::from_millis(200);

pub async fn server(command: Server, paths: &Paths) -> anyhow::Result<ExitCode> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let profile = paths.profile.name();
    let not_running = || println!("abcd isn't running · profile {profile}");
    match command {
        Server::Status => {
            let Ok(client) = Client::connect(&paths.socket(), &client::hello()).await else {
                not_running();
                return Ok(ExitCode::FAILURE);
            };
            let status: ServerStatus = client.call(Call::ServerStatus).await?;
            let socket = tilde(&paths.socket(), home.as_deref());
            print!(
                "{}",
                status_text(&status, client.server.protocol, &socket, now())
            );
        }
        Server::Stop => match client::stop_running(paths).await {
            Some(pid) => println!("stopped abcd · profile {profile} · pid {pid}"),
            None => not_running(),
        },
        Server::Restart => {
            let was = client::stop_running(paths).await;
            let client = client::restart(paths, &sibling()?, &client::hello()).await?;
            let pid = client.server.pid.unwrap_or_default();
            let verb = if was.is_some() {
                "restarted"
            } else {
                "started"
            };
            println!("{verb} abcd · profile {profile} · pid {pid}");
        }
    }
    Ok(ExitCode::SUCCESS)
}

pub fn config(command: Config, paths: &Paths) -> ExitCode {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let file = paths.config_file();
    let shown = tilde(&file, home.as_deref());
    match command {
        Config::Path => println!("{shown}"),
        Config::Template => print!(
            "{}",
            Settings::template(paths.profile.name(), Path::new(&shown))
        ),
        Config::Check => match check_settings(&file) {
            Ok(_) => println!("ok {shown}"),
            Err(errors) => {
                for error in errors {
                    eprintln!("error: {error}");
                }
                return ExitCode::FAILURE;
            }
        },
    }
    ExitCode::SUCCESS
}

/// The settings in `file`, keys and all, or everything wrong with them.
pub fn check_settings(file: &Path) -> Result<Settings, Vec<String>> {
    let settings = Settings::load(file).map_err(|e| vec![Settings::describe(&e)])?;
    crate::keys::Keymap::from_settings(&settings.keys)?;
    Ok(settings)
}

/// The abcd binary beside this abc.
pub fn sibling() -> anyhow::Result<PathBuf> {
    let me = std::env::current_exe().context("finding abc's own path")?;
    Ok(me.with_file_name("abcd"))
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// `abc server status`'s report.
pub fn status_text(status: &ServerStatus, protocol: u32, socket: &str, now: u64) -> String {
    let build = if status.build == BUILD_ID {
        format!("{} (same as this abc)", status.build)
    } else {
        format!("{} (this abc is {BUILD_ID})", status.build)
    };
    let uptime = uptime(now.saturating_sub(status.started));
    let rows = [
        ("pid", status.pid.to_string()),
        ("build", build),
        ("protocol", protocol.to_string()),
        ("uptime", uptime),
        ("socket", socket.to_string()),
    ];
    let mut text = format!("abcd is running · profile {}\n", status.profile);
    for (label, value) in rows {
        text.push_str(&format!("  {label:<9} {value}\n"));
    }
    text
}

/// "45s", "12m", "2h 14m", "3d 2h".
pub fn uptime(secs: u64) -> String {
    let (m, h, d) = (secs / 60, secs / 3600, secs / 86_400);
    match secs {
        0..60 => format!("{secs}s"),
        60..3600 => format!("{m}m"),
        3600..86_400 => format!("{h}h {}m", m % 60),
        _ => format!("{d}d {}h", h % 24),
    }
}

/// `path` with the home folder as `~`.
pub fn tilde(path: &Path, home: Option<&Path>) -> String {
    match home.and_then(|home| path.strip_prefix(home).ok()) {
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

/// `abc logs`: the last lines of abcd's log, or abc's, read straight from the
/// file so they're there when abcd isn't.
pub fn logs(args: &Logs, paths: &Paths) -> anyhow::Result<ExitCode> {
    let path = if args.client {
        paths.client_log()
    } else {
        paths.server_log()
    };
    let Ok(file) = File::open(&path) else {
        eprintln!("no log yet at {}", path.display());
        return Ok(ExitCode::FAILURE);
    };
    let mut reader = BufReader::new(file);
    let mut out = io::stdout().lock();
    for line in last_lines(&mut reader, args.lines)? {
        writeln!(out, "{line}")?;
    }
    if !args.follow {
        return Ok(ExitCode::SUCCESS);
    }
    drop(out);
    follow(reader, &path)
}

/// The last `n` lines from `reader`, leaving it at the end.
fn last_lines(reader: &mut impl BufRead, n: usize) -> io::Result<Vec<String>> {
    let mut lines = std::collections::VecDeque::with_capacity(n + 1);
    for line in reader.lines() {
        lines.push_back(line?);
        if lines.len() > n {
            lines.pop_front();
        }
    }
    Ok(lines.into())
}

/// Prints lines as they're written. A log that shrinks was started afresh
/// (abcd keeps the last run's as `.1`): read the new one from its start.
fn follow(mut reader: BufReader<File>, path: &Path) -> anyhow::Result<ExitCode> {
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? > 0 {
            print!("{line}");
            io::stdout().flush()?;
            continue;
        }
        std::thread::sleep(FOLLOW);
        let read = reader.stream_position()?;
        let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(read);
        if size < read {
            reader = BufReader::new(File::open(path)?);
            reader.seek(SeekFrom::Start(0))?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uptime_reads_at_a_glance() {
        assert_eq!(uptime(45), "45s");
        assert_eq!(uptime(12 * 60 + 5), "12m");
        assert_eq!(uptime(2 * 3600 + 14 * 60), "2h 14m");
        assert_eq!(uptime(3 * 86_400 + 2 * 3600), "3d 2h");
    }

    #[test]
    fn home_is_shown_as_a_tilde() {
        let home = Path::new("/Users/me");
        assert_eq!(
            tilde(Path::new("/Users/me/.local/state/abc"), Some(home)),
            "~/.local/state/abc"
        );
        assert_eq!(tilde(Path::new("/tmp/x"), Some(home)), "/tmp/x");
    }

    #[test]
    fn last_lines_keeps_only_the_last() {
        let text = "1\n2\n3\n4\n";
        let lines = last_lines(&mut text.as_bytes(), 2).unwrap();
        assert_eq!(lines, ["3", "4"]);
    }

    #[test]
    fn server_status_reads_like_its_design() {
        let mut status = ServerStatus {
            pid: 4242,
            build: BUILD_ID.into(),
            profile: "dev".into(),
            started: 1_000,
            clients: 2,
        };
        let socket = "~/.local/state/abc/dev/abcd.sock";
        let same = status_text(&status, 1, socket, 1_000 + 2 * 3600 + 14 * 60);
        insta::assert_snapshot!("server-status", same.replace(BUILD_ID, "0.1.0+3f9a2c1e"));
        status.build = "0.0.9+0ddba11".into();
        let other = status_text(&status, 1, socket, 1_045);
        insta::assert_snapshot!(
            "server-status-other-build",
            other.replace(BUILD_ID, "0.1.0+3f9a2c1e")
        );
    }
}
