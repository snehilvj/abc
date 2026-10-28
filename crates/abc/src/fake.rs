//! The fake world the gallery and snapshot tests draw. It mirrors the approved
//! screen designs, so a gallery screen can be compared with its design directly.

use std::path::{Path, PathBuf};

use abc_core::model::{
    Checks, GitStatus, Head, PrState, PullRequest, Repo, RepoState, Review, Worktree,
};

fn worktree(repo: &str, name: &str, branch: &str) -> Worktree {
    let is_main = name == repo;
    let path = if is_main {
        format!("~/code/{repo}")
    } else {
        format!("~/code/wt/{repo}/{name}")
    };
    Worktree {
        path: PathBuf::from(path),
        name: name.into(),
        head: Head::Branch(branch.into()),
        is_main,
        locked: false,
        missing: false,
        git: Some(GitStatus {
            upstream: true,
            ..GitStatus::default()
        }),
        pr: None,
        last_opened: None,
    }
}

fn pr(
    number: u32,
    state: PrState,
    checks: Option<Checks>,
    review: Option<Review>,
    title: &str,
) -> Option<PullRequest> {
    Some(PullRequest {
        number,
        title: title.into(),
        url: format!("https://github.com/privue/repo/pull/{number}"),
        state,
        checks,
        review,
    })
}

fn git(changed: u32, ahead: u32, behind: u32) -> Option<GitStatus> {
    Some(GitStatus {
        changed,
        ahead,
        behind,
        upstream: true,
    })
}

fn repo(name: &str, worktrees: Vec<Worktree>) -> Repo {
    Repo {
        name: name.into(),
        path: PathBuf::from(format!("~/code/{name}")),
        state: RepoState::Present { worktrees },
    }
}

/// Two repos, seven worktrees: the populated frame.
pub fn repos() -> Vec<Repo> {
    vec![
        repo(
            "privue-platform",
            vec![
                Worktree {
                    git: git(3, 2, 0),
                    pr: pr(
                        142,
                        PrState::Open,
                        Some(Checks::Passing),
                        Some(Review::Approved),
                        "GST mismatch check",
                    ),
                    last_opened: Some(NOW),
                    ..worktree("privue-platform", "gst-check", "feat/gst-check")
                },
                Worktree {
                    git: git(0, 0, 1),
                    pr: pr(
                        139,
                        PrState::Open,
                        Some(Checks::Failing),
                        Some(Review::ChangesRequested),
                        "Round GST to paise",
                    ),
                    last_opened: Some(NOW - 2 * HOUR),
                    ..worktree("privue-platform", "gst-rounding", "fix/gst-rounding")
                },
                Worktree {
                    git: git(1, 0, 0),
                    pr: pr(141, PrState::Merged, None, None, "Ledger rounding"),
                    ..worktree("privue-platform", "ledger-round", "fix/ledger-round")
                },
                Worktree {
                    last_opened: Some(NOW - 3 * DAY),
                    ..worktree("privue-platform", "privue-platform", "main")
                },
            ],
        ),
        repo(
            "ecourts",
            vec![
                Worktree {
                    git: git(0, 4, 0),
                    pr: pr(
                        88,
                        PrState::Draft,
                        Some(Checks::Pending),
                        Some(Review::Required),
                        "GST lookup",
                    ),
                    last_opened: Some(NOW - DAY),
                    ..worktree("ecourts", "gst-lookup", "feat/gst-lookup")
                },
                Worktree {
                    git: Some(GitStatus {
                        changed: 7,
                        ahead: 0,
                        behind: 0,
                        upstream: false,
                    }),
                    last_opened: Some(NOW - 5 * DAY),
                    ..worktree("ecourts", "ocr-retry", "feat/ocr-retry")
                },
                Worktree {
                    git: git(0, 0, 3),
                    last_opened: Some(NOW - 14 * DAY),
                    ..worktree("ecourts", "ecourts", "main")
                },
            ],
        ),
    ]
}

