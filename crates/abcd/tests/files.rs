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
