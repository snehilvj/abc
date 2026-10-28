//! The fake world the gallery and snapshot tests draw. It mirrors the approved
//! screen designs, so a gallery screen can be compared with its design directly.

use std::path::PathBuf;

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
