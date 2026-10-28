//! Every symbol abc draws: Nerd Font glyphs, one terminal cell each. abc needs
//! a Nerd Font in the terminal: a patched font such as FiraCode Nerd Font Mono,
//! or Symbols Nerd Font Mono as a fallback font. `ui::label` draws them.

pub const REPO: &str = "\u{f401}";
/// A repository's main checkout.
pub const HOME: &str = "\u{f46d}";
pub const BRANCH: &str = "\u{f418}";
/// A detached HEAD.
pub const COMMIT: &str = "\u{f417}";
/// The current worktree.
pub const CURRENT: &str = "\u{f44a}";

pub const PR_OPEN: &str = "\u{f407}";
pub const PR_DRAFT: &str = "\u{f4dd}";
pub const PR_MERGED: &str = "\u{f419}";
pub const PR_CLOSED: &str = "\u{f4dc}";
pub const CHECKS_PASSING: &str = "\u{f42e}";
pub const CHECKS_FAILING: &str = "\u{f467}";
pub const CHECKS_PENDING: &str = "\u{f43a}";

/// Uncommitted changes.
pub const CHANGED: &str = "\u{f444}";
pub const AHEAD: &str = "\u{f431}";
pub const BEHIND: &str = "\u{f433}";
pub const LOCK: &str = "\u{f023}";

/// Messages: something worked, failed, needs attention, or is just so.
pub const SUCCESS: &str = "\u{f4a4}";
pub const FAILURE: &str = "\u{f530}";
pub const WARNING: &str = "\u{f421}";
pub const INFO: &str = "\u{f449}";
/// Starting, connecting or restarting.
pub const SYNC: &str = "\u{f46a}";
pub const DISCONNECTED: &str = "\u{f529}";