/// Every row variant: locked, merged, closed, detached, missing worktree, missing repo.
pub fn repos_with_variants() -> Vec<Repo> {
    vec![
        repo(
            "privue-platform",
            vec![
                Worktree {
                    git: git(3, 2, 0),
                    pr: pr(
                        142,
                        PrState::Open,
                        Some(Checks::Passing),
                        Some(Review::Approved),
                        "GST mismatch check",
                    ),
                    ..worktree("privue-platform", "gst-check", "feat/gst-check")
                },
                Worktree {
                    git: git(0, 0, 1),
                    locked: true,
                    pr: pr(
                        139,
                        PrState::Open,
                        Some(Checks::Failing),
                        Some(Review::ChangesRequested),
                        "Round GST to paise",
                    ),
                    ..worktree("privue-platform", "gst-rounding", "fix/gst-rounding")
                },
                Worktree {
                    pr: pr(141, PrState::Merged, None, None, "Ledger rounding"),
                    ..worktree("privue-platform", "ledger-round", "fix/ledger-round")
                },
                Worktree {
                    pr: pr(97, PrState::Closed, None, None, "OCR spike"),
                    ..worktree("privue-platform", "old-spike", "spike/ocr")
                },
                Worktree {
                    head: Head::Detached("a1b2c3d".into()),
                    ..worktree("privue-platform", "bisect", "")
                },
                Worktree {
                    missing: true,
                    git: None,
                    ..worktree("privue-platform", "old-idea", "feat/old-idea")
                },
            ],
        ),
        repo(
            "ecourts",
            vec![
                Worktree {
                    git: git(0, 4, 0),
                    pr: pr(
                        88,
                        PrState::Draft,
                        Some(Checks::Pending),
                        Some(Review::Required),
                        "GST lookup",
                    ),
                    ..worktree("ecourts", "gst-lookup", "feat/gst-lookup")
                },
                Worktree {
                    git: Some(GitStatus {
                        changed: 7,
                        ahead: 0,
                        behind: 0,
                        upstream: false,
                    }),
                    ..worktree("ecourts", "ocr-retry", "feat/ocr-retry")
                },
            ],
        ),
        Repo {
            name: "old-scraper".into(),
            path: PathBuf::from("~/code/old-scraper"),
            state: RepoState::Missing,
        },
    ]
}

/// The same repos with no pull request status, as when `gh` is unavailable.
pub fn repos_without_prs() -> Vec<Repo> {
    let mut repos = repos();
    for repo in &mut repos {
        if let RepoState::Present { worktrees } = &mut repo.state {
            for w in worktrees {
                w.pr = None;
            }
        }
    }
    repos
}

/// A fixed "now" so relative times never change between runs.
pub const NOW: u64 = 1_793_000_000;
const HOUR: u64 = 3600;
const DAY: u64 = 24 * HOUR;

/// The path of the worktree that starts out current.
pub fn current() -> PathBuf {
    PathBuf::from("~/code/wt/privue-platform/gst-check")
}

/// The files of every privue-platform worktree.
pub const PLATFORM_FILES: &[&str] = &[
    ".github/workflows/check.yml",
    ".gitignore",
    "apps/web/package.json",
    "apps/web/src/main.tsx",
    "apps/web/src/pages/home.tsx",
    "apps/web/src/pages/verify-gst.tsx",
    "data/dump.sql",
    "docs/gst.md",
    "docs/logo.png",
    "generated/rows.rs",
    "package.json",
    "packages/integrations/gst/client.ts",
    "packages/integrations/gst/verify.ts",
    "packages/scoring/score.py",
    "packages/scoring/verifier.py",
    "packages/shared/text.ts",
    "README.md",
    "tsconfig.json",
];

