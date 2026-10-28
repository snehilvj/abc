# Changelog

What changed in each release of abc. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org). A release's section here becomes
its notes on GitHub.

## [Unreleased]

## [0.1.0] - unreleased

The first release: a terminal workspace across many repositories and their
worktrees, with a background server that owns the state.

### Added

- **Sidebar** of every repository and worktree: folder, branch or detached
  commit, uncommitted changes, commits ahead and behind, locked and missing
  worktrees, missing repositories, and the branch's pull request with its
  checks and review. A detailed (two lines per worktree) and a compact style.
- **Explorer** of a worktree's files as git sees them: dotfiles in, `.git` and
  what `.gitignore` leaves out, out. It follows files added and removed on
  disk, keeping its open folders and selection, and the viewer follows the
  open file as it changes, keeping its place.
- **Mouse wheel** scrolls the pane under the pointer; abc keeps the terminal
  from scrolling its history behind it.
- **Viewer**: read-only and Vim-style. Rust, TypeScript, TSX, JavaScript and
  Python are highlighted with tree-sitter, and some 220 other file types
  (Markdown and MDX, TOML, YAML, JSON, HTML, Jinja, shell, Dockerfiles, SQL,
  ...) with bat's syntax definitions, built in; a shebang names a script's
  language, and HTML with Jinja's tags is highlighted as Jinja. Motions and
  counts, visual mode, `/` and `* #` to find, `:` to go to a line, `y` to copy
  through the system's clipboard tool (or OSC 52 over ssh), and `g y` to copy
  an `@path:12-20` reference for Claude Code. Binary files and files over the
  size limit say so instead.
- **Switcher** (`ctrl-p`, `space f`, `space w`): fuzzy-find a worktree or a
  file in the current one.
- **Find in files** (`space /`): smart-case text across a worktree, results
  grouped by file as they arrive, opening at the line found.
- **Add a repository** (`space r a`, `abc repo add`) from a folder or a git
  URL, which is cloned first; checked as you type, with a name of its own.
- **Remove a repository** (`space r d`, `abc repo remove`); its files stay.
- **Help** (`?`) listing every key for the pane you're in, and a **which-key
  popup** after `space`, both read from the keymap so they show your keys.
- **abcd**, the server: one per profile, started by abc when needed and left
  running when abc quits. It rescans worktrees while abc is open, asks `gh`
  for pull request status, and serves every abc and CLI command over a Unix
  socket. A stale abcd is restarted when abc is rebuilt.
- **Commands**: `abc repo list|add|remove`, `abc server status|restart|stop`,
  `abc logs [--client] [--follow]`, `abc config path|check|template`.
- **Profiles** (`--profile`, `ABC_PROFILE`) with separate settings, state and
  logs.
- **Settings** in `config.toml`, every option documented by
  `abc config template`, applied within a second of a change: theme, layout,
  sidebar, keys (`[keys.global]` and a table per pane, `"none"` to unbind),
  icons (`[icons.glyphs]`, one by one), the mouse, rows and delays, where
  URLs are cloned, the viewer's limits, git and GitHub intervals, and the log
  filter.
- **Your own themes** in `themes/<name>.toml`: a built-in theme to start from
  and the colours to change, by role. `abc config theme <name>` prints one to
  copy.
- **Releases** for macOS (arm64, x86_64) and Linux (x86_64, arm64).

[Unreleased]: https://github.com/snehilvj/abc/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/snehilvj/abc/releases/tag/v0.1.0
