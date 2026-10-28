//! A worktree's files: listing them as the explorer shows them, reading one
//! for the viewer, and finding text in them. Everything here blocks; callers
//! run it off the async threads.

use std::fs::File;
use std::io::Read;
use std::path::{Component, Path};
use std::sync::atomic::{AtomicBool, Ordering};

use abc_core::model::{FileContents, FoundFile, FoundLine};
use abc_core::text;

/// How much of a file is looked at for a NUL byte, to call it binary.
const SNIFF: usize = 8 * 1024;

/// Every file in `worktree`, relative and `/`-separated, sorted: dotfiles
/// included, `.git` and what `.gitignore` leaves out excluded.
pub fn list(worktree: &Path) -> Vec<String> {
    let walk = ignore::WalkBuilder::new(worktree)
        .hidden(false)
        .filter_entry(|entry| entry.file_name() != ".git")
        .build();
    let mut files: Vec<String> = walk
        .flatten()
        .filter(|entry| entry.file_type().is_some_and(|t| t.is_file()))
        .filter_map(|entry| {
            let relative = entry.path().strip_prefix(worktree).ok()?;
            let parts: Vec<String> = relative
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect();
            Some(parts.join("/"))
        })
        .collect();
    files.sort();
    files
}

/// Why a file can't be read.
#[derive(Debug, PartialEq, Eq)]
pub enum Unreadable {
    /// The path would leave the worktree.
    Outside,
    Io(String),
}

/// When a file was last written, and its size: what tells abcd it changed.
pub type Stamp = (std::time::SystemTime, u64);

/// `path`'s stamp, or none if it's gone or would leave the worktree.
pub fn stamp(worktree: &Path, path: &str) -> Option<Stamp> {
    let file = inside(worktree, path).ok()?;
    let meta = std::fs::metadata(file).ok()?;
    Some((meta.modified().ok()?, meta.len()))
}

/// `path` under `worktree`, resolved, if it stays inside it.
fn inside(worktree: &Path, path: &str) -> Result<std::path::PathBuf, Unreadable> {
    let relative = Path::new(path);
    if relative.is_absolute()
        || relative
            .components()
            .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return Err(Unreadable::Outside);
    }
    let io = |e: std::io::Error| Unreadable::Io(e.to_string());
    let root = std::fs::canonicalize(worktree).map_err(io)?;
    let file = std::fs::canonicalize(root.join(relative)).map_err(io)?;
    if file.starts_with(&root) {
        Ok(file)
    } else {
        Err(Unreadable::Outside)
    }
}

/// `path`, relative to `worktree`, for the viewer: its text, or that it's
/// binary or over `limit` bytes. A path that would leave the worktree,
/// through `..` or a link, is refused.
pub fn read(worktree: &Path, path: &str, limit: u64) -> Result<FileContents, Unreadable> {
    let io = |e: std::io::Error| Unreadable::Io(e.to_string());
    let file = inside(worktree, path)?;
    let size = std::fs::metadata(&file).map_err(io)?.len();
    if size > limit {
        return Ok(FileContents::TooLarge { size, limit });
    }
    let mut bytes = Vec::with_capacity(size as usize);
    File::open(&file)
        .map_err(io)?
        .read_to_end(&mut bytes)
        .map_err(io)?;
    if bytes[..bytes.len().min(SNIFF)].contains(&0) {
        return Ok(FileContents::Binary { size });
    }
    Ok(FileContents::Text {
        text: String::from_utf8_lossy(&bytes).into_owned(),
    })
}