/// The files of every ecourts worktree.
pub const ECOURTS_FILES: &[&str] = &[
    ".gitignore",
    "Dockerfile",
    "README.md",
    "ecourts/__init__.py",
    "ecourts/lookup.py",
    "ecourts/ocr.py",
    "ecourts/retry.py",
    "pyproject.toml",
    "tests/test_lookup.py",
    "tests/test_ocr.py",
];

/// Loaded file lists for every worktree, with `verify.ts` open in the current one.
/// The files abcd would list for `worktree`, one of `repos`'.
pub fn files(repos: &[Repo], worktree: &Path) -> Vec<String> {
    let ecourts = repos
        .iter()
        .any(|r| r.name == "ecourts" && r.worktrees().iter().any(|w| w.path == worktree));
    let files = if ecourts {
        ECOURTS_FILES
    } else {
        PLATFORM_FILES
    };
    files.iter().map(|f| f.to_string()).collect()
}

pub fn listings(repos: &[Repo]) -> std::collections::HashMap<PathBuf, crate::explorer::Listing> {
    use crate::explorer::{Explorer, Listing};
    let mut listings = std::collections::HashMap::new();
    for repo in repos {
        let files = if repo.name == "ecourts" {
            ECOURTS_FILES
        } else {
            PLATFORM_FILES
        };
        for w in repo.worktrees() {
            let mut explorer = Explorer::new(files);
            if w.path == current() {
                let verify: Vec<String> = "packages/integrations/gst/verify.ts"
                    .split('/')
                    .map(String::from)
                    .collect();
                explorer.reveal(&verify);
            }
            listings.insert(w.path.clone(), Listing::Ready(Box::new(explorer)));
        }
    }
    listings
}

const VERIFY_TS: &str = r#"import { normalise } from "../../shared/text"
import type { GstClient } from "./client"

export type GstInput = {
  gstin: string
  legalName: string
  state?: string
}

export type Result =
  | { kind: "ok"; gstin: string }
  | { kind: "mismatch"; reason: string }

// Checks a GSTIN against the registry and the
// details the customer gave us.
export async function verify(
  input: GstInput,
  client: GstClient,
): Promise<Result> {
  const a = normalise(input)
  const record = await client.lookup(a.gstin)
  if (!record) {
    return mismatch(a, "not registered")
  }
  if (record.legalName !== a.legalName) {
    return mismatch(a, "name differs")
  }
  if (a.state && record.state !== a.state) {
    return mismatch(a, `state is ${record.state}`)
  }
  return { kind: "ok", gstin: a.gstin }
}

function mismatch(a: GstInput, reason: string): Result {
  return {
    kind: "mismatch",
    reason: `${a.gstin}: ${reason}`,
  }
}

const GSTIN = /^[0-9]{2}[A-Z]{5}[0-9]{4}[A-Z][1-9A-Z]Z[0-9A-Z]$/

export function isGstin(value: string): boolean {
  return GSTIN.test(value.trim().toUpperCase())
}

export const RETRIES = 3
export const TIMEOUT_MS = 5_000

export function describe(result: Result): string {
  switch (result.kind) {
    case "ok":
      return `verified ${result.gstin}`
    case "mismatch":
      return result.reason
  }
}
"#;

const CLIENT_TS: &str = r#"export type Record = { gstin: string; legalName: string; state: string }

export interface GstClient {
  lookup(gstin: string): Promise<Record | undefined>
}

export class HttpGstClient implements GstClient {
  constructor(private readonly base: string) {}

  async lookup(gstin: string): Promise<Record | undefined> {
    const response = await fetch(`${this.base}/gstin/${gstin}`)
    if (response.status === 404) return undefined
    return (await response.json()) as Record
  }
}
"#;

const TEXT_TS: &str = r#"export function normalise<T extends { gstin: string; legalName: string }>(input: T): T {
  return { ...input, gstin: input.gstin.trim().toUpperCase(), legalName: input.legalName.trim() }
}
"#;

const MAIN_TSX: &str = r#"import { createRoot } from "react-dom/client"
import { Home } from "./pages/home"

