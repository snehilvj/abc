//! The commands besides the TUI: `abc repo …`, `abc server …`, `abc config
//! …` and `abc logs`. Output is plain text without icons, so it pipes and greps cleanly.
//! `server status`, `server stop`, `config` and `logs` never start abcd.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use abc_core::build::BUILD_ID;
use abc_core::client::{self, Client};
use abc_core::model::Repo;
use abc_core::paths::Paths;
use abc_core::protocol::{self, Call, Github, Repos, ServerStatus};
use abc_core::settings::Settings;
use anyhow::Context as _;

#[derive(Debug, clap::Subcommand)]
pub enum RepoCommand {
    /// The repositories added, in the order they were added.
    List,
    /// Add the repository at a folder, or clone one from a git URL and add it.
    Add {
        /// A folder on this machine, or a git URL.
        path: String,
        /// The name to add it under; by default the folder's or the repository's.
        #[arg(long)]
        name: Option<String>,
    },
    /// Forget a repository. Its files stay on disk.
    Remove { name: String },
}

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
    /// Print a theme (dark, light, ansi, or one of yours) as a file to copy to
    /// `themes/<name>.toml` and change.
    Theme { name: String },
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
        Config::Theme { name } => match crate::theme::Theme::named(&name, &paths.config_dir) {
            Ok(theme) => print!("{}", theme.to_toml()),
            Err(why) => {
                eprintln!("error: {why}");
                return ExitCode::FAILURE;
            }
        },
        Config::Check => match check_settings(&file).and_then(|settings| {
            // The theme file too, apart: a mistake there leaves the other settings be.
            let built_in = settings.theme == "auto"
                || crate::theme::Theme::built_in(&settings.theme).is_some();
            if !built_in {
                crate::theme::Theme::named(&settings.theme, &paths.config_dir)
                    .map_err(|why| vec![why])?;
            }
            Ok(settings)
        }) {
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
    let keys = crate::keys::Keymap::from_settings(&settings.keys).err();
    let icons = crate::icons::with_overrides(&settings.icons.glyphs).err();
    let errors: Vec<String> = keys.into_iter().chain(icons).flatten().collect();
    if errors.is_empty() {
        Ok(settings)
    } else {
        Err(errors)
    }
}

/// `abc repo …`, through abcd, starting it if need be.
pub async fn repo(command: RepoCommand, paths: &Paths) -> anyhow::Result<ExitCode> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let client = connect(paths).await?;
    let result = match command {
        RepoCommand::List => client
            .call::<Repos>(Call::RepoList)
            .await
            .map(|r| repo_list(&r.repos, home.as_deref())),
        RepoCommand::Add { path, name } => {
            let path = absolute(&path);
            client
                .call::<protocol::Added>(Call::RepoAdd { path, name })
                .await
                .map(|added| repo_added(&added, home.as_deref()))
        }
        RepoCommand::Remove { name } => {
            let path = client
                .call::<Repos>(Call::RepoList)
                .await
                .ok()
                .and_then(|r| r.repos.into_iter().find(|r| r.name == name))
                .map(|r| tilde(&r.path, home.as_deref()));
            client
                .call::<Repos>(Call::RepoRemove { name: name.clone() })
                .await
                .map(|_| repo_removed(&name, path.as_deref()))
                .map_err(|mut failure| {
                    if failure.code == protocol::ErrorCode::NotFound {
                        failure.message.push_str(" (see: abc repo list)");
                    }
                    failure
                })
        }
    };
    match result {
        Ok(text) => {
            print!("{text}");
            Ok(ExitCode::SUCCESS)
        }
        Err(failure) => {
            eprintln!("error: {}", failure.message);
            Ok(ExitCode::FAILURE)
        }
    }
}

/// A folder relative to where abc runs, made absolute so abcd reads it the
/// same; `~` and git URLs go as they are.
fn absolute(path: &str) -> String {
    let path = path.trim();
    if path.starts_with('~') || abc_core::source::is_url(path) {
        return path.into();
    }
    std::path::absolute(path).map_or_else(|_| path.into(), |p| p.display().to_string())
}

/// abcd, started or restarted as need be; a note on stderr if this abc is stale.
async fn connect(paths: &Paths) -> anyhow::Result<Client> {
    use abc_core::client::Startup;
    let (client, startup) = client::start(paths, &sibling()?, &client::hello()).await?;
    match startup {
        Startup::Ready { .. } => {}
        Startup::Restarted { from } => eprintln!("note: restarted abcd, which was build {from}"),
        Startup::StaleClient => eprintln!(
            "note: this abc ({BUILD_ID}) is older or newer than abcd ({}); reopen it soon",
            client.server.build
        ),
    }
    Ok(client)
}

