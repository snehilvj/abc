//! Adding, listing and removing repositories over abcd's socket.

mod common;

use std::time::Duration;

use abc_core::model::RepoCheck;
use abc_core::protocol::{self, Call, Done, ErrorCode, Event, Repos};
use common::{connect, git, home, repo, shut_down};

fn add(path: &std::path::Path, name: Option<&str>) -> Call {
    Call::RepoAdd {
        path: path.display().to_string(),
        name: name.map(String::from),
    }
}

#[tokio::test]
async fn repositories_are_added_listed_and_removed() {
    let home = home();
    let app = repo(home.path(), "app", &["feat/x"]);
    let client = connect(home.path()).await;
    let added: protocol::Added = client.call(add(&app, None)).await.unwrap();
    assert_eq!(added.name, "app");
    assert_eq!(added.repos[0].path, app);
    assert_eq!(added.repos[0].worktrees().len(), 2);
    let listed: Repos = client.call(Call::RepoList).await.unwrap();
    assert_eq!(listed.repos, added.repos);
    let removed: Repos = client
        .call(Call::RepoRemove { name: "app".into() })
        .await
        .unwrap();
    assert!(removed.repos.is_empty());
    assert!(app.join(".git").exists(), "its files stay");
    shut_down(client).await;
}

#[tokio::test]
async fn what_cant_be_added_says_why() {
    let home = home();
    let app = repo(home.path(), "app", &["feat/x"]);
    let downloads = home.path().join("Downloads");
    std::fs::create_dir(&downloads).unwrap();
    let client = connect(home.path()).await;
    let check = |path: &std::path::Path| Call::RepoCheck {
        path: path.display().to_string(),
        name: String::new(),
    };
    let ready: RepoCheck = client.call(check(&app)).await.unwrap();
    assert_eq!(ready, RepoCheck::Ready { worktrees: 2 });
    let not_git: RepoCheck = client.call(check(&downloads)).await.unwrap();
    assert_eq!(not_git, RepoCheck::NotGit);

    let failed = client
        .call::<protocol::Added>(add(&downloads, None))
        .await
        .unwrap_err();
    assert_eq!(failed.code, ErrorCode::CantAdd);
    assert_eq!(
        failed.message,
        format!("{} is not a git repository", downloads.display())
    );

    let _: protocol::Added = client.call(add(&app, None)).await.unwrap();
    let linked = home.path().join("app-feat-x");
    let twice = client
        .call::<protocol::Added>(add(&linked, None))
        .await
        .unwrap_err();
    assert_eq!(
        twice.message,
        format!("{} is already added as app", linked.display())
    );

    let other = repo(home.path(), "other", &[]);
    let taken = client
        .call::<protocol::Added>(add(&other, Some("app")))
        .await
        .unwrap_err();
    assert_eq!(taken.message, "a repository is already named app");

    let unknown = client
        .call::<Repos>(Call::RepoRemove {
            name: "nope".into(),
        })
        .await
        .unwrap_err();
    assert_eq!(
        (unknown.code, unknown.message.as_str()),
        (ErrorCode::NotFound, "no repository named nope")
    );
    shut_down(client).await;
}

#[tokio::test]
async fn a_change_reaches_every_subscriber() {
    let home = home();
    let app = repo(home.path(), "app", &[]);
    let watcher = connect(home.path()).await;
    let mut events = watcher.events().unwrap();
    let _: Done = watcher.call(Call::Subscribe).await.unwrap();
    let other = connect(home.path()).await;
    let _: protocol::Added = other.call(add(&app, Some("mine"))).await.unwrap();
    let event = tokio::time::timeout(Duration::from_secs(5), events.recv())
        .await
        .unwrap()
        .unwrap();
    let Event::ReposChanged { repos } = event else {
        panic!("expected repos.changed, got {event:?}");
    };
    assert_eq!(repos[0].name, "mine");
    shut_down(other).await;
}

#[tokio::test]
async fn repositories_outlive_abcd() {
    let home = home();
    let app = repo(home.path(), "app", &[]);
    let client = connect(home.path()).await;
    let _: protocol::Added = client.call(add(&app, None)).await.unwrap();
    shut_down(client).await;
    let paths = common::paths(home.path());
    while paths.socket().exists() {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let client = connect(home.path()).await;
    let listed: Repos = client.call(Call::RepoList).await.unwrap();
    assert_eq!(listed.repos[0].name, "app");
    shut_down(client).await;
}

#[tokio::test]
async fn the_list_follows_the_disk() {
    let home = home();
    let app = repo(home.path(), "app", &[]);
    let gone = repo(home.path(), "gone", &[]);
    let client = connect(home.path()).await;
    let _: protocol::Added = client.call(add(&app, None)).await.unwrap();
    let _: protocol::Added = client.call(add(&gone, None)).await.unwrap();

    let made_outside = home.path().join("app-later");
    git(
        &app,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "later",
            made_outside.to_str().unwrap(),
        ],
    );
    std::fs::remove_dir_all(&gone).unwrap();
    let listed: Repos = client.call(Call::RepoList).await.unwrap();
    assert_eq!(
        listed.repos[0].worktrees().len(),
        2,
        "a worktree made outside abc"
    );
    assert!(
        listed.repos[1].is_missing(),
        "a repository whose folder went"
    );
    shut_down(client).await;
}