createRoot(document.getElementById("root")!).render(<Home title="GST checks" />)
"#;

const VERIFY_GST_TSX: &str = r#"import { useState } from "react"
import { verify } from "@privue/integrations/gst/verify"

export function VerifyGst({ client }: { client: GstClient }) {
  const [gstin, setGstin] = useState("")
  return (
    <form onSubmit={() => verify({ gstin, legalName: "" }, client)}>
      <input value={gstin} onChange={(e) => setGstin(e.target.value)} />
      <button type="submit">Verify</button>
    </form>
  )
}
"#;

const SCORE_PY: &str = r#""""Scores a counterparty from its verification results."""

from dataclasses import dataclass

WEIGHTS = {"gst": 0.4, "bank": 0.35, "litigation": 0.25}


@dataclass
class Signal:
    name: str
    passed: bool


def score(signals: list[Signal]) -> float:
    # Each passed signal adds its weight; unknown signals count for nothing.
    total = 0.0
    for signal in signals:
        if signal.passed:
            total += WEIGHTS.get(signal.name, 0)
    return round(total * 100, 1)
"#;

const README_MD: &str = "# privue-platform\n\nMonorepo for the platform services.\n\n## Layout\n\n- `packages/`: shared libraries\n- `apps/`: deployable apps\n";

/// A file's contents, as abcd would send them.
pub fn read(path: &str) -> Result<abc_core::model::FileContents, String> {
    use abc_core::model::FileContents;
    let text = |t: &str| {
        Ok(FileContents::Text {
            text: t.to_string(),
        })
    };
    match path {
        "packages/integrations/gst/verify.ts" => text(VERIFY_TS),
        "packages/integrations/gst/client.ts" => text(CLIENT_TS),
        "packages/shared/text.ts" => text(TEXT_TS),
        "apps/web/src/main.tsx" => text(MAIN_TSX),
        "apps/web/src/pages/verify-gst.tsx" | "apps/web/src/pages/home.tsx" => text(VERIFY_GST_TSX),
        "packages/scoring/score.py" | "packages/scoring/verifier.py" => text(SCORE_PY),
        "README.md" | "docs/gst.md" => text(README_MD),
        "generated/rows.rs" => Ok(FileContents::Text { text: rows() }),
        "docs/logo.png" => Ok(FileContents::Binary { size: 24_123 }),
        "data/dump.sql" => Ok(FileContents::TooLarge {
            size: 12_400_000,
            limit: 5_000_000,
        }),
        _ if path.ends_with(".py") => text(SCORE_PY),
        _ => text("# fake file\n"),
    }
}

/// A generated 10,000-line Rust file, for scrolling.
pub fn rows() -> String {
    let mut out = String::from("// Generated: one constant per row.\n");
    for n in 2..=10_000 {
        out.push_str(&format!("pub const ROW_{n}: u32 = {n};\n"));
    }
    out.pop();
    out
}

/// The current worktree's viewer, with `path` open and highlighted.
pub fn viewer(path: &str) -> crate::viewer::Viewer {
    use crate::viewer::{Document, Viewer};
    let name = path.rsplit('/').next().unwrap_or(path);
    let document = match read(path) {
        Ok(contents) => match Document::from_contents(contents, name) {
            Document::Text(text) => Document::Text(text.highlighted()),
            other => other,
        },
        Err(why) => Document::Failed(why),
    };
    Viewer::new(path.split('/').map(String::from).collect(), document)
}

