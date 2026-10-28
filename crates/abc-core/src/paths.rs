//! Profiles, and where each keeps its settings, state, logs and socket.
//!
//! `--profile <name>` or `ABC_PROFILE` picks a profile, `stable` by default.
//! Settings live in `$XDG_CONFIG_HOME/abc/<profile>/`, and state, logs and the
//! socket in `$XDG_STATE_HOME/abc/<profile>/`, falling back to `~/.config` and
//! `~/.local/state`, on macOS too. Profiles share nothing.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The profile used when neither `--profile` nor `ABC_PROFILE` names one.
pub const DEFAULT_PROFILE: &str = "stable";
/// The environment variable that names the profile when `--profile` doesn't.
pub const PROFILE_ENV: &str = "ABC_PROFILE";
/// The longest profile name.
const MAX_NAME: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PathError {
    #[error(
        "{0:?} isn't a profile name: use up to {MAX_NAME} letters, digits, - and _, \
         starting with a letter or digit"
    )]
    BadProfile(String),
    #[error("can't tell where your home folder is: HOME isn't set")]
    NoHome,
}

/// A profile's name, checked to be safe as a folder name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile(String);

impl Profile {
    pub fn new(name: &str) -> Result<Self, PathError> {
        let bad = || PathError::BadProfile(name.into());
        let first = name.chars().next().ok_or_else(bad)?;
        let fits = name.len() <= MAX_NAME
            && first.is_ascii_alphanumeric()
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        if fits {
            Ok(Self(name.into()))
        } else {
            Err(bad())
        }
    }

    /// `flag` if given, else `ABC_PROFILE` if set, else `stable`.
    pub fn choose(
        flag: Option<&str>,
        env: impl Fn(&str) -> Option<OsString>,
    ) -> Result<Self, PathError> {
        let from_env = env(PROFILE_ENV).filter(|v| !v.is_empty());
        match (flag, from_env) {
            (Some(name), _) => Self::new(name),
            (None, Some(name)) => Self::new(&name.to_string_lossy()),
            (None, None) => Self::new(DEFAULT_PROFILE),
        }
    }

    pub fn name(&self) -> &str {
        &self.0
    }
}

/// A profile's folders and the files in them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    pub profile: Profile,
    /// Settings: `config.toml` and `themes/`.
    pub config_dir: PathBuf,
    /// State, logs and the socket.
    pub state_dir: PathBuf,
}

impl Paths {
    /// The folders for `profile`, reading `XDG_CONFIG_HOME`, `XDG_STATE_HOME`
    /// and `HOME` through `env`. A relative XDG path is ignored, as the XDG
    /// spec asks.
    pub fn new(
        profile: Profile,
        env: impl Fn(&str) -> Option<OsString>,
    ) -> Result<Self, PathError> {
        let home = || {
            env("HOME")
                .filter(|h| !h.is_empty())
                .map(PathBuf::from)
                .ok_or(PathError::NoHome)
        };
        let base = |var: &str, fallback: &str| -> Result<PathBuf, PathError> {
            match env(var).map(PathBuf::from) {
                Some(dir) if dir.is_absolute() => Ok(dir),
                _ => Ok(home()?.join(fallback)),
            }
        };
        let config_dir = base("XDG_CONFIG_HOME", ".config")?
            .join("abc")
            .join(profile.name());
        let state_dir = base("XDG_STATE_HOME", ".local/state")?
            .join("abc")
            .join(profile.name());
        Ok(Self {
            profile,
            config_dir,
            state_dir,
        })
    }

    /// The folders for `profile` from this process's environment.
    pub fn from_env(profile: Profile) -> Result<Self, PathError> {
        Self::new(profile, |var| std::env::var_os(var))
    }

    /// The folder holding every profile's settings, as `XDG_CONFIG_HOME` would name it.
    pub fn config_home(&self) -> &Path {
        self.config_dir
            .ancestors()
            .nth(2)
            .expect("built as <home>/abc/<profile>")
    }

    /// The folder holding every profile's state, as `XDG_STATE_HOME` would name it.
    pub fn state_home(&self) -> &Path {
        self.state_dir
            .ancestors()
            .nth(2)
            .expect("built as <home>/abc/<profile>")
    }

    pub fn config_file(&self) -> PathBuf {
        self.config_dir.join("config.toml")
    }

    /// Where abcd listens.
    pub fn socket(&self) -> PathBuf {
        self.state_dir.join("abcd.sock")
    }

    /// Held by the running abcd, so there's one per profile.
    pub fn lock(&self) -> PathBuf {
        self.state_dir.join("abcd.lock")
    }

    pub fn server_log(&self) -> PathBuf {
        self.state_dir.join("abcd.log")
    }

    pub fn client_log(&self) -> PathBuf {
        self.state_dir.join("abc.log")
    }

    /// The registered repositories and when worktrees were last opened.
    pub fn state_file(&self) -> PathBuf {
        self.state_dir.join("state.json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
        let vars: HashMap<String, OsString> = vars
            .iter()
            .map(|(k, v)| (k.to_string(), OsString::from(v)))
            .collect();
        move |k| vars.get(k).cloned()
    }

    #[test]
    fn the_profile_comes_from_the_flag_then_the_environment_then_stable() {
        let set = env(&[("ABC_PROFILE", "dev")]);
        assert_eq!(Profile::choose(Some("test"), &set).unwrap().name(), "test");
        assert_eq!(Profile::choose(None, &set).unwrap().name(), "dev");
        assert_eq!(Profile::choose(None, env(&[])).unwrap().name(), "stable");
        let empty = env(&[("ABC_PROFILE", "")]);
        assert_eq!(Profile::choose(None, empty).unwrap().name(), "stable");
    }

    #[test]
    fn profile_names_must_be_safe_folder_names() {
        for good in ["stable", "dev", "pr-142", "a_b", "0"] {
            assert!(Profile::new(good).is_ok(), "{good}");
        }
        for bad in ["", "-x", ".hidden", "../up", "a/b", "dév", &"x".repeat(33)] {
            assert_eq!(
                Profile::new(bad),
                Err(PathError::BadProfile(bad.into())),
                "{bad}"
            );
        }
    }

    #[test]
    fn folders_follow_xdg_then_fall_back_to_home() {
        let dev = || Profile::new("dev").unwrap();
        let xdg = env(&[
            ("XDG_CONFIG_HOME", "/x/config"),
            ("XDG_STATE_HOME", "/x/state"),
            ("HOME", "/home/me"),
        ]);
        let paths = Paths::new(dev(), xdg).unwrap();
        assert_eq!(
            paths.config_file(),
            PathBuf::from("/x/config/abc/dev/config.toml")
        );
        assert_eq!(paths.socket(), PathBuf::from("/x/state/abc/dev/abcd.sock"));
        assert_eq!(paths.state_home(), Path::new("/x/state"));
        assert_eq!(paths.config_home(), Path::new("/x/config"));

        let home = Paths::new(dev(), env(&[("HOME", "/home/me")])).unwrap();
        assert_eq!(home.config_dir, PathBuf::from("/home/me/.config/abc/dev"));
        assert_eq!(
            home.state_dir,
            PathBuf::from("/home/me/.local/state/abc/dev")
        );

        let relative = env(&[("XDG_STATE_HOME", "state"), ("HOME", "/home/me")]);
        let paths = Paths::new(dev(), relative).unwrap();
        assert_eq!(
            paths.state_dir, home.state_dir,
            "relative XDG paths are ignored"
        );

        assert_eq!(Paths::new(dev(), env(&[])), Err(PathError::NoHome));
    }
}
