//! The repositories a profile has added, kept in `state.json`, and what git
//! says about each now.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use abc_core::model::{GitStatus, Head, Repo, RepoCheck, RepoState, Worktree};
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tracing::warn;

use abc_core::source::same_remote;

use crate::git::{self, Listed};

/// What abcd keeps between runs.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Saved {
    /// In the order they were added.
    pub repos: Vec<Added>,
    /// When a client last opened each worktree: seconds since the Unix epoch.
    #[serde(default)]
    pub visits: BTreeMap<PathBuf, u64>,
}

/// A repository as it was added.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Added {
    pub name: String,
    /// The main worktree, or a bare repository's folder.
    pub path: PathBuf,
    /// git's common folder, which every worktree of the repository shares.
    pub git_dir: PathBuf,
}

impl Saved {
    /// Reads `state.json`; none yet is an empty registry.
    pub fn load(file: &Path) -> anyhow::Result<Self> {
        match std::fs::read(file) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| {
                anyhow::anyhow!(
                    "{} is damaged ({e}); move it aside to start afresh",
                    file.display()
                )
            }),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(anyhow::anyhow!("reading {}: {e}", file.display())),
        }
    }

    /// Writes `state.json` whole or not at all: a new file, then a rename.
    pub fn save(&self, file: &Path) -> io::Result<()> {
        let mut temporary = file.as_os_str().to_owned();
        temporary.push(".new");
        let json = serde_json::to_vec_pretty(self).expect("state serializes");
        std::fs::write(&temporary, json)?;
        std::fs::rename(&temporary, file)
    }

    pub fn named(&self, name: &str) -> Option<&Added> {
        self.repos.iter().find(|r| r.name == name)
    }
}

/// `text` as a path: `~` is the home folder, and a relative path is taken
/// from it too.
pub fn expand(text: &str, home: &Path) -> PathBuf {
    let text = text.trim();
    let path = match text.strip_prefix('~') {
        Some(rest) => home.join(rest.trim_start_matches('/')),
        None => PathBuf::from(text),
    };
    if path.is_absolute() {
        path
    } else {
        home.join(path)
    }
}

/// git, run in `dir`, never asking for a password.
fn git_in(dir: &Path) -> Command {
    let mut git = Command::new("git");
    git.arg("-C").arg(dir).env("GIT_TERMINAL_PROMPT", "0");
    git.kill_on_drop(true);
    git
}

/// git's output, or what it said went wrong.
async fn run(mut git: Command) -> Result<Vec<u8>, String> {
    let out = git
        .output()
        .await
        .map_err(|e| format!("running git: {e}"))?;
    if out.status.success() {
        Ok(out.stdout)
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

/// The repository `folder` belongs to: its main worktree (or bare folder)
/// and common git folder. None if it isn't in a git repository.
pub async fn resolve(folder: &Path) -> Option<(PathBuf, PathBuf)> {
    let mut common = git_in(folder);
    common.args(["rev-parse", "--path-format=absolute", "--git-common-dir"]);
    let git_dir = String::from_utf8(run(common).await.ok()?).ok()?;
    let git_dir = std::fs::canonicalize(git_dir.trim()).ok()?;
    let listed = worktrees(folder).await.ok()?;
    let main = listed.first()?.path.clone();
    Some((main, git_dir))
}

async fn worktrees(folder: &Path) -> Result<Vec<Listed>, String> {
    let mut list = git_in(folder);
    list.args(["worktree", "list", "--porcelain", "-z"]);
    Ok(git::parse_worktrees(&run(list).await?))
}

/// What adding `folder` would do, against what's already added.
pub async fn check_folder(folder: &Path, saved: &Saved) -> RepoCheck {
    if !folder.exists() {
        return RepoCheck::NotFound;
    }
    let Some((_, git_dir)) = resolve(folder).await else {
        return RepoCheck::NotGit;
    };
    if let Some(added) = saved.repos.iter().find(|r| r.git_dir == git_dir) {
        return RepoCheck::AlreadyAdded {
            name: added.name.clone(),
        };
    }
    match worktrees(folder).await {
        Ok(listed) => RepoCheck::Ready {
            worktrees: listed.iter().filter(|l| !l.bare).count(),
        },
        Err(_) => RepoCheck::NotGit,
    }
}

/// How long git may take to reach a remote before abcd gives up on it.
const REMOTE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

/// What adding git URL `url` would do, cloning it into `into` (written for
/// people as `shown`), against what's already added.
pub async fn check_url(url: &str, into: &Path, shown: &str, saved: &Saved) -> RepoCheck {
    for added in &saved.repos {
        if remotes(&added.path)
            .await
            .iter()
            .any(|r| same_remote(r, url))
        {
            return RepoCheck::AlreadyAdded {
                name: added.name.clone(),
            };
        }
    }
    if into.exists() {
        return RepoCheck::TargetExists { into: shown.into() };
    }
    let mut reach = Command::new("git");
    reach
        .args(["ls-remote", "--heads", "--", url])
        .env("GIT_TERMINAL_PROMPT", "0")
        .kill_on_drop(true);
    match tokio::time::timeout(REMOTE_TIMEOUT, run(reach)).await {
        Ok(Ok(_)) => RepoCheck::Remote { into: shown.into() },
        _ => RepoCheck::NotFound,
    }
}

/// The URLs of a repository's remotes.
async fn remotes(folder: &Path) -> Vec<String> {
    let mut config = git_in(folder);
    config.args(["config", "--get-regexp", r"^remote\..*\.url$"]);
    let Ok(out) = run(config).await else {
        return Vec::new();
    };
    String::from_utf8_lossy(&out)
        .lines()
        .filter_map(|line| line.split_once(' ').map(|(_, url)| url.to_string()))
        .collect()
}

/// Clones `url` into `into`, making the folders above it as needed.
pub async fn clone(url: &str, into: &Path) -> Result<(), String> {
    if let Some(parent) = into.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("creating {}: {e}", parent.display()))?;
    }
    let mut clone = Command::new("git");
    clone
        .args(["clone", "--quiet", "--", url])
        .arg(into)
        .env("GIT_TERMINAL_PROMPT", "0")
        .kill_on_drop(true);
    run(clone).await.map(|_| ())
}

