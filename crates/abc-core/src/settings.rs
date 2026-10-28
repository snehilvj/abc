//! Settings, read by both abc and abcd: built-in defaults, then the profile's
//! `config.toml`, then `ABC_*` environment variables where a setting names
//! one. Each concern has its own table, and every option's docs and default
//! live here, so `abc config template` can print them all. Unknown keys are
//! errors.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use confique::Config;
use serde::Deserialize;

#[derive(Debug, Clone, Config)]
#[config(layer_attr(serde(deny_unknown_fields)))]
pub struct Settings {
    /// "auto" picks dark or light from your terminal's background. Built in:
    /// "auto", "dark", "light", "ansi". Your own: `themes/<name>.toml` beside
    /// this file.
    #[config(default = "auto", env = "ABC_THEME")]
    pub theme: String,

    #[config(nested)]
    pub icons: Icons,

    #[config(nested)]
    pub layout: Layout,

    #[config(nested)]
    pub sidebar: Sidebar,

    #[config(nested)]
    pub switcher: Switcher,

    #[config(nested)]
    pub find: Find,

    #[config(nested)]
    pub viewer: Viewer,

    #[config(nested)]
    pub add_repo: AddRepo,

    #[config(nested)]
    pub keys: Keys,

    #[config(nested)]
    pub git: Git,

    #[config(nested)]
    pub github: Github,

    #[config(nested)]
    pub server: Server,
}

/// Icons. abc needs a Nerd Font.
#[derive(Debug, Clone, Config)]
#[config(layer_attr(serde(deny_unknown_fields)))]
pub struct Icons {
    /// Cells between an icon and its label.
    #[config(default = 2)]
    pub gap: u16,
}

/// The widths of the sidebar, explorer and viewer, in cells.
#[derive(Debug, Clone, Config)]
#[config(layer_attr(serde(deny_unknown_fields)))]
pub struct Layout {
    #[config(default = 38)]
    pub sidebar_width: u16,
    /// The explorer is as wide as the sidebar unless set.
    pub explorer_width: Option<u16>,
    /// Below this, the explorer narrows first, then the sidebar.
    #[config(default = 40)]
    pub viewer_min_width: u16,
    #[config(default = 20)]
    pub explorer_min_width: u16,
}

#[derive(Debug, Clone, Config)]
#[config(layer_attr(serde(deny_unknown_fields)))]
pub struct Sidebar {
    /// "detailed": two lines per worktree. "compact": one line.
    #[config(default = "detailed")]
    pub style: SidebarStyle,
    /// Blank rows between repositories.
    #[config(default = 1)]
    pub repo_gap: u16,
    /// Blank rows between worktrees.
    #[config(default = 0)]
    pub worktree_gap: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SidebarStyle {
    Detailed,
    Compact,
}

/// The switcher: ctrl-p, space f and space w.
#[derive(Debug, Clone, Config)]
#[config(layer_attr(serde(deny_unknown_fields)))]
pub struct Switcher {
    /// Rows of results.
    #[config(default = 10)]
    pub rows: usize,
}

/// Find in files: space /.
#[derive(Debug, Clone, Config)]
#[config(layer_attr(serde(deny_unknown_fields)))]
pub struct Find {
    /// Rows of results.
    #[config(default = 20)]
    pub rows: usize,
    /// The most matching lines one search returns.
    #[config(default = 1000)]
    pub max_lines: usize,
    /// How long typing pauses before a search starts, in milliseconds.
    #[config(default = 150)]
    pub delay_ms: u64,
}

#[derive(Debug, Clone, Config)]
#[config(layer_attr(serde(deny_unknown_fields)))]
pub struct Viewer {
    /// Spaces a tab takes.
    #[config(default = 4)]
    pub tab_width: usize,
    /// Larger files aren't opened, in megabytes.
    #[config(default = 5)]
    pub max_file_size_mb: u64,
}

/// Adding a repository: space r a, and abc repo add.
#[derive(Debug, Clone, Config)]
#[config(layer_attr(serde(deny_unknown_fields)))]
pub struct AddRepo {
    /// Where a repository added by its git URL is cloned: this folder, then
    /// its name.
    #[config(default = "~/code")]
    pub clone_dir: String,
    /// How long typing pauses before abcd checks the path, in milliseconds.
    #[config(default = 150)]
    pub check_delay_ms: u64,
}

/// Keys, over the built-in ones. Sequences are keys separated by spaces:
/// "space r a", "ctrl-p", "g g". An action of "none" unbinds a sequence.
#[derive(Debug, Clone, Config)]
#[config(layer_attr(serde(deny_unknown_fields)))]
pub struct Keys {
    /// Keys that work everywhere, such as "ctrl-p" = "switcher".
    #[config(default = {})]
    pub global: BTreeMap<String, String>,
    #[config(default = {})]
    pub sidebar: BTreeMap<String, String>,
    #[config(default = {})]
    pub explorer: BTreeMap<String, String>,
    #[config(default = {})]
    pub viewer: BTreeMap<String, String>,
    /// The viewer with a selection.
    #[config(default = {})]
    pub visual: BTreeMap<String, String>,
    /// Names for the which-key popup's groups, such as "space r" = "repos".
    #[config(default = {})]
    pub groups: BTreeMap<String, String>,