/// `abc repo list`: a table, or how to add the first.
pub fn repo_list(repos: &[Repo], home: Option<&Path>) -> String {
    if repos.is_empty() {
        return "No repositories. Add one with: abc repo add <path>\n".into();
    }
    let width = repos
        .iter()
        .map(|r| r.name.chars().count())
        .max()
        .unwrap_or(0)
        .max(4);
    let mut text = format!("{:<width$}  WORKTREES  PATH\n", "NAME");
    for repo in repos {
        let worktrees = if repo.is_missing() {
            "missing".to_string()
        } else {
            repo.worktrees().len().to_string()
        };
        let path = tilde(&repo.path, home);
        text.push_str(&format!("{:<width$}  {worktrees:<9}  {path}\n", repo.name));
    }
    text
}

/// `abc repo add`'s report.
pub fn repo_added(added: &protocol::Added, home: Option<&Path>) -> String {
    let Some(repo) = added.repos.iter().find(|r| r.name == added.name) else {
        return format!("added {}\n", added.name);
    };
    let worktrees = crate::app::plural(repo.worktrees().len(), "worktree");
    format!(
        "added {} · {} · {worktrees}\n",
        added.name,
        tilde(&repo.path, home)
    )
}

/// `abc repo remove`'s report.
pub fn repo_removed(name: &str, path: Option<&str>) -> String {
    match path {
        Some(path) => format!("removed {name} · files left on disk at {path}\n"),
        None => format!("removed {name}\n"),
    }
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
        (
            "repos",
            format!(
                "{} ({})",
                status.repos,
                crate::app::plural(status.worktrees, "worktree")
            ),
        ),
        ("github", github_text(&status.github)),
    ];
    let mut text = format!("abcd is running · profile {}\n", status.profile);
    for (label, value) in rows {
        text.push_str(&format!("  {label:<9} {value}\n"));
    }
    text
}

/// How pull request status stands, for `abc server status`.
fn github_text(github: &Github) -> String {
    match github {
        Github::On { login } => format!("gh logged in as {login}"),
        Github::Off { why } => why.clone(),
        Github::Disabled => "off (github.enabled = false)".into(),
        Github::Unknown => "not asked yet: abcd asks once abc is open".into(),
    }
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

    fn repos() -> Vec<Repo> {
        let mut repos = crate::fake::repos_with_variants();
        for repo in &mut repos {
            repo.path = Path::new("/Users/me/code").join(&repo.name);
        }
        repos
    }

    #[test]
    fn repo_commands_read_like_their_design() {
        let home = Some(Path::new("/Users/me"));
        insta::assert_snapshot!("repo-list", repo_list(&repos(), home));
        insta::assert_snapshot!("repo-list-empty", repo_list(&[], home));
        let added = protocol::Added {
            name: "ecourts".into(),
            repos: repos(),
        };
        insta::assert_snapshot!("repo-added", repo_added(&added, home));
        insta::assert_snapshot!(
            "repo-removed",
            repo_removed("ecourts", Some("~/code/ecourts"))
        );
    }

    #[test]
    fn relative_folders_are_made_absolute_but_not_urls_or_home() {
        assert!(Path::new(&absolute("code/app")).is_absolute());
        assert_eq!(absolute("~/code/app"), "~/code/app");
        assert_eq!(
            absolute(" git@github.com:o/r.git "),
            "git@github.com:o/r.git"
        );
    }

    #[test]
    fn server_status_reads_like_its_design() {
        let mut status = ServerStatus {
            pid: 4242,
            build: BUILD_ID.into(),
            profile: "dev".into(),
            started: 1_000,
            clients: 2,
            repos: 3,
            worktrees: 7,
            github: Github::On {
                login: "snehilvj".into(),
            },
        };
        let socket = "~/.local/state/abc/dev/abcd.sock";
        let same = status_text(&status, 1, socket, 1_000 + 2 * 3600 + 14 * 60);
        insta::assert_snapshot!("server-status", same.replace(BUILD_ID, "0.1.0+3f9a2c1e"));
        status.build = "0.0.9+0ddba11".into();
        status.github = Github::Off {
            why: "gh is not logged in · PR status is off · run gh auth login".into(),
        };
        let other = status_text(&status, 1, socket, 1_045);
        insta::assert_snapshot!(
            "server-status-other-build",
            other.replace(BUILD_ID, "0.1.0+3f9a2c1e")
        );
    }
}
