//! What the integration tests share: a home of their own, an abcd started
//! in it, and git repositories made with no user config.

#![allow(dead_code)]

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use abc_core::client::{self, Client};
use abc_core::paths::{Paths, Profile};
use abc_core::protocol::{Call, Done};

/// A home for one test, under /tmp: macOS caps socket paths at 104 bytes.
/// Removing it removes abcd's socket, so an abcd left by a failed test exits.
pub fn home() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("abc-")
        .tempdir_in("/tmp")
        .unwrap()
}

pub fn paths(home: &Path) -> Paths {
    let home = home.to_path_buf();
    Paths::new(Profile::new("test").unwrap(), move |var| match var {
        "HOME" => Some(OsString::from(&home)),
        "XDG_STATE_HOME" => Some(home.join("state").into()),
        "XDG_CONFIG_HOME" => Some(home.join("config").into()),
        _ => None,
    })
    .unwrap()
}

pub fn abcd() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_abcd"))
}

/// A client of the abcd in `home`, starting it if need be.
pub async fn connect(home: &Path) -> Client {
    client::start(&paths(home), abcd(), &client::hello())
        .await
        .unwrap()
        .0
}

pub async fn shut_down(client: Client) {
    let _: Done = client.call(Call::ServerShutdown).await.unwrap();
}

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

/// A repository at `dir/name` with one commit, and a linked worktree
/// `dir/name-<branch>` per branch. Its path as git reports it.
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
