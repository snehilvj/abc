//! abcd's state: the registry and the repositories as last scanned, shared by
//! every connection, and the changes pushed to those that subscribed.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use abc_core::model::{Repo, RepoCheck, RepoState};
use abc_core::protocol::{self, Done, ErrorCode, Event, Failure, Github, Repos};
use abc_core::settings::{self, Settings};
use abc_core::source::{self, is_url};
use tokio::process::Command;
use tokio::sync::{Mutex, broadcast};
use tracing::{info, warn};

use crate::github::{self, BranchPr};
use crate::registry::{self, Added, Saved};

/// How long gh may take before abcd gives up on it this time round.
const GH_TIMEOUT: Duration = Duration::from_secs(30);

pub struct State {
    /// `state.json`.
    file: PathBuf,
    /// Where `~` and relative paths start.
    home: PathBuf,
    /// Where git URLs are cloned (`add_repo.clone_dir`), as written.
    clone_dir: String,
    /// The `github` settings.
    github: settings::Github,
    /// Set while pull requests are being fetched, so fetches never overlap.
    fetching: AtomicBool,
    inner: Mutex<Inner>,
    /// Changes, for every subscribed connection.
    pub events: broadcast::Sender<Event>,
    /// Connections that subscribed; abcd rescans only while there are some.
    pub subscribers: AtomicUsize,
}

struct Inner {
    saved: Saved,
    /// As last scanned, in the order added, pull requests attached.
    repos: Vec<Repo>,
    /// Each repository's pull requests, as gh last listed them.
    prs: HashMap<String, Vec<BranchPr>>,
    /// Who owns each repository's origin, to prefer its pull requests over forks'.
    owners: HashMap<String, Option<String>>,
    github: Github,
    /// When pull requests were last fetched; none means as soon as anyone watches.
    fetched: Option<Instant>,
}

impl Inner {
    /// Gives every worktree its branch's pull request, or none.
    fn attach(&self, repos: &mut [Repo]) {
        for repo in repos {
            let prs = self.prs.get(&repo.name).map_or(&[][..], Vec::as_slice);
            let owner = self.owners.get(&repo.name).cloned().flatten();
            if let RepoState::Present { worktrees } = &mut repo.state {
                for worktree in worktrees {
                    worktree.pr = match &worktree.head {
                        abc_core::model::Head::Branch(branch) => {
                            github::pick(prs, branch, owner.as_deref()).cloned()
                        }
                        abc_core::model::Head::Detached(_) => None,
                    };
                }
            }
        }
    }
}

fn failure(code: ErrorCode, message: impl Into<String>) -> Failure {
    Failure {
        code,
        message: message.into(),
    }
}

impl State {
    /// Reads `file` and scans every repository in it.
    pub async fn load(file: PathBuf, home: PathBuf, settings: &Settings) -> anyhow::Result<Self> {
        let saved = Saved::load(&file)?;
        let repos = scan_all(&saved).await;
        info!("{} repositories", repos.len());
        Ok(Self {
            file,
            home,
            clone_dir: settings.add_repo.clone_dir.clone(),
            github: settings.github.clone(),
            fetching: AtomicBool::new(false),
            inner: Mutex::new(Inner {
                saved,
                repos,
                prs: HashMap::new(),
                owners: HashMap::new(),
                github: Github::Unknown,
                fetched: None,
            }),
            events: broadcast::channel(64).0,
            subscribers: AtomicUsize::new(0),
        })
    }

    /// How many repositories and worktrees, and whether GitHub is on.
    pub async fn summary(&self) -> (usize, usize, Github) {
        let inner = self.inner.lock().await;
        let worktrees = inner.repos.iter().map(|r| r.worktrees().len()).sum();
        (inner.repos.len(), worktrees, inner.github.clone())
    }

    /// Scans again, telling subscribers if anything changed.
    pub async fn rescan(&self) -> Vec<Repo> {
        let mut inner = self.inner.lock().await;
        let mut repos = scan_all(&inner.saved).await;
        inner.attach(&mut repos);
        if repos != inner.repos {
            inner.repos = repos;
            self.changed(&inner);
        }
        inner.repos.clone()
    }

    fn changed(&self, inner: &Inner) {
        let _ = self.events.send(Event::ReposChanged {
            repos: inner.repos.clone(),
        });
    }

    fn save(&self, saved: &Saved) -> Result<(), Failure> {
        saved.save(&self.file).map_err(|e| {
            warn!("saving {}: {e}", self.file.display());
            failure(ErrorCode::Io, format!("couldn't save abc's state: {e}"))
        })
    }

    /// What adding `path`, a folder or git URL, would do. For a URL, `name`
    /// (or, if empty, the repository's) decides where it's cloned.
    pub async fn check(&self, path: &str, name: &str) -> Result<RepoCheck, Failure> {
        let saved = self.inner.lock().await.saved.clone();
        if !is_url(path) {
            let folder = registry::expand(path, &self.home);
            return Ok(registry::check_folder(&folder, &saved).await);
        }
        let url = path.trim();
        let name = self.url_name(url, Some(name))?;
        let (into, shown) = self.clone_target(&name);
        // No lock while git reaches the remote, which can take a while.
        Ok(registry::check_url(url, &into, &shown, &saved).await)
    }