/// Finds `query` in `files` of `worktree`, handing each file with matches to
/// `found` as it goes, until `max_lines` lines have matched or `cancelled` is
/// set. Binary files and ones over `limit` bytes are skipped. Says whether it
/// stopped at `max_lines` with more to find.
pub fn search(
    worktree: &Path,
    files: &[String],
    query: &str,
    max_lines: usize,
    limit: u64,
    cancelled: &AtomicBool,
    mut found: impl FnMut(FoundFile),
) -> bool {
    let mut lines = 0;
    for path in files {
        if cancelled.load(Ordering::Relaxed) {
            return false;
        }
        let Ok(FileContents::Text { text }) = read(worktree, path, limit) else {
            continue;
        };
        let mut file = FoundFile {
            path: path.clone(),
            lines: Vec::new(),
        };
        for (i, line) in text.split('\n').enumerate() {
            let line = line.strip_suffix('\r').unwrap_or(line);
            if !text::contains(query, line) {
                continue;
            }
            if lines == max_lines {
                if !file.lines.is_empty() {
                    found(file);
                }
                return true;
            }
            lines += 1;
            file.lines.push(FoundLine {
                number: i as u32 + 1,
                text: line.to_string(),
            });
        }
        if !file.lines.is_empty() {
            found(file);
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::tests::repo;

    /// A worktree with files of every kind.
    fn worktree() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let app = repo(dir.path(), "app", &[]);
        let write = |path: &str, bytes: &[u8]| {
            let path = app.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, bytes).unwrap();
        };
        write(".gitignore", b"target/\n*.log\n");
        write("src/main.rs", b"fn main() {\n    let gstin = GSTIN;\n}\n");
        write("src/lib.rs", b"// GstClient\r\npub mod gst;\r\n");
        write(".github/check.yml", b"on: push\n");
        write("target/debug/out", b"gst");
        write("run.log", b"gst");
        write("logo.png", b"\x89PNG\0\0gst");
        (dir, app)
    }

    #[test]
    fn files_are_listed_as_the_explorer_shows_them() {
        let (_dir, app) = worktree();
        assert_eq!(
            list(&app),
            [
                ".github/check.yml",
                ".gitignore",
                "logo.png",
                "src/lib.rs",
                "src/main.rs"
            ]
        );
    }

    #[test]
    fn a_file_is_read_as_text_binary_or_too_large() {
        let (_dir, app) = worktree();
        let main = read(&app, "src/main.rs", 1024).unwrap();
        assert!(matches!(main, FileContents::Text { ref text } if text.starts_with("fn main")));
        assert_eq!(
            read(&app, "logo.png", 1024),
            Ok(FileContents::Binary { size: 9 })
        );
        assert_eq!(
            read(&app, "src/main.rs", 10),
            Ok(FileContents::TooLarge {
                size: 37,
                limit: 10
            })
        );
        assert!(matches!(
            read(&app, "nope.rs", 1024),
            Err(Unreadable::Io(_))
        ));
    }

    #[test]
    fn nothing_outside_the_worktree_is_read() {
        let (dir, app) = worktree();
        std::fs::write(dir.path().join("secret"), "x").unwrap();
        assert_eq!(read(&app, "../secret", 1024), Err(Unreadable::Outside));
        assert_eq!(read(&app, "/etc/hosts", 1024), Err(Unreadable::Outside));
        std::os::unix::fs::symlink(dir.path().join("secret"), app.join("link")).unwrap();
        assert_eq!(read(&app, "link", 1024), Err(Unreadable::Outside));
    }

    #[test]
    fn search_finds_lines_smart_case_and_stops_at_the_cap() {
        let (_dir, app) = worktree();
        let files = list(&app);
        let none = AtomicBool::new(false);
        let mut found = Vec::new();
        let capped = search(&app, &files, "gst", 10, 1024, &none, |f| found.push(f));
        assert!(!capped);
        let paths: Vec<&str> = found.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, ["src/lib.rs", "src/main.rs"], "not the binary file");
        assert_eq!(found[0].lines[0].text, "// GstClient", "without the \\r");
        assert_eq!(found[1].lines[0].number, 2);

        let mut found = Vec::new();
        search(&app, &files, "GST", 10, 1024, &none, |f| found.push(f));
        assert_eq!(found.len(), 1, "a capital makes case count");

        let mut found = Vec::new();
        assert!(search(&app, &files, "gst", 2, 1024, &none, |f| found.push(f)));
        assert_eq!(found.iter().map(|f| f.lines.len()).sum::<usize>(), 2);

        let cancelled = AtomicBool::new(true);
        let mut found = Vec::new();
        search(&app, &files, "gst", 10, 1024, &cancelled, |f| found.push(f));
        assert!(found.is_empty());
    }
}
