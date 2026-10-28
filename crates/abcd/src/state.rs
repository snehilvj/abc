//! abcd's state: the registry and the repositories as last scanned, shared by
//! every connection, and the changes pushed to those that subscribed.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicUsize;

use abc_core::model::{Repo, RepoCheck, RepoState};
use abc_core::protocol::{self, Done, ErrorCode, Event, Failure, Repos};
use tokio::sync::{Mutex, broadcast};
use tracing::{info, warn};

use crate::registry::{self, Added, Saved};

pub struct State {
    /// `state.json`.
    file: PathBuf,
    /// Where `~` and relative paths start.
    home: PathBuf,
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
    pub async fn load(file: PathBuf, home: PathBuf) -> anyhow::Result<Self> {
        let saved = Saved::load(&file)?;
        let repos = scan_all(&saved).await;
        info!("{} repositories", repos.len());
        Ok(Self {
            file,
            home,
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

    /// What adding `path` would do. The name matters only for a git URL.
    pub async fn check(&self, path: &str, _name: &str) -> Result<RepoCheck, Failure> {
        let folder = registry::expand(path, &self.home);
        let inner = self.inner.lock().await;
        Ok(registry::check_folder(&folder, &inner.saved).await)
    }

    /// Adds the repository at `path` under `name`, or its folder's name.
    pub async fn add(&self, path: &str, name: Option<String>) -> Result<protocol::Added, Failure> {
        let shown = path.trim();
        let folder = registry::expand(path, &self.home);
        let mut inner = self.inner.lock().await;
        let cant = |message: String| failure(ErrorCode::CantAdd, message);
        match registry::check_folder(&folder, &inner.saved).await {
            RepoCheck::Ready { .. } => {}
            RepoCheck::NotFound => return Err(cant(format!("{shown} doesn't exist"))),
            RepoCheck::AlreadyAdded { name } => {
                return Err(cant(format!("{shown} is already added as {name}")));
            }
            _ => return Err(cant(format!("{shown} is not a git repository"))),
        }
        let (main, git_dir) = registry::resolve(&folder)
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
