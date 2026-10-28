//! Reading what git says about a repository's worktrees. Running git is
//! separate from parsing what it prints, so the parsing is tested on its own.

use std::path::PathBuf;

use abc_core::model::GitStatus;

/// One record of `git worktree list --porcelain -z`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Listed {
    pub path: PathBuf,
    /// The commit checked out; none in a bare repository.
    pub head: Option<String>,
    /// The branch checked out, without `refs/heads/`; none when detached.
    pub branch: Option<String>,
    /// The repository itself, when it's bare: not a worktree to show.
    pub bare: bool,
    pub locked: bool,
    /// git can no longer find the worktree's folder.
    pub prunable: bool,
}

/// Parses `git worktree list --porcelain -z`: records of NUL-ended lines,
/// each record ended by an empty line. The main worktree, or the bare
/// repository, comes first. Lines git may add later are skipped.
pub fn parse_worktrees(out: &[u8]) -> Vec<Listed> {
    let text = String::from_utf8_lossy(out);
    let mut listed = Vec::new();
    let mut current: Option<Listed> = None;
    for line in text.split('\0') {
        if line.is_empty() {
            listed.extend(current.take());
            continue;
        }
        let (key, value) = line.split_once(' ').unwrap_or((line, ""));
        if key == "worktree" {
            listed.extend(current.take());
            current = Some(Listed {
                path: PathBuf::from(value),
                ..Listed::default()
            });
            continue;
        }
        let Some(record) = current.as_mut() else {
            continue;
        };
        match key {
            "HEAD" => record.head = Some(value.into()),
            "branch" => {
                record.branch = Some(value.strip_prefix("refs/heads/").unwrap_or(value).into());
            }
            "bare" => record.bare = true,
            "locked" => record.locked = true,
            "prunable" => record.prunable = true,
            _ => {}
        }
    }
    listed.extend(current);
    listed
}

/// Parses `git status --porcelain=v2 --branch -z`: the `# branch.*` headers,
/// then one NUL-ended entry per changed path. A rename (`2`) is followed by
/// its original path as a field of its own. Untracked files count as
/// changed; ignored ones don't.
pub fn parse_status(out: &[u8]) -> GitStatus {
    let text = String::from_utf8_lossy(out);
    let mut status = GitStatus::default();
    let mut fields = text.split('\0');
    while let Some(field) = fields.next() {
        if let Some(header) = field.strip_prefix("# ") {
            let (key, value) = header.split_once(' ').unwrap_or((header, ""));
            match key {
                "branch.upstream" => status.upstream = true,
                "branch.ab" => {
                    for count in value.split(' ') {
                        let number = |n: &str| n.parse().unwrap_or(0);
                        if let Some(n) = count.strip_prefix('+') {
                            status.ahead = number(n);
                        } else if let Some(n) = count.strip_prefix('-') {
                            status.behind = number(n);
                        }
                    }
                }
                _ => {}
            }
            continue;
        }
        match field.chars().next() {
            Some('1' | 'u' | '?') => status.changed += 1,
            Some('2') => {
                status.changed += 1;
                fields.next();
            }
            _ => {}
        }
    }
    status
}

#[cfg(test)]
mod tests {
    use super::*;

    fn records(lines: &[&[&str]]) -> Vec<u8> {
        let mut out = String::new();
        for record in lines {
            for line in *record {
                out.push_str(line);
                out.push('\0');
            }
            out.push('\0');
        }
        out.into_bytes()
    }

    #[test]
    fn main_linked_detached_locked_and_prunable_worktrees() {
        let out = records(&[
            &[
                "worktree /code/app",
                "HEAD 1111111111",
                "branch refs/heads/main",
            ],
            &[
                "worktree /code/wt/app/feat x",
                "HEAD 2222222222",
                "branch refs/heads/feat/x",
                "locked",
            ],
            &[
                "worktree /code/wt/app/bisect",
                "HEAD 3333333333",
                "detached",
            ],
            &[
                "worktree /code/wt/app/gone",
                "HEAD 4444444444",
                "branch refs/heads/old",
                "prunable gitdir file points to non-existent location",
            ],
        ]);
        let listed = parse_worktrees(&out);
        assert_eq!(listed.len(), 4);
        assert_eq!(
            listed[0],
            Listed {
                path: "/code/app".into(),
                head: Some("1111111111".into()),
                branch: Some("main".into()),
                ..Listed::default()
            }
        );
        assert_eq!(
            listed[1].path,
            PathBuf::from("/code/wt/app/feat x"),
            "spaces kept"
        );
        assert_eq!(listed[1].branch.as_deref(), Some("feat/x"));
        assert!(listed[1].locked);
        assert_eq!(
            (listed[2].branch.as_deref(), listed[2].locked),
            (None, false)
        );
        assert!(listed[3].prunable);
    }

    #[test]
    fn a_bare_repository_comes_first_without_a_head() {
        let out = records(&[
            &["worktree /code/app.git", "bare"],
            &[
                "worktree /code/wt/app/main",
                "HEAD 1111111111",
                "branch refs/heads/main",
            ],
        ]);
        let listed = parse_worktrees(&out);
        assert!(listed[0].bare && listed[0].head.is_none());
        assert!(!listed[1].bare);
    }

    #[test]
    fn status_counts_changes_ahead_and_behind() {
        let out = [
            "# branch.oid 0a0461d7",
            "# branch.head main",
            "# branch.upstream origin/main",
            "# branch.ab +1 -3",
            "1 .M N... 100644 100644 100644 6178 6178 b",
            "2 R. N... 100644 100644 100644 7898 7898 R100 c",
            "a",
            "? n",
            "! target",
            "",
        ]
        .join("\0");
        assert_eq!(
            parse_status(out.as_bytes()),
            GitStatus {
                changed: 3,
                ahead: 1,
                behind: 3,
                upstream: true
            },
            "the rename's old path isn't another change; ignored files aren't changes"
        );
        let clean = "# branch.oid 0a04\0# branch.head (detached)\0";
        assert_eq!(parse_status(clean.as_bytes()), GitStatus::default());
    }

    #[test]
    fn lines_from_a_newer_git_are_skipped() {
        let out = records(&[&[
            "worktree /code/app",
            "HEAD 1111111111",
            "branch refs/heads/main",
            "sparse",
        ]]);
        assert_eq!(parse_worktrees(&out).len(), 1);
        assert!(parse_worktrees(b"").is_empty());
    }
}