/// The name a repository gets unless one is given: its folder's, without `.git`.
pub fn default_name(main: &Path) -> String {
    let name = main
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    name.strip_suffix(".git").map(String::from).unwrap_or(name)
}

/// The repository as it is now: its worktrees, or missing if its folder or
/// git have gone.
pub async fn scan(added: &Added, visits: &BTreeMap<PathBuf, u64>) -> Repo {
    let missing = Repo {
        name: added.name.clone(),
        path: added.path.clone(),
        state: RepoState::Missing,
    };
    if !added.path.exists() {
        return missing;
    }
    let listed = match worktrees(&added.path).await {
        Ok(listed) => listed,
        Err(why) => {
            warn!("listing {}'s worktrees: {why}", added.name);
            return missing;
        }
    };
    let mut worktrees = Vec::new();
    for (i, listed) in listed.iter().enumerate().filter(|(_, l)| !l.bare) {
        let mut worktree = worktree(listed, i == 0, visits);
        if !worktree.missing {
            worktree.git = status(&worktree.path).await;
        }
        worktrees.push(worktree);
    }
    Repo {
        state: RepoState::Present { worktrees },
        ..missing
    }
}

/// A worktree's branch status and changes, if git can say.
async fn status(worktree: &Path) -> Option<GitStatus> {
    let mut status = git_in(worktree);
    status.args(["status", "--porcelain=v2", "--branch", "-z"]);
    run(status).await.ok().map(|out| git::parse_status(&out))
}