/// What abcd's find in files would send for `query` over `files`: text files
/// only, no more than `max_lines` lines, and whether more matched.
pub fn search(
    files: &[String],
    query: &str,
    max_lines: usize,
) -> (Vec<abc_core::model::FoundFile>, bool) {
    use abc_core::model::{FileContents, FoundFile, FoundLine};
    let search = crate::search::Search::new(query, true);
    let (mut found, mut lines) = (Vec::new(), 0);
    for path in files {
        let Ok(FileContents::Text { text }) = read(path) else {
            continue;
        };
        let mut file = FoundFile {
            path: path.clone(),
            lines: Vec::new(),
        };
        for (i, line) in text.split('\n').enumerate() {
            if search.matches(line).is_empty() {
                continue;
            }
            if lines == max_lines {
                if !file.lines.is_empty() {
                    found.push(file);
                }
                return (found, true);
            }
            lines += 1;
            file.lines.push(FoundLine {
                number: i as u32 + 1,
                text: line.to_string(),
            });
        }
        if !file.lines.is_empty() {
            found.push(file);
        }
    }
    (found, false)
}

/// Folders on the fake disk, for adding a repository.
pub const FOLDERS: &[&str] = &[
    "~/code/",
    "~/code/ecourts/",
    "~/code/notes/",
    "~/code/privue-agents/",
    "~/code/privue-agreements/",
    "~/code/privue-platform/",
    "~/Desktop/",
    "~/Documents/",
    "~/Downloads/",
    "~/work/",
    "~/work/ecourts/",
];

/// Repositories the fake remote host has, by `owner/name`; the first two are added already.
const REMOTES: &[&str] = &[
    "privue/privue-platform",
    "privue/ecourts",
    "privue/privue-agents",
    "privue/notes",
    "privue/privue-docs",
];

/// Where abcd clones to (`add_repo.clone_dir`).
pub const CLONE_DIR: &str = "~/code";

/// What abcd would make of `path`, a folder or git URL, as a repository
/// named `name`, with `repos` added already.
pub fn check_repo(path: &str, name: &str, repos: &[Repo]) -> abc_core::model::RepoCheck {
    use abc_core::model::RepoCheck;
    if crate::add_repo::is_url(path) {
        let remote = path
            .trim_end_matches('/')
            .trim_end_matches(".git")
            .rsplit(['/', ':'])
            .take(2)
            .collect::<Vec<_>>();
        let remote = format!("{}/{}", remote.get(1).unwrap_or(&""), remote[0]);
        return match REMOTES.iter().position(|r| *r == remote) {
            None => RepoCheck::NotFound,
            Some(i) if i < 2 => RepoCheck::AlreadyAdded {
                name: remote.rsplit('/').next().unwrap_or("").into(),
            },
            Some(_) => {
                let into = format!("{CLONE_DIR}/{name}");
                if FOLDERS.contains(&format!("{into}/").as_str()) {
                    RepoCheck::TargetExists { into }
                } else {
                    RepoCheck::Remote { into }
                }
            }
        };
    }
    let folder = path.trim_end_matches('/');
    if let Some(repo) = repos.iter().find(|r| r.path == Path::new(folder)) {
        return RepoCheck::AlreadyAdded {
            name: repo.name.clone(),
        };
    }
    match folder {
        "~/code/privue-agents" => RepoCheck::Ready { worktrees: 3 },
        "~/code/privue-agreements" | "~/work/ecourts" => RepoCheck::Ready { worktrees: 1 },
        _ if FOLDERS.contains(&format!("{folder}/").as_str()) => RepoCheck::NotGit,
        _ => RepoCheck::NotFound,
    }
}

/// The repository abcd would add from `path` (cloned into the clone folder
/// for a URL): its main worktree, on main.
pub fn added(path: &str, name: &str) -> Repo {
    let folder = if crate::add_repo::is_url(path) {
        format!("{CLONE_DIR}/{name}")
    } else {
        path.trim_end_matches('/').to_string()
    };
    Repo {
        name: name.into(),
        path: PathBuf::from(&folder),
        state: RepoState::Present {
            worktrees: vec![Worktree {
                path: PathBuf::from(&folder),
                name: folder.rsplit('/').next().unwrap_or(name).into(),
                is_main: true,
                ..worktree(name, name, "main")
            }],
        },
    }
}
