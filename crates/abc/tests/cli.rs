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

#[test]
fn config_path_template_and_check() {
    let home = tempfile::tempdir().unwrap();
    let path = abc(home.path(), &["--profile", "test", "config", "path"]);
    assert_eq!(
        String::from_utf8_lossy(&path.stdout),
        "~/config/abc/test/config.toml\n"
    );
    let ok = abc(home.path(), &["--profile", "test", "config", "check"]);
    assert!(ok.status.success(), "no file is fine: every default");

    let template = abc(home.path(), &["--profile", "test", "config", "template"]);
    let file = home.path().join("config/abc/test/config.toml");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, &template.stdout).unwrap();
    let ok = abc(home.path(), &["--profile", "test", "config", "check"]);
    assert_eq!(
        String::from_utf8_lossy(&ok.stdout),
        "ok ~/config/abc/test/config.toml\n",
        "the template is valid"
    );

    std::fs::write(
        &file,
        "[sidebar]\nwidht = 3\n[keys.global]\n\"space q\" = \"teleport\"\n",
    )
    .unwrap();
    let bad = abc(home.path(), &["--profile", "test", "config", "check"]);
    assert_eq!(bad.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&bad.stderr).contains("unknown field `widht`"));
}
