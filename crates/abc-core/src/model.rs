//! What abcd knows about repositories and their worktrees, as sent to clients.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A registered repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Repo {
    /// The name it was registered under; unique within a profile.
    pub name: String,
    /// The repository's main folder (for a bare repository, its git folder).
    pub path: PathBuf,
    pub state: RepoState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RepoState {
    Present {
        worktrees: Vec<Worktree>,
    },
    /// The repository's folder no longer exists.
    Missing,
}

impl Repo {
    pub fn worktrees(&self) -> &[Worktree] {
        match &self.state {
            RepoState::Present { worktrees } => worktrees,
            RepoState::Missing => &[],
        }
    }

    pub fn is_missing(&self) -> bool {
        matches!(self.state, RepoState::Missing)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Worktree {
    pub path: PathBuf,
    /// The worktree's folder name.
    pub name: String,
    pub head: Head,
    /// The repository's main checkout rather than a linked worktree.
    pub is_main: bool,
    pub locked: bool,
    /// The folder is gone (git calls the worktree prunable).
    pub missing: bool,
    /// None until abcd has run `git status` in it.
    pub git: Option<GitStatus>,
    /// The most recently updated pull request for its branch, if any.
    pub pr: Option<PullRequest>,
    /// Seconds since the Unix epoch when a client last opened it.
    pub last_opened: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "name", rename_all = "snake_case")]
pub enum Head {
    Branch(String),
    /// A detached HEAD, as a short commit id.
    Detached(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct GitStatus {
    /// Files with uncommitted changes, including untracked ones.
    pub changed: u32,
    pub ahead: u32,
    pub behind: u32,
    /// Whether the branch tracks an upstream.
    pub upstream: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullRequest {
    pub number: u32,
    pub title: String,
    pub url: String,
    pub state: PrState,
    pub checks: Option<Checks>,
    pub review: Option<Review>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrState {
    Open,
    Draft,
    Merged,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Checks {
    Passing,
    Failing,
    Pending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Review {
    Approved,
    ChangesRequested,
    Required,
}

/// A file's contents as abcd sends them to the viewer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FileContents {
    Text {
        text: String,
    },
    /// A NUL byte in the first 8 KB.
    Binary {
        size: u64,
    },
    /// Larger than `viewer.max_file_size_mb`; not sent.
    TooLarge {
        size: u64,
        limit: u64,
    },
}

/// A file with lines that matched a find in files, as abcd sends it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FoundFile {
    /// Relative to the worktree, such as `src/main.rs`.
    pub path: String,
    pub lines: Vec<FoundLine>,
}

/// A line that matched: its number, counting from 1, and its text as the file has it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FoundLine {
    pub number: u32,
    pub text: String,
}

/// What abcd made of a folder or git URL offered as a repository to add.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RepoCheck {
    /// A git repository abc can add, and how many worktrees it has.
    Ready {
        worktrees: usize,
    },
    /// A repository at a git URL, which abcd will clone into `into` and add.
    Remote {
        into: String,
    },
    /// No such folder, or nothing at that URL.
    NotFound,
    NotGit,
    /// Already added, under this name; found by its git folder (so through
    /// any of its worktrees too) or, for a URL, by its remote.
    AlreadyAdded {
        name: String,
    },
    /// The folder a clone would go into already exists.
    TargetExists {
        into: String,
    },
}