    /// The name a URL's repository is added under: `name` if given, else its own.
    fn url_name(&self, url: &str, name: Option<&str>) -> Result<String, Failure> {
        let name = match name.map(str::trim).filter(|n| !n.is_empty()) {
            Some(name) => name,
            None => source::default_name(url),
        };
        if name.is_empty() {
            return Err(failure(ErrorCode::CantAdd, "a repository needs a name"));
        }
        Ok(name.to_string())
    }

    /// Where a repository named `name` is cloned, and how to show it.
    fn clone_target(&self, name: &str) -> (PathBuf, String) {
        let into = registry::expand(&self.clone_dir, &self.home).join(name);
        let shown = format!("{}/{name}", self.clone_dir.trim_end_matches('/'));
        (into, shown)
    }

    /// Adds the repository at `path` under `name`, or its own name. A git
    /// URL is cloned into the clone folder first.
    pub async fn add(&self, path: &str, name: Option<String>) -> Result<protocol::Added, Failure> {
        let cant = |message: String| failure(ErrorCode::CantAdd, message);
        if !is_url(path) {
            let folder = registry::expand(path, &self.home);
            return self.add_folder(&folder, path.trim(), name).await;
        }
        let url = path.trim();
        let name = self.url_name(url, name.as_deref())?;
        if self.inner.lock().await.saved.named(&name).is_some() {
            return Err(cant(format!("a repository is already named {name}")));
        }
        let (into, shown) = self.clone_target(&name);
        match self.check(url, &name).await? {
            RepoCheck::Remote { .. } => {}
            RepoCheck::AlreadyAdded { name } => {
                return Err(cant(format!("{url} is already added as {name}")));
            }
            RepoCheck::TargetExists { into } => {
                return Err(cant(format!(
                    "{into} already exists; name it something else"
                )));
            }
            _ => return Err(cant(format!("no repository at {url}"))),
        }
        info!("cloning {url} into {}", into.display());
        registry::clone(url, &into)
            .await
            .map_err(|why| cant(format!("couldn't clone {url}: {why}")))?;
        self.add_folder(&into, &shown, Some(name)).await
    }

    /// Adds the repository `folder` is in (written for people as `shown`).
    async fn add_folder(
        &self,
        folder: &Path,
        shown: &str,
        name: Option<String>,
    ) -> Result<protocol::Added, Failure> {
        let mut inner = self.inner.lock().await;
        let cant = |message: String| failure(ErrorCode::CantAdd, message);
        match registry::check_folder(folder, &inner.saved).await {
            RepoCheck::Ready { .. } => {}
            RepoCheck::NotFound => return Err(cant(format!("{shown} doesn't exist"))),
            RepoCheck::AlreadyAdded { name } => {
                return Err(cant(format!("{shown} is already added as {name}")));
            }
            _ => return Err(cant(format!("{shown} is not a git repository"))),
        }
        let (main, git_dir) = registry::resolve(folder)
            .await
            .ok_or_else(|| cant(format!("{shown} is not a git repository")))?;
        let name = match name.as_deref().map(str::trim) {
            Some(name) => name.to_string(),
            None => registry::default_name(&main),
        };
        if name.is_empty() {
            return Err(cant("a repository needs a name".into()));
        }
        if inner.saved.named(&name).is_some() {
            return Err(cant(format!("a repository is already named {name}")));
        }
        let added = Added {
            name: name.clone(),
            path: main,
            git_dir,
        };
        let mut saved = inner.saved.clone();
        saved.repos.push(added.clone());
        self.save(&saved)?;
        let repo = registry::scan(&added, &saved.visits).await;
        info!("added {name} at {}", added.path.display());
        inner.saved = saved;
        inner.repos.push(repo);
        // Its pull requests, at the next chance.
        inner.fetched = None;
        self.changed(&inner);
        Ok(protocol::Added {
            name,
            repos: inner.repos.clone(),
        })
    }

    /// Forgets the repository named `name`; its files stay where they are.
    pub async fn remove(&self, name: &str) -> Result<Repos, Failure> {
        let mut inner = self.inner.lock().await;
        if inner.saved.named(name).is_none() {
            return Err(failure(
                ErrorCode::NotFound,
                format!("no repository named {name}"),
            ));
        }
        let mut saved = inner.saved.clone();
        saved.repos.retain(|r| r.name != name);
        let worktrees: Vec<PathBuf> = inner
            .repos
            .iter()
            .filter(|r| r.name == name)
            .flat_map(|r| r.worktrees().iter().map(|w| w.path.clone()))
            .collect();
        saved.visits.retain(|path, _| !worktrees.contains(path));
        self.save(&saved)?;
        info!("removed {name}");
        inner.saved = saved;
        inner.repos.retain(|r| r.name != name);
        inner.prs.remove(name);
        inner.owners.remove(name);
        self.changed(&inner);
        Ok(Repos {
            repos: inner.repos.clone(),
        })
    }

