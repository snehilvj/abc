//! abcd's state: the registry and the repositories as last scanned, shared by
//! every connection, and the changes pushed to those that subscribed.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicUsize;

use abc_core::model::{Repo, RepoCheck, RepoState};
use abc_core::protocol::{self, Done, ErrorCode, Event, Failure, Repos};
use abc_core::source::{self, is_url};
use tokio::sync::{Mutex, broadcast};
use tracing::{info, warn};

use crate::registry::{self, Added, Saved};

pub struct State {
    /// `state.json`.
    file: PathBuf,
    /// Where `~` and relative paths start.
    home: PathBuf,
    /// Where git URLs are cloned (`add_repo.clone_dir`), as written.
    clone_dir: String,
    inner: Mutex<Inner>,
    /// Changes, for every subscribed connection.
    pub events: broadcast::Sender<Event>,
    /// Connections that subscribed; abcd rescans only while there are some.
    pub subscribers: AtomicUsize,
}

struct Inner {
    saved: Saved,
    /// As last scanned, in the order added.
    repos: Vec<Repo>,
}

fn failure(code: ErrorCode, message: impl Into<String>) -> Failure {
    Failure {
        code,
        message: message.into(),
    }
}

impl State {
    /// Reads `file` and scans every repository in it.
    pub async fn load(file: PathBuf, home: PathBuf, clone_dir: String) -> anyhow::Result<Self> {
        let saved = Saved::load(&file)?;
        let repos = scan_all(&saved).await;
        info!("{} repositories", repos.len());
        Ok(Self {
            file,
            home,
            clone_dir,
            inner: Mutex::new(Inner { saved, repos }),
            events: broadcast::channel(64).0,
            subscribers: AtomicUsize::new(0),
        })
    }

    /// Scans again, telling subscribers if anything changed.
    pub async fn rescan(&self) -> Vec<Repo> {
        let mut inner = self.inner.lock().await;
        let repos = scan_all(&inner.saved).await;
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

async fn scan_all(saved: &Saved) -> Vec<Repo> {
    let mut repos = Vec::with_capacity(saved.repos.len());
    for added in &saved.repos {
        repos.push(registry::scan(added, &saved.visits).await);
    }
    repos
}