fn worktree(listed: &Listed, is_main: bool, visits: &BTreeMap<PathBuf, u64>) -> Worktree {
    let head = match (&listed.branch, &listed.head) {
        (Some(branch), _) => Head::Branch(branch.clone()),
        (None, Some(commit)) => Head::Detached(commit.chars().take(7).collect()),
        (None, None) => Head::Detached(String::new()),
    };
    Worktree {
        path: listed.path.clone(),
        name: default_name(&listed.path),
        head,
        is_main,
        locked: listed.locked,
        missing: listed.prunable || !listed.path.exists(),
        git: None,
        pr: None,
        last_opened: visits.get(&listed.path).copied(),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// git with no user or system config, so tests never see yours.
    pub fn git(dir: &Path, args: &[&str]) {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args([
                "-c",
                "user.name=abc",
                "-c",
                "user.email=abc@example.com",
                "-c",
                "init.defaultBranch=main",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// A repository at `dir/name` with one commit, and linked worktrees.
    pub fn repo(dir: &Path, name: &str, linked: &[&str]) -> PathBuf {
        let path = dir.join(name);
        std::fs::create_dir_all(&path).unwrap();
        git(&path, &["init", "-q"]);
        git(&path, &["commit", "-q", "--allow-empty", "-m", "first"]);
        for branch in linked {
            let to = dir.join(format!("{name}-{}", branch.replace('/', "-")));
            git(
                &path,
                &["worktree", "add", "-q", "-b", branch, to.to_str().unwrap()],
            );
        }
        std::fs::canonicalize(path).unwrap()
    }

    fn added(name: &str, path: &Path, git_dir: PathBuf) -> Added {
        Added {
            name: name.into(),
            path: path.into(),
            git_dir,
        }
    }

    #[test]
    fn paths_expand_from_home() {
        let home = Path::new("/home/me");
        assert_eq!(expand("~/code/x", home), PathBuf::from("/home/me/code/x"));
        assert_eq!(expand(" ~ ", home), PathBuf::from("/home/me"));
        assert_eq!(expand("code/x", home), PathBuf::from("/home/me/code/x"));
        assert_eq!(expand("/srv/x", home), PathBuf::from("/srv/x"));
    }

    #[test]
    fn state_is_saved_whole_and_read_back() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("state.json");
        assert_eq!(Saved::load(&file).unwrap(), Saved::default(), "none yet");
        let mut saved = Saved::default();
        saved.repos.push(added(
            "app",
            Path::new("/code/app"),
            "/code/app/.git".into(),
        ));
        saved.visits.insert("/code/app".into(), 42);
        saved.save(&file).unwrap();
        assert_eq!(Saved::load(&file).unwrap(), saved);
        std::fs::write(&file, "{").unwrap();
        assert!(
            Saved::load(&file)
                .unwrap_err()
                .to_string()
                .contains("damaged")
        );
    }

    #[tokio::test]
    async fn a_folder_is_checked_before_it_is_added() {
        let dir = tempfile::tempdir().unwrap();
        let app = repo(dir.path(), "app", &["feat/x"]);
        let mut saved = Saved::default();
        assert_eq!(
            check_folder(&app, &saved).await,
            RepoCheck::Ready { worktrees: 2 }
        );
        assert_eq!(
            check_folder(&dir.path().join("nope"), &saved).await,
            RepoCheck::NotFound
        );
        assert_eq!(check_folder(dir.path(), &saved).await, RepoCheck::NotGit);

        let (main, git_dir) = resolve(&app.join("..").join("app-feat-x")).await.unwrap();
        assert_eq!(main, app, "a linked worktree resolves to its main one");
        saved.repos.push(added("app", &main, git_dir));
        let linked = dir.path().join("app-feat-x");
        assert_eq!(
            check_folder(&linked, &saved).await,
            RepoCheck::AlreadyAdded { name: "app".into() },
            "already added, through a worktree"
        );
    }

    #[tokio::test]
    async fn a_scan_lists_worktrees_and_notices_what_went() {
        let dir = tempfile::tempdir().unwrap();
        let app = repo(dir.path(), "app", &["feat/x"]);
        let (_, git_dir) = resolve(&app).await.unwrap();
        let entry = added("app", &app, git_dir);
        let linked = std::fs::canonicalize(dir.path().join("app-feat-x")).unwrap();
        let visits = BTreeMap::from([(linked.clone(), 7)]);
        let repo = scan(&entry, &visits).await;
        let worktrees = repo.worktrees();
        assert_eq!(worktrees.len(), 2);
        assert!(worktrees[0].is_main && worktrees[0].head == Head::Branch("main".into()));
        assert_eq!(worktrees[1].name, "app-feat-x");
        assert_eq!(worktrees[1].last_opened, Some(7));

        std::fs::remove_dir_all(&linked).unwrap();
        let repo = scan(&entry, &visits).await;
        assert!(repo.worktrees()[1].missing, "a worktree whose folder went");
        std::fs::remove_dir_all(&app).unwrap();
        assert!(scan(&entry, &visits).await.is_missing());
    }

    #[tokio::test]
    async fn a_scan_reads_each_worktrees_git_status() {
        let dir = tempfile::tempdir().unwrap();
        let app = repo(dir.path(), "app", &[]);
        std::fs::write(app.join("new.txt"), "x").unwrap();
        let (_, git_dir) = resolve(&app).await.unwrap();
        let repo = scan(&added("app", &app, git_dir), &BTreeMap::new()).await;
        let status = repo.worktrees()[0].git.expect("git status");
        assert_eq!((status.changed, status.upstream), (1, false));
    }

    #[test]
    fn names_default_to_the_folder() {
        assert_eq!(default_name(Path::new("/code/app")), "app");
        assert_eq!(default_name(Path::new("/code/app.git")), "app");
    }
}
