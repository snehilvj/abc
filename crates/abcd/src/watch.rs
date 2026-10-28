//! Following what one connection has open: its worktree's files, listed
//! again every `git.status_interval`, and the file in its viewer, looked at
//! every second. A change goes to that connection only, as an event. What
//! the connection was last sent is remembered, so a change made while it
//! looked elsewhere is said as soon as it looks back.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use abc_core::protocol::{Event, encode};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::files::{self, Stamp};

/// How often the open file is looked at.
const FILE_CHECK: Duration = Duration::from_secs(1);

/// What one connection has been sent, and the task following what it has open.
#[derive(Default)]
pub struct Watch {
    seen: Arc<Mutex<Seen>>,
    task: Option<JoinHandle<()>>,
}

#[derive(Default)]
struct Seen {
    /// Each worktree's files, as last sent.
    listed: HashMap<PathBuf, Vec<String>>,
    /// Each file's stamp when it was last read for this connection.
    read: HashMap<(PathBuf, String), Option<Stamp>>,
}

impl Watch {
    /// A worktree's files were sent, in an answer.
    pub fn listed(&self, worktree: &Path, files: &[String]) {
        let mut seen = self.seen.lock().unwrap();
        seen.listed.insert(worktree.to_path_buf(), files.to_vec());
    }

    /// A file was read for this connection; `stamp` is from just before.
    pub fn read(&self, worktree: &Path, path: &str, stamp: Option<Stamp>) {
        let mut seen = self.seen.lock().unwrap();
        seen.read
            .insert((worktree.to_path_buf(), path.to_string()), stamp);
    }

    /// Follows `worktree` and `path` from now on, in place of what was
    /// followed before, listing every `every`.
    pub fn follow(
        &mut self,
        worktree: PathBuf,
        path: Option<String>,
        every: Duration,
        out: mpsc::UnboundedSender<String>,
    ) {
        self.stop();
        let seen = self.seen.clone();
        self.task = Some(tokio::spawn(async move {
            let mut tick = tokio::time::interval(FILE_CHECK);
            // At once: anything changed since this connection last looked.
            let mut listed_at: Option<Instant> = None;
            loop {
                tick.tick().await;
                if let Some(path) = &path
                    && let Some(event) = file_changed(&seen, &worktree, path).await
                    && out.send(encode(&event)).is_err()
                {
                    return;
                }
                if listed_at.is_none_or(|at| at.elapsed() >= every) {
                    listed_at = Some(Instant::now());
                    if let Some(event) = files_changed(&seen, &worktree).await
                        && out.send(encode(&event)).is_err()
                    {
                        return;
                    }
                }
            }
        }));
    }

    /// Stops following. Its task holds a sender: the connection's writer
    /// finishes only once it's gone.
    pub fn stop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        self.stop();
    }
}

/// `file.changed` if the file's stamp differs from when it was last read.
/// A file never read here is noted, not reported.
async fn file_changed(seen: &Mutex<Seen>, worktree: &Path, path: &str) -> Option<Event> {
    let now = {
        let (worktree, path) = (worktree.to_path_buf(), path.to_string());
        tokio::task::spawn_blocking(move || files::stamp(&worktree, &path))
            .await
            .ok()?
    };
    let key = (worktree.to_path_buf(), path.to_string());
    let mut seen = seen.lock().unwrap();
    match seen.read.insert(key, now) {
        Some(before) if before != now => Some(Event::FileChanged {
            worktree: worktree.to_path_buf(),
            path: path.to_string(),
        }),
        _ => None,
    }
}

/// `files.changed` if the worktree's files differ from those last sent.
async fn files_changed(seen: &Mutex<Seen>, worktree: &Path) -> Option<Event> {
    let files = {
        let worktree = worktree.to_path_buf();
        tokio::task::spawn_blocking(move || files::list(&worktree))
            .await
            .ok()?
    };
    let mut seen = seen.lock().unwrap();
    match seen.listed.insert(worktree.to_path_buf(), files.clone()) {
        Some(before) if before != files => Some(Event::FilesChanged {
            worktree: worktree.to_path_buf(),
            files,
        }),
        _ => None,
    }
}
