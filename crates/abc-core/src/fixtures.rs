//! Fixture repositories to run abc against, made with the git CLI and none
//! of your git config: every kind of worktree the sidebar shows, files in the
//! languages the viewer highlights, ignored folders, a 10,000-line file and a
//! binary one. `just fixtures` makes them and adds them to the dev profile.

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A repository made, to add under `name`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fixture {
    pub name: String,
    /// The main worktree, or a bare repository's folder.
    pub path: PathBuf,
}

/// git in `dir` with no user or system config, a fixed identity and fixed dates.
pub fn git(dir: &Path, args: &[&str]) -> io::Result<()> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "user.name=abc fixtures",
            "-c",
            "user.email=fixtures@example.com",
            "-c",
            "init.defaultBranch=main",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_DATE", "2026-10-01T09:00:00Z")
        .env("GIT_COMMITTER_DATE", "2026-10-01T09:00:00Z")
        .output()?;
    if out.status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        )))
    }
}

/// Writes `bytes` to `path`, making its folders first.
fn put(path: &Path, bytes: impl AsRef<[u8]>) -> io::Result<()> {
    std::fs::create_dir_all(path.parent().expect("files are in folders"))?;
    std::fs::write(path, bytes)
}

fn write(root: &Path, files: &[(&str, &str)]) -> io::Result<()> {
    for (path, text) in files {
        put(&root.join(path), text)?;
    }
    Ok(())
}

fn commit(dir: &Path, message: &str) -> io::Result<()> {
    git(dir, &["add", "-A"])?;
    git(dir, &["commit", "-q", "-m", message])
}

/// The file fixtures leave in their folder, so a later run knows it's
/// theirs to replace.
const MARKER: &str = ".abc-fixtures";

/// Makes the fixtures in `root`, replacing any made there before. A folder
/// that isn't empty and wasn't made by fixtures is refused, never deleted.
pub fn generate(root: &Path) -> io::Result<Vec<Fixture>> {
    if root.exists() {
        let empty = std::fs::read_dir(root)?.next().is_none();
        if !empty && !root.join(MARKER).exists() {
            return Err(io::Error::other(format!(
                "{} isn't empty and wasn't made by abc's fixtures: choose another folder",
                root.display()
            )));
        }
        std::fs::remove_dir_all(root)?;
    }
    std::fs::create_dir_all(root)?;
    std::fs::write(
        root.join(MARKER),
        "Made by abc's fixtures; run again to replace.\n",
    )?;
    let root = std::fs::canonicalize(root)?;
    Ok(vec![
        shop(&root)?,
        scraper(&root)?,
        notes(&root)?,
        old_tool(&root)?,
    ])
}

/// A TypeScript app pushed to an upstream: a linked worktree with changes,
/// a locked one, a detached one, a commit ahead, ignored folders, a large
/// file and a binary one.
fn shop(root: &Path) -> io::Result<Fixture> {
    let shop = root.join("shop");
    std::fs::create_dir_all(&shop)?;
    git(&shop, &["init", "-q"])?;
    write(
        &shop,
        &[
            (".gitignore", "node_modules/\ndist/\n"),
            (
                "README.md",
                "# shop\n\nA small storefront: the cart, its totals, checkout.\n",
            ),
            (
                "package.json",
                "{\n  \"name\": \"shop\",\n  \"private\": true,\n  \"scripts\": { \"build\": \"tsc\" }\n}\n",
            ),
            (
                "src/cart.ts",
                "export type Item = { sku: string; price: number; quantity: number }\n\n\
                 export function subtotal(items: Item[]): number {\n  \
                 return items.reduce((sum, item) => sum + item.price * item.quantity, 0)\n}\n",
            ),
            (
                "src/totals.ts",
                "import { Item, subtotal } from \"./cart\"\n\n\
                 // GST is added on the subtotal and rounded to the paise.\n\
                 export const GST_RATE = 0.18\n\n\
                 export function total(items: Item[]): number {\n  \
                 return Math.round(subtotal(items) * (1 + GST_RATE) * 100) / 100\n}\n",
            ),
            (
                "src/app.tsx",
                "import { total } from \"./totals\"\n\n\
                 export function Checkout({ items }: { items: Item[] }) {\n  \
                 return <button>Pay ₹{total(items)}</button>\n}\n",
            ),
            ("node_modules/react/index.js", "module.exports = {}\n"),
            ("dist/app.js", "// built\n"),
        ],
    )?;
    let rows: String = (1..=10_000)
        .map(|n| format!("{n},SKU-{n:05},{}.{:02}\n", n % 997, n % 100))
        .collect();
    put(&shop.join("data/prices.csv"), rows)?;
    let mut png = vec![
        0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0x0d,
    ];
    png.extend_from_slice(b"IHDR\0\0\0\x10\0\0\0\x10");
    put(&shop.join("assets/logo.png"), png)?;
    commit(&shop, "Start the shop")?;
    write(
        &shop,
        &[(
            "src/cart.ts",
            "export type Item = { sku: string; price: number; quantity: number }\n\nexport function subtotal(items: Item[]): number {\n  return items.reduce((sum, item) => sum + item.price * item.quantity, 0)\n}\n\nexport const empty = (items: Item[]) => items.length === 0\n",
        )],
    )?;
    commit(&shop, "Tell an empty cart")?;

    // An upstream it tracks, then a commit not pushed yet.
    let upstream = root.join("upstream/shop.git");
    git(
        root,
        &["init", "-q", "--bare", &upstream.display().to_string()],
    )?;
    git(
        &shop,
        &["remote", "add", "origin", &upstream.display().to_string()],
    )?;
    git(&shop, &["push", "-q", "-u", "origin", "main"])?;
    write(
        &shop,
        &[(
            "README.md",
            "# shop\n\nA small storefront: the cart, its totals, checkout.\n\nRun `npm run build`.\n",
        )],
    )?;
    commit(&shop, "Say how to build")?;

    let linked = |branch: &str, folder: &str| -> io::Result<PathBuf> {
        let path = root.join("wt").join(folder);
        git(
            &shop,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                branch,
                &path.display().to_string(),
            ],
        )?;
        Ok(path)
    };
    let checkout = linked("feat/checkout", "shop-checkout")?;
    write(
        &checkout,
        &[(
            "src/checkout.tsx",
            "export function Pay() {\n  return null\n}\n",
        )],
    )?;
    write(
        &checkout,
        &[(
            "src/app.tsx",
            "import { total } from \"./totals\"\nimport { Pay } from \"./checkout\"\n",
        )],
    )?;
    let totals = linked("fix/totals", "shop-totals")?;
    git(
        &shop,
        &[
            "worktree",
            "lock",
            &totals.display().to_string(),
            "--reason",
            "reviewing",
        ],
    )?;
    let bisect = root.join("wt/shop-bisect");
    git(
        &shop,
        &[
            "worktree",
            "add",
            "-q",
            "--detach",
            &bisect.display().to_string(),
            "HEAD~1",
        ],
    )?;
    Ok(Fixture {
        name: "shop".into(),
        path: shop,
    })
}