    #[config(nested)]
    pub which_key: WhichKey,
}

/// The popup that lists what can follow keys typed so far.
#[derive(Debug, Clone, Config)]
#[config(layer_attr(serde(deny_unknown_fields)))]
pub struct WhichKey {
    /// How long a sequence waits before the popup shows, in milliseconds.
    #[config(default = 300)]
    pub delay_ms: u64,
}

/// Worktree status from git.
#[derive(Debug, Clone, Config)]
#[config(layer_attr(serde(deny_unknown_fields)))]
pub struct Git {
    /// Seconds between rescans of worktrees and their git status, while abc
    /// is open.
    #[config(default = 5)]
    pub status_interval: u64,
}

/// Pull request status from GitHub, through the gh CLI.
#[derive(Debug, Clone, Config)]
#[config(layer_attr(serde(deny_unknown_fields)))]
pub struct Github {
    #[config(default = true)]
    pub enabled: bool,
    /// Seconds between refreshes of pull request status.
    #[config(default = 60)]
    pub pr_interval: u64,
    /// The gh command, if it isn't on PATH as gh.
    #[config(default = "gh")]
    pub gh: String,
}

/// abcd, the background server.
#[derive(Debug, Clone, Config)]
#[config(layer_attr(serde(deny_unknown_fields)))]
pub struct Server {
    /// What abcd and abc log: a tracing filter, such as "info" or "abcd=debug".
    #[config(default = "info", env = "ABC_LOG")]
    pub log: String,
}

impl Settings {
    /// Defaults, then `file` if it exists, then the environment.
    pub fn load(file: &Path) -> Result<Self, confique::Error> {
        Self::builder().env().file(file).load()
    }

    /// Every setting at its default.
    pub fn defaults() -> Self {
        Self::builder().load().expect("every setting has a default")
    }

    /// Defaults, then `file`, ignoring the environment.
    pub fn from_file(file: &Path) -> Result<Self, confique::Error> {
        Self::builder().file(file).load()
    }

    /// Why settings didn't load, with the cause TOML gave: the file,
    /// then the key or value and where it is.
    pub fn describe(error: &confique::Error) -> String {
        let mut text = error.to_string();
        let mut cause = std::error::Error::source(error);
        while let Some(why) = cause {
            text.push_str(": ");
            text.push_str(why.to_string().trim());
            cause = why.source();
        }
        text
    }

    /// Every option with its docs and default, as a `config.toml` to start from.
    pub fn template(profile: &str, file: &Path) -> String {
        let header = format!(
            "# abc settings · profile \"{profile}\" · {}\n\
             # Every key is optional; the values below are the defaults.\n\
             # Generated by: abc config template\n\n",
            file.display()
        );
        let options = confique::toml::FormatOptions::default();
        header + &confique::toml::template::<Self>(options)
    }
}

/// Notices when a settings file changes: written, created or removed.
#[derive(Debug, Clone)]
pub struct Watch {
    file: PathBuf,
    seen: Option<(SystemTime, u64)>,
}

impl Watch {
    /// Watches `file`, as it is now.
    pub fn new(file: &Path) -> Self {
        Self {
            file: file.to_path_buf(),
            seen: stamp(file),
        }
    }

