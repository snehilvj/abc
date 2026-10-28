//! Settings, read by both abc and abcd: built-in defaults, then the profile's
//! `config.toml`, then `ABC_*` environment variables where a setting names
//! one. Each concern has its own table, and every option's docs and default
//! live here, so `abc config template` can print them all. Unknown keys are
//! errors.

use std::collections::BTreeMap;
use std::path::Path;

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
}

/// abcd, the background server.
#[derive(Debug, Clone, Config)]
#[config(layer_attr(serde(deny_unknown_fields)))]
pub struct Server {
    /// What abcd logs: a tracing filter, such as "info" or "abcd=debug".
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
