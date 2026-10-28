//! Where a repository to add comes from: a folder on this machine, or a git
//! URL to clone. abc and abcd read the text alike.

/// A git URL, such as `https://github.com/owner/repo` or `git@github.com:owner/repo.git`.
pub fn is_url(text: &str) -> bool {
    text.contains("://") || text.starts_with("git@")
}

/// The last part of a folder or URL, without `.git`: `repo` for
/// `git@github.com:owner/repo.git`. Nothing for `~` alone.
pub fn default_name(text: &str) -> &str {
    let text = text.trim().trim_end_matches('/');
    let text = text.strip_suffix(".git").unwrap_or(text);
    let name = text.rsplit(['/', ':']).next().unwrap_or("");
    if name == "~" { "" } else { name }
}

/// Whether two remote URLs name the same repository, however written:
/// `git@github.com:o/r.git`, `ssh://git@github.com/o/r` and
/// `https://github.com/o/r/` all do.
pub fn same_remote(a: &str, b: &str) -> bool {
    remote_key(a) == remote_key(b)
}

/// Who owns the repository at a remote URL: `privue` for
/// `git@github.com:privue/ecourts.git`.
pub fn owner(url: &str) -> Option<String> {
    let key = remote_key(url);
    let mut parts = key.rsplit('/');
    parts.next()?;
    parts.next().filter(|o| !o.is_empty()).map(String::from)
}

/// A remote reduced to host and path, lower-cased: `github.com/o/r`.
fn remote_key(url: &str) -> String {
    let url = url.trim();
    let (rest, scp) = match url.split_once("://") {
        Some((_, rest)) => (rest, false),
        None => (url, true),
    };
    // user@host
    let rest = match rest.split_once('@') {
        Some((user, host)) if !user.contains('/') => host,
        _ => rest,
    };
    // host:path, scp style
    let rest = if scp && !rest.starts_with('/') {
        rest.replacen(':', "/", 1)
    } else {
        rest.to_string()
    };
    let rest = rest.trim_end_matches('/');
    let rest = rest.strip_suffix(".git").unwrap_or(rest);
    rest.to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_repository_however_its_url_is_written() {
        let https = "https://github.com/Privue/ecourts";
        for same in [
            "git@github.com:privue/ecourts.git",
            "ssh://git@github.com/privue/ecourts",
            "https://github.com/privue/ecourts.git/",
        ] {
            assert!(same_remote(https, same), "{same}");
        }
        assert!(!same_remote(https, "https://github.com/privue/notes"));
        assert!(same_remote("file:///srv/app.git", "/srv/app"));
    }

    #[test]
    fn the_owner_is_read_from_the_url() {
        assert_eq!(
            owner("git@github.com:Privue/ecourts.git").as_deref(),
            Some("privue")
        );
        assert_eq!(owner("https://github.com/cli/cli").as_deref(), Some("cli"));
        assert_eq!(owner("ecourts"), None);
    }

    #[test]
    fn urls_and_names() {
        assert!(is_url("file:///srv/app.git") && is_url("git@github.com:o/r"));
        assert!(!is_url("~/code/app"));
        assert_eq!(default_name("git@github.com:privue/agents.git"), "agents");
    }
}