    /// Whether the file has changed since this was last asked.
    pub fn changed(&mut self) -> bool {
        let now = stamp(&self.file);
        let changed = now != self.seen;
        self.seen = now;
        changed
    }
}

/// When a file was last written, and its size: a write that keeps the same
/// time to the tick still changes the size, mostly.
fn stamp(file: &Path) -> Option<(SystemTime, u64)> {
    let meta = std::fs::metadata(file).ok()?;
    Some((meta.modified().ok()?, meta.len()))
}

/// A settings error as one line for the status line: `config.toml line 2:
/// unknown field `widht``, without TOML's picture of the line.
pub fn one_line(why: &str) -> String {
    let message = why.lines().last().unwrap_or(why).trim();
    let message = message.split(", expected").next().unwrap_or(message);
    match why.split("TOML parse error at line ").nth(1) {
        Some(rest) => {
            let line: String = rest.chars().take_while(char::is_ascii_digit).collect();
            format!("config.toml line {line}: {message}")
        }
        None => message.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, toml: &str) -> std::path::PathBuf {
        let file = dir.join("config.toml");
        std::fs::write(&file, toml).unwrap();
        file
    }

    #[test]
    fn without_a_file_everything_has_its_default() {
        let dir = tempfile::tempdir().unwrap();
        let settings = Settings::from_file(&dir.path().join("config.toml")).unwrap();
        assert_eq!(settings.theme, Settings::defaults().theme);
        assert_eq!(settings.theme, "auto");
        assert_eq!(settings.add_repo.clone_dir, "~/code");
        assert_eq!(settings.sidebar.style, SidebarStyle::Detailed);
        assert_eq!(settings.layout.explorer_width, None);
        assert!(settings.keys.global.is_empty());
    }

    #[test]
    fn the_file_sets_what_it_names() {
        let dir = tempfile::tempdir().unwrap();
        let file = write(
            dir.path(),
            "theme = \"light\"\n\
             [sidebar]\nstyle = \"compact\"\n\
             [keys.global]\n\"space x\" = \"quit\"\n\
             [add_repo]\nclone_dir = \"/srv/src\"\n",
        );
        let settings = Settings::from_file(&file).unwrap();
        assert_eq!(settings.theme, "light");
        assert_eq!(settings.sidebar.style, SidebarStyle::Compact);
        assert_eq!(settings.sidebar.repo_gap, 1, "the rest keep their defaults");
        assert_eq!(settings.keys.global["space x"], "quit");
        assert_eq!(settings.add_repo.clone_dir, "/srv/src");
    }

    #[test]
    fn unknown_keys_and_wrong_values_are_errors() {
        let dir = tempfile::tempdir().unwrap();
        let typo = write(dir.path(), "[sidebar]\nwidht = 3\n");
        let error = Settings::describe(&Settings::from_file(&typo).unwrap_err());
        assert!(error.contains("widht"), "{error}");
        let wrong = write(dir.path(), "[sidebar]\nstyle = \"fancy\"\n");
        assert!(Settings::from_file(&wrong).is_err());
    }

    #[test]
    fn a_watch_notices_writes_and_removal() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("config.toml");
        let mut watch = Watch::new(&file);
        assert!(!watch.changed(), "nothing yet");
        std::fs::write(&file, "theme = \"dark\"\n").unwrap();
        assert!(watch.changed(), "created");
        assert!(!watch.changed(), "once");
        std::fs::write(&file, "theme = \"light\"\n").unwrap();
        assert!(watch.changed(), "written");
        std::fs::remove_file(&file).unwrap();
        assert!(watch.changed(), "removed");
    }

    #[test]
    fn errors_fit_on_one_line() {
        let dir = tempfile::tempdir().unwrap();
        let file = write(dir.path(), "theme = \"dark\"\n[sidebar]\nwidht = 3\n");
        let why = Settings::describe(&Settings::from_file(&file).unwrap_err());
        assert_eq!(one_line(&why), "config.toml line 3: unknown field `widht`");
        assert_eq!(
            one_line("keys.global: \"x\": bad"),
            "keys.global: \"x\": bad"
        );
    }

    #[test]
    fn the_template_lists_every_option_and_loads_as_the_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("config.toml");
        let template = Settings::template("dev", &file);
        for table in ["[icons]", "[layout]", "[keys.which_key]", "[server]"] {
            assert!(template.contains(table), "{table} missing");
        }
        assert!(template.contains("Can also be specified via environment variable `ABC_LOG`"));
        std::fs::write(&file, &template).unwrap();
        let settings = Settings::from_file(&file).unwrap();
        assert_eq!(settings.find.max_lines, 1000);
    }
}
