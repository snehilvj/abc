//! Pull request status, from a stand-in for gh that answers as gh does.

mod common;

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::Duration;

use abc_core::protocol::{self, Call, Done, Event, Github, ServerStatus};
use common::{connect, home, repo, shut_down};

const PRS: &str = r#"[{"number": 7, "title": "Feature X", "url": "https://example.com/7",
  "state": "OPEN", "isDraft": false, "headRefName": "feat/x", "reviewDecision": "APPROVED",
  "updatedAt": "2026-10-28T09:00:00Z", "headRepositoryOwner": {"login": "me"},
  "statusCheckRollup": [{"__typename": "CheckRun", "status": "COMPLETED", "conclusion": "SUCCESS"}]}]"#;

/// Settings using a gh that is `signed_in` or not, rescanning every second.
fn settings(home: &Path, signed_in: bool, enabled: bool) {
    let bin = home.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::write(bin.join("prs.json"), PRS).unwrap();
    let auth = if signed_in {
        r#"echo '{"hosts": {"github.com": [{"state": "success", "active": true, "login": "tester"}]}}'"#
    } else {
        r#"echo '{"hosts": {"github.com": [{"state": "error", "active": true, "login": "tester"}]}}'; exit 1"#
    };
    let script = format!(
        "#!/bin/sh\ncase \"$1 $2\" in\n  \"auth status\") {auth} ;;\n  \"pr list\") cat \"{}\" ;;\nesac\n",
        bin.join("prs.json").display()
    );
    let gh = bin.join("gh");
    std::fs::write(&gh, script).unwrap();
    std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
    let config = common::paths(home).config_file();
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    let toml = format!(
        "[git]\nstatus_interval = 1\n[github]\nenabled = {enabled}\ngh = {:?}\n",
        gh.display().to_string()
    );
    std::fs::write(config, toml).unwrap();
}

/// Adds `home/app` with a worktree on feat/x, subscribes, and waits for the
/// GitHub status to settle.
async fn watching(home: &Path) -> abc_core::client::Client {
    let app = repo(home, "app", &["feat/x"]);
    let client = connect(home).await;
    let _: protocol::Added = client
        .call(Call::RepoAdd {
            path: app.display().to_string(),
            name: None,
        })
        .await
        .unwrap();
    let _: Done = client.call(Call::Subscribe).await.unwrap();
    for _ in 0..100 {
        let status: ServerStatus = client.call(Call::ServerStatus).await.unwrap();
        if status.github != Github::Unknown {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    client
}

#[tokio::test]
async fn a_worktree_gets_its_branchs_pull_request() {
    let home = home();
    settings(home.path(), true, true);
    let app = repo(home.path(), "app", &["feat/x"]);
    let client = connect(home.path()).await;
    let mut events = client.events().unwrap();
    let _: Done = client.call(Call::Subscribe).await.unwrap();
    let _: protocol::Added = client
        .call(Call::RepoAdd {
            path: app.display().to_string(),
            name: None,
        })
        .await
        .unwrap();
    let pr = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Some(Event::ReposChanged { repos }) = events.recv().await
                && let Some(pr) = repos[0].worktrees().iter().find_map(|w| w.pr.clone())
            {
                return pr;
            }
        }
    })
    .await
    .expect("a pull request within 10 s");
    assert_eq!((pr.number, pr.title.as_str()), (7, "Feature X"));
    let status: ServerStatus = client.call(Call::ServerStatus).await.unwrap();
    assert_eq!(
        status.github,
        Github::On {
            login: "tester".into()
        }
    );
    shut_down(client).await;
}

#[tokio::test]
async fn signed_out_or_turned_off_says_so() {
    let home = home();
    settings(home.path(), false, true);
    let client = watching(home.path()).await;
    let status: ServerStatus = client.call(Call::ServerStatus).await.unwrap();
    assert!(
        matches!(&status.github, Github::Off { why } if why.contains("not logged in")),
        "{:?}",
        status.github
    );
    shut_down(client).await;

    let home = common::home();
    settings(home.path(), true, false);
    let client = watching(home.path()).await;
    let status: ServerStatus = client.call(Call::ServerStatus).await.unwrap();
    assert_eq!(status.github, Github::Disabled);
    shut_down(client).await;
}