#[tokio::test]
async fn opening_a_worktree_is_remembered() {
    let home = home();
    let app = repo(home.path(), "app", &[]);
    let client = connect(home.path()).await;
    let _: protocol::Added = client.call(add(&app, None)).await.unwrap();
    let _: Done = client
        .call(Call::WorktreeVisit {
            worktree: app.clone(),
        })
        .await
        .unwrap();
    let listed: Repos = client.call(Call::RepoList).await.unwrap();
    assert!(listed.repos[0].worktrees()[0].last_opened.is_some());
    let unknown = client
        .call::<Done>(Call::WorktreeVisit {
            worktree: home.path().join("nowhere"),
        })
        .await
        .unwrap_err();
    assert_eq!(unknown.code, ErrorCode::NotFound);
    shut_down(client).await;
}

/// A bare repository at `home/remote/<name>.git` to clone, as a file:// URL,
/// and settings that clone into `home/code`.
fn remote(home: &std::path::Path, name: &str) -> String {
    let upstream = repo(&home.join("upstream"), name, &[]);
    let bare = home.join("remote").join(format!("{name}.git"));
    std::fs::create_dir_all(bare.parent().unwrap()).unwrap();
    git(
        home,
        &[
            "clone",
            "-q",
            "--bare",
            upstream.to_str().unwrap(),
            bare.to_str().unwrap(),
        ],
    );
    let config = common::paths(home).config_file();
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    let clone_dir = home.join("code");
    std::fs::write(
        &config,
        format!(
            "[add_repo]\nclone_dir = {:?}\n",
            clone_dir.display().to_string()
        ),
    )
    .unwrap();
    format!("file://{}", bare.display())
}

fn check_url(url: &str, name: &str) -> Call {
    Call::RepoCheck {
        path: url.into(),
        name: name.into(),
    }
}

#[tokio::test]
async fn a_git_url_is_cloned_and_added() {
    let home = home();
    let url = remote(home.path(), "app");
    let client = connect(home.path()).await;
    let into = home.path().join("code/app").display().to_string();
    let check: RepoCheck = client.call(check_url(&url, "")).await.unwrap();
    assert_eq!(check, RepoCheck::Remote { into: into.clone() });
    let named: RepoCheck = client.call(check_url(&url, "mine")).await.unwrap();
    assert_eq!(
        named,
        RepoCheck::Remote {
            into: home.path().join("code/mine").display().to_string()
        },
        "the name decides the folder"
    );

    let added: protocol::Added = client
        .call(Call::RepoAdd {
            path: url.clone(),
            name: None,
        })
        .await
        .unwrap();
    assert_eq!(added.name, "app");
    assert!(home.path().join("code/app/.git").exists(), "cloned");
    assert_eq!(added.repos[0].worktrees().len(), 1);

    let again: RepoCheck = client.call(check_url(&url, "other")).await.unwrap();
    assert_eq!(
        again,
        RepoCheck::AlreadyAdded { name: "app".into() },
        "by its remote"
    );
    shut_down(client).await;
}

#[tokio::test]
async fn a_url_that_cant_be_cloned_says_why() {
    let home = home();
    let url = remote(home.path(), "app");
    std::fs::create_dir_all(home.path().join("code/app")).unwrap();
    let client = connect(home.path()).await;
    let exists: RepoCheck = client.call(check_url(&url, "")).await.unwrap();
    assert!(matches!(exists, RepoCheck::TargetExists { .. }));
    let failed = client
        .call::<protocol::Added>(Call::RepoAdd {
            path: url.clone(),
            name: None,
        })
        .await
        .unwrap_err();
    assert!(
        failed
            .message
            .ends_with("code/app already exists; name it something else")
    );

    let nowhere = format!("file://{}/remote/nope.git", home.path().display());
    let missing: RepoCheck = client.call(check_url(&nowhere, "")).await.unwrap();
    assert_eq!(missing, RepoCheck::NotFound);
    let failed = client
        .call::<protocol::Added>(Call::RepoAdd {
            path: nowhere.clone(),
            name: None,
        })
        .await
        .unwrap_err();
    assert_eq!(failed.message, format!("no repository at {nowhere}"));
    shut_down(client).await;
}