/// Python and Rust, with a worktree whose folder has gone.
fn scraper(root: &Path) -> io::Result<Fixture> {
    let scraper = root.join("scraper");
    std::fs::create_dir_all(&scraper)?;
    git(&scraper, &["init", "-q"])?;
    write(
        &scraper,
        &[
            (".gitignore", "target/\n__pycache__/\n"),
            (
                "scraper/__init__.py",
                "\"\"\"Fetches court listings.\"\"\"\n",
            ),
            (
                "scraper/fetch.py",
                "import time\n\nRETRIES = 3\n\n\ndef fetch(url: str, attempt: int = 0) -> str:\n    \
                 # Back off a little more each time.\n    time.sleep(2 ** attempt)\n    return url\n",
            ),
            (
                "Cargo.toml",
                "[package]\nname = \"parse\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
            ),
            (
                "src/main.rs",
                "/// Reads one listing per line.\nfn main() {\n    for line in std::io::stdin().lines() {\n        \
                 println!(\"{}\", line.unwrap().trim());\n    }\n}\n",
            ),
            ("target/debug/parse", "built\n"),
        ],
    )?;
    commit(&scraper, "Fetch and parse listings")?;
    let retry = root.join("wt/scraper-retry");
    git(
        &scraper,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "feat/retry",
            &retry.display().to_string(),
        ],
    )?;
    let gone = root.join("wt/scraper-old");
    git(
        &scraper,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "old/idea",
            &gone.display().to_string(),
        ],
    )?;
    std::fs::remove_dir_all(gone)?;
    Ok(Fixture {
        name: "scraper".into(),
        path: scraper,
    })
}

/// A bare repository with a linked worktree.
fn notes(root: &Path) -> io::Result<Fixture> {
    let source = root.join("upstream/notes-source");
    std::fs::create_dir_all(&source)?;
    git(&source, &["init", "-q"])?;
    write(&source, &[("notes.md", "# Notes\n\n- one\n")])?;
    commit(&source, "Start notes")?;
    let bare = root.join("notes.git");
    git(
        root,
        &[
            "clone",
            "-q",
            "--bare",
            &source.display().to_string(),
            &bare.display().to_string(),
        ],
    )?;
    let main = root.join("wt/notes-main");
    git(
        &bare,
        &["worktree", "add", "-q", &main.display().to_string(), "main"],
    )?;
    Ok(Fixture {
        name: "notes".into(),
        path: bare,
    })
}

/// A repository to add and then delete, to see a missing one.
fn old_tool(root: &Path) -> io::Result<Fixture> {
    let old = root.join("old-tool");
    std::fs::create_dir_all(&old)?;
    git(&old, &["init", "-q"])?;
    write(&old, &[("README.md", "# old-tool\n")])?;
    commit(&old, "Begin")?;
    Ok(Fixture {
        name: "old-tool".into(),
        path: old,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_fixtures_didnt_make_is_never_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let precious = dir.path().join("code");
        std::fs::create_dir_all(&precious).unwrap();
        std::fs::write(precious.join("work.txt"), "keep me").unwrap();
        let refused = generate(&precious).unwrap_err();
        assert!(
            refused
                .to_string()
                .contains("wasn't made by abc's fixtures"),
            "{refused}"
        );
        assert!(precious.join("work.txt").exists(), "untouched");

        let empty = dir.path().join("empty");
        std::fs::create_dir_all(&empty).unwrap();
        assert!(generate(&empty).is_ok(), "an empty folder is fine");
        assert!(generate(&empty).is_ok(), "and fixtures' own, again");
    }

    #[test]
    fn fixtures_are_made_afresh_each_time() {
        let dir = tempfile::tempdir().unwrap();
        let made = generate(dir.path()).unwrap();
        let names: Vec<&str> = made.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["shop", "scraper", "notes", "old-tool"]);
        assert!(made.iter().all(|f| f.path.exists()));
        let rows = std::fs::read_to_string(made[0].path.join("data/prices.csv")).unwrap();
        assert_eq!(rows.lines().count(), 10_000);
        let again = generate(dir.path()).unwrap();
        assert_eq!(again, made, "the same again");
    }
}