    /// Whether `worktree` is a worktree of an added repository, still there.
    pub async fn knows(&self, worktree: &Path) -> bool {
        let inner = self.inner.lock().await;
        inner
            .repos
            .iter()
            .flat_map(|r| r.worktrees())
            .any(|w| w.path == worktree && !w.missing)
    }

    /// A client opened `worktree` at `now`, seconds since the Unix epoch.
    pub async fn visit(&self, worktree: &Path, now: u64) -> Result<Done, Failure> {
        let mut inner = self.inner.lock().await;
        let opened = inner
            .repos
            .iter_mut()
            .find_map(|repo| match &mut repo.state {
                RepoState::Present { worktrees } => {
                    worktrees.iter_mut().find(|w| w.path == worktree)
                }
                RepoState::Missing => None,
            });
        let Some(opened) = opened else {
            return Err(failure(
                ErrorCode::NotFound,
                format!("no worktree at {}", worktree.display()),
            ));
        };
        opened.last_opened = Some(now);
        let mut saved = inner.saved.clone();
        saved.visits.insert(worktree.to_path_buf(), now);
        self.save(&saved)?;
        inner.saved = saved;
        self.changed(&inner);
        Ok(Done {})
    }
}

impl State {
    /// Fetches pull requests if `github.pr_interval` has passed since the
    /// last fetch, or there hasn't been one, and no fetch is running.
    pub async fn fetch_prs_if_due(&self) {
        let interval = Duration::from_secs(self.github.pr_interval);
        let due = match self.inner.lock().await.fetched {
            Some(at) => at.elapsed() >= interval,
            None => true,
        };
        if !due || self.fetching.swap(true, Ordering::SeqCst) {
            return;
        }
        self.fetch_prs().await;
        self.fetching.store(false, Ordering::SeqCst);
    }

    /// Asks gh for every repository's pull requests. A repository gh fails
    /// on keeps what it had; gh missing or signed out turns them all off.
    async fn fetch_prs(&self) {
        let github = if self.github.enabled {
            gh_login(&self.github.gh).await
        } else {
            Github::Disabled
        };
        let present: Vec<(String, PathBuf)> = {
            let inner = self.inner.lock().await;
            inner
                .repos
                .iter()
                .filter(|r| !r.is_missing())
                .map(|r| (r.name.clone(), r.path.clone()))
                .collect()
        };
        let mut fetched = HashMap::new();
        let mut owners = HashMap::new();
        if matches!(github, Github::On { .. }) {
            for (name, path) in present {
                let origin = registry::origin(&path).await;
                owners.insert(name.clone(), origin.as_deref().and_then(source::owner));
                match gh_prs(&self.github.gh, &path).await {
                    Ok(prs) => {
                        fetched.insert(name, prs);
                    }
                    Err(why) => warn!("listing {name}'s pull requests: {why}"),
                }
            }
        }
        let mut inner = self.inner.lock().await;
        if matches!(github, Github::On { .. }) {
            for (name, prs) in std::mem::take(&mut inner.prs) {
                fetched.entry(name).or_insert(prs);
            }
        } else if inner.github != github {
            info!("pull request status: {github:?}");
        }
        inner.prs = fetched;
        inner.owners = owners;
        inner.github = github;
        inner.fetched = Some(Instant::now());
        let mut repos = inner.repos.clone();
        inner.attach(&mut repos);
        if repos != inner.repos {
            inner.repos = repos;
            self.changed(&inner);
        }
    }
}

/// Whether gh is installed and signed in, and as whom.
async fn gh_login(gh: &str) -> Github {
    let off = |why: &str| Github::Off { why: why.into() };
    let mut status = Command::new(gh);
    status
        .args(["auth", "status", "--json", "hosts"])
        .kill_on_drop(true);
    let out = match tokio::time::timeout(GH_TIMEOUT, status.output()).await {
        Ok(Ok(out)) => out,
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => {
            return off("gh isn't installed · PR status is off");
        }
        _ => return off("gh didn't answer · PR status is off for now"),
    };
    match github::parse_login(&out.stdout) {
        Some(login) => Github::On { login },
        None => off("gh is not logged in · PR status is off · run gh auth login"),
    }
}

/// A repository's pull requests, as gh lists them.
async fn gh_prs(gh: &str, repo: &Path) -> Result<Vec<BranchPr>, String> {
    let mut list = Command::new(gh);
    list.args([
        "pr",
        "list",
        "--state",
        "all",
        "--limit",
        "100",
        "--json",
        github::FIELDS,
    ])
    .current_dir(repo)
    .kill_on_drop(true);
    let out = tokio::time::timeout(GH_TIMEOUT, list.output())
        .await
        .map_err(|_| "gh took too long".to_string())?
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    github::parse_prs(&out.stdout)
}

async fn scan_all(saved: &Saved) -> Vec<Repo> {
    let mut repos = Vec::with_capacity(saved.repos.len());
    for added in &saved.repos {
        repos.push(registry::scan(added, &saved.visits).await);
    }
    repos
}
