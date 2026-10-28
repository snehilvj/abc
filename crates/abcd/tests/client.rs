//! abc meeting abcd: starting it, reusing it, and builds that differ.

use std::ffi::OsString;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use abc_core::build::BUILD_ID;
use abc_core::client::{self, Client, StartError, Startup};
use abc_core::paths::{Paths, Profile};
use abc_core::protocol::{Call, Done, Hello, PROTOCOL, ServerStatus};

/// A home for one test, under /tmp: macOS caps socket paths at 104 bytes.
/// Removing it removes abcd's socket, so an abcd left by a failed test exits.
fn home() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("abc-")
        .tempdir_in("/tmp")
        .unwrap()
}

fn paths(home: &Path) -> Paths {
    let home = home.to_path_buf();
    Paths::new(Profile::new("test").unwrap(), move |var| match var {
        "HOME" => Some(OsString::from(&home)),
        "XDG_STATE_HOME" => Some(home.join("state").into()),
        "XDG_CONFIG_HOME" => Some(home.join("config").into()),
        _ => None,
    })
    .unwrap()
}

fn abcd() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_abcd"))
}

fn me(protocol: u32, build: &str) -> Hello {
    Hello {
        protocol,
        build: build.into(),
        pid: None,
        profile: None,
    }
}

async fn pid(client: &Client) -> u32 {
    client
        .call::<ServerStatus>(Call::ServerStatus)
        .await
        .unwrap()
        .pid
}

async fn shut_down(client: Client) {
    let _: Done = client.call(Call::ServerShutdown).await.unwrap();
}

#[tokio::test]
async fn starts_abcd_when_it_isnt_running_and_reuses_it_after() {
    let home = home();
    let paths = paths(home.path());
    let (first, startup) = client::start(&paths, abcd(), &client::hello())
        .await
        .unwrap();
    assert_eq!(startup, Startup::Ready { started: true });
    let (second, startup) = client::start(&paths, abcd(), &client::hello())
        .await
        .unwrap();
    assert_eq!(startup, Startup::Ready { started: false });
    assert_eq!(pid(&first).await, pid(&second).await);
    assert!(
        paths.socket().starts_with(home.path()),
        "in the test's home"
    );
    shut_down(second).await;
}

#[tokio::test]
async fn a_stale_abcd_is_restarted() {
    let home = home();
    let paths = paths(home.path());
    let mut old_abcd = Command::new(abcd())
        .args(["--profile", "test", "--build-id", "0.0.0+feedface"])
        .env("XDG_STATE_HOME", paths.state_home())
        .env("XDG_CONFIG_HOME", paths.config_home())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    while !paths.socket().exists() {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let stale = Client::connect(&paths.socket(), &client::hello())
        .await
        .unwrap();
    let stale_pid = pid(&stale).await;
    drop(stale);

    let (client, startup) = client::start(&paths, abcd(), &client::hello())
        .await
        .unwrap();
    assert_eq!(
        startup,
        Startup::Restarted {
            from: "0.0.0+feedface".into()
        }
    );
    assert_eq!(client.server.build, BUILD_ID);
    assert_ne!(pid(&client).await, stale_pid);
    assert!(
        old_abcd.wait().unwrap().success(),
        "the old one stopped cleanly"
    );
    shut_down(client).await;
}

#[tokio::test]
async fn a_stale_abcd_of_another_protocol_is_restarted_too() {
    let home = home();
    let paths = paths(home.path());
    let mut old_abcd = Command::new(abcd())
        .args(["--profile", "test", "--build-id", "0.0.0+0ddba11"])
        .args(["--protocol", &(PROTOCOL + 1).to_string()])
        .env("XDG_STATE_HOME", paths.state_home())
        .env("XDG_CONFIG_HOME", paths.config_home())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    while !paths.socket().exists() {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let (client, startup) = client::start(&paths, abcd(), &client::hello())
        .await
        .unwrap();
    assert_eq!(
        startup,
        Startup::Restarted {
            from: "0.0.0+0ddba11".into()
        }
    );
    assert_eq!(client.server.protocol, PROTOCOL);
    assert!(old_abcd.wait().unwrap().success());
    shut_down(client).await;
}

#[tokio::test]
async fn restart_and_stop_work_whether_or_not_abcd_runs() {
    let home = home();
    let paths = paths(home.path());
    assert_eq!(client::stop_running(&paths).await, None, "nothing to stop");
    let first = client::restart(&paths, abcd(), &client::hello())
        .await
        .unwrap();
    let first_pid = pid(&first).await;
    let second = client::restart(&paths, abcd(), &client::hello())
        .await
        .unwrap();
    assert_ne!(pid(&second).await, first_pid);
    assert_eq!(client::stop_running(&paths).await, second.server.pid);
    assert!(!paths.socket().exists());
}

#[tokio::test]
async fn a_stale_client_carries_on_without_restarting_abcd() {
    let home = home();
    let paths = paths(home.path());
    let (current, _) = client::start(&paths, abcd(), &client::hello())
        .await
        .unwrap();
    let old = me(PROTOCOL, "0.0.0+01dc11e7");
    let (stale, startup) = client::start(&paths, abcd(), &old).await.unwrap();
    assert_eq!(startup, Startup::StaleClient);
    assert_eq!(pid(&stale).await, pid(&current).await, "not restarted");
    shut_down(current).await;
}

#[tokio::test]
async fn a_client_of_another_protocol_is_told_to_reopen() {
    let home = home();
    let paths = paths(home.path());
    let (current, _) = client::start(&paths, abcd(), &client::hello())
        .await
        .unwrap();
    let newer = me(PROTOCOL + 1, "9.0.0+00000000");
    match client::start(&paths, abcd(), &newer).await {
        Err(StartError::Incompatible { ours, theirs }) => {
            assert_eq!((ours, theirs), (PROTOCOL + 1, PROTOCOL));
        }
        other => panic!("expected Incompatible, got {:?}", other.map(|(_, s)| s)),
    }
    let _: ServerStatus = current.call(Call::ServerStatus).await.unwrap();
    shut_down(current).await;
}

#[tokio::test]
async fn calls_from_many_tasks_get_their_own_answers() {
    let home = home();
    let paths = paths(home.path());
    let (client, _) = client::start(&paths, abcd(), &client::hello())
        .await
        .unwrap();
    let client = std::sync::Arc::new(client);
    let calls: Vec<_> = (0..20)
        .map(|_| {
            let client = client.clone();
            tokio::spawn(async move { client.call::<ServerStatus>(Call::ServerStatus).await })
        })
        .collect();
    for call in calls {
        assert_eq!(call.await.unwrap().unwrap().profile, "test");
    }
    let client = std::sync::Arc::try_unwrap(client).ok().unwrap();
    shut_down(client).await;
}
