//! A worktree's files over abcd's socket: listing, reading, finding text.

mod common;

use abc_core::model::FileContents;
use abc_core::protocol::{self, Call, ErrorCode, Event, Files, SearchDone};
use common::{connect, home, repo, shut_down};

/// An added repository with a few files; its path.
async fn added(home: &std::path::Path, client: &abc_core::client::Client) -> std::path::PathBuf {
    let app = repo(home, "app", &[]);
    std::fs::write(app.join(".gitignore"), "*.log\n").unwrap();
    std::fs::create_dir_all(app.join("src")).unwrap();
    std::fs::write(app.join("src/main.rs"), "fn main() {\n    gst();\n}\n").unwrap();
    std::fs::write(app.join("src/gst.rs"), "pub fn gst() {}\n").unwrap();
    std::fs::write(app.join("debug.log"), "gst").unwrap();
    let _: protocol::Added = client
        .call(Call::RepoAdd {
            path: app.display().to_string(),
            name: None,
        })
        .await
        .unwrap();
    app
}

#[tokio::test]
async fn files_are_listed_and_read() {
    let home = home();
    let client = connect(home.path()).await;
    let app = added(home.path(), &client).await;
    let files: Files = client
        .call(Call::WorktreeFiles {
            worktree: app.clone(),
        })
        .await
        .unwrap();
    assert_eq!(files.files, [".gitignore", "src/gst.rs", "src/main.rs"]);

    let read = |path: &str| Call::FileRead {
        worktree: app.clone(),
        path: path.into(),
    };
    let main: FileContents = client.call(read("src/main.rs")).await.unwrap();
    assert_eq!(
        main,
        FileContents::Text {
            text: "fn main() {\n    gst();\n}\n".into()
        }
    );
    let outside = client.call::<FileContents>(read("../x")).await.unwrap_err();
    assert_eq!(outside.code, ErrorCode::BadRequest);
    let unknown = client
        .call::<Files>(Call::WorktreeFiles {
            worktree: home.path().to_path_buf(),
        })
        .await
        .unwrap_err();
    assert_eq!(unknown.code, ErrorCode::NotFound, "only added worktrees");
    shut_down(client).await;
}

#[tokio::test]
async fn a_search_sends_its_finds_then_answers() {
    let home = home();
    let client = connect(home.path()).await;
    let mut events = client.events().unwrap();
    let app = added(home.path(), &client).await;
    let done: SearchDone = client
        .call(Call::Search {
            worktree: app,
            query: "gst".into(),
            max_lines: 100,
        })
        .await
        .unwrap();
    assert!(!done.capped);
    let mut paths = Vec::new();
    while let Ok(event) = events.try_recv() {
        if let Event::SearchFound { files, .. } = event {
            paths.extend(files.into_iter().map(|f| f.path));
        }
    }
    assert_eq!(paths, ["src/gst.rs", "src/main.rs"], "not the ignored log");
    shut_down(client).await;
}

/// Waits up to `secs` for an event `matches` accepts.
async fn event_within(
    events: &mut tokio::sync::mpsc::UnboundedReceiver<Event>,
    secs: u64,
    matches: impl Fn(&Event) -> bool,
) -> Option<Event> {
    tokio::time::timeout(std::time::Duration::from_secs(secs), async {
        loop {
            match events.recv().await {
                Some(event) if matches(&event) => return Some(event),
                Some(_) => {}
                None => return None,
            }
        }
    })
    .await
    .ok()
    .flatten()
}

/// Settings that list again every second, so a test needn't wait five.
fn rescan_every_second(home: &std::path::Path) {
    let config = common::paths(home).config_file();
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(config, "[git]\nstatus_interval = 1\n").unwrap();
}

#[tokio::test]
async fn what_a_connection_has_open_is_followed() {
    let home = home();
    rescan_every_second(home.path());
    let client = connect(home.path()).await;
    let mut events = client.events().unwrap();
    let app = added(home.path(), &client).await;
    let _: Files = client
        .call(Call::WorktreeFiles {
            worktree: app.clone(),
        })
        .await
        .unwrap();
    let _: FileContents = client
        .call(Call::FileRead {
            worktree: app.clone(),
            path: "src/main.rs".into(),
        })
        .await
        .unwrap();
    let _: protocol::Done = client
        .call(Call::Watch {
            worktree: app.clone(),
            path: Some("src/main.rs".into()),
        })
        .await
        .unwrap();

    std::fs::write(
        app.join("src/main.rs"),
        "fn main() {\n    gst(); gst();\n}\n",
    )
    .unwrap();
    let changed = event_within(&mut events, 5, |e| matches!(e, Event::FileChanged { .. })).await;
    assert_eq!(
        changed,
        Some(Event::FileChanged {
            worktree: app.clone(),
            path: "src/main.rs".into()
        })
    );

    std::fs::write(app.join("src/new.rs"), "").unwrap();
    let listed = event_within(&mut events, 5, |e| matches!(e, Event::FilesChanged { .. })).await;
    let Some(Event::FilesChanged { files, .. }) = listed else {
        panic!("no files.changed");
    };
    assert!(files.contains(&"src/new.rs".to_string()));
    shut_down(client).await;
}

#[tokio::test]
async fn a_change_while_looking_elsewhere_is_said_on_looking_back() {
    let home = home();
    let client = connect(home.path()).await;
    let mut events = client.events().unwrap();
    let app = added(home.path(), &client).await;
    let _: FileContents = client
        .call(Call::FileRead {
            worktree: app.clone(),
            path: "src/gst.rs".into(),
        })
        .await
        .unwrap();
    let watch = |path: &str| Call::Watch {
        worktree: app.clone(),
        path: Some(path.into()),
    };
    let _: protocol::Done = client.call(watch("src/main.rs")).await.unwrap();
    std::fs::write(app.join("src/gst.rs"), "pub fn gst() { changed() }\n").unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    let _: protocol::Done = client.call(watch("src/gst.rs")).await.unwrap();
    let changed = event_within(&mut events, 3, |e| matches!(e, Event::FileChanged { .. })).await;
    assert!(
        matches!(changed, Some(Event::FileChanged { ref path, .. }) if path == "src/gst.rs"),
        "{changed:?}"
    );
    let unknown = client
        .call::<protocol::Done>(Call::Watch {
            worktree: home.path().to_path_buf(),
            path: None,
        })
        .await
        .unwrap_err();
    assert_eq!(unknown.code, ErrorCode::NotFound);
    shut_down(client).await;
}
