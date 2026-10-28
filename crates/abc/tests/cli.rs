//! The abc binary's commands that work without abcd.

use std::process::Command;

fn abc(home: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_abc"))
        .args(args)
        .env("HOME", home)
        .env("XDG_STATE_HOME", home.join("state"))
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env_remove("ABC_PROFILE")
        .output()
        .unwrap()
}

#[test]
fn server_status_and_stop_never_start_abcd() {
    let home = tempfile::tempdir().unwrap();
    let status = abc(home.path(), &["--profile", "test", "server", "status"]);
    assert_eq!(status.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&status.stdout),
        "abcd isn't running · profile test\n"
    );
    let stop = abc(home.path(), &["server", "stop"]);
    assert!(stop.status.success());
    assert_eq!(
        String::from_utf8_lossy(&stop.stdout),
        "abcd isn't running · profile stable\n"
    );
    assert!(!home.path().join("state/abc/test/abcd.sock").exists());
}

#[test]
fn logs_say_when_there_is_none_yet() {
    let home = tempfile::tempdir().unwrap();
    let logs = abc(home.path(), &["--profile", "test", "logs"]);
    assert_eq!(logs.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&logs.stderr).starts_with("no log yet at "));
}

#[test]
fn a_bad_profile_name_is_refused() {
    let home = tempfile::tempdir().unwrap();
    let out = abc(home.path(), &["--profile", "../up", "server", "status"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("isn't a profile name"));
}
