# abc

A terminal workspace for working across many git repositories and their
worktrees. A sidebar of every worktree, with its branch, uncommitted changes,
commits ahead and behind, and pull request; a file explorer; a read-only,
Vim-style file viewer with syntax highlighting; find in files; and a switcher
to jump anywhere. Local-first: nothing leaves your machine except what `gh`
asks GitHub for pull request status.

[getabc.dev](https://getabc.dev)

## Requirements

- macOS or Linux.
- git 2.31 or newer.
- A [Nerd Font](https://www.nerdfonts.com) in your terminal, such as
  FiraCode Nerd Font Mono, or Symbols Nerd Font Mono as a fallback. Without
  one, icons draw as boxes.
- Optional: the [GitHub CLI](https://cli.github.com), signed in with
  `gh auth login`, for pull request status. abc stores no token of its own.

## Install

From a [release](https://github.com/snehilvj/abc/releases): download the
archive for your system, check it against its `.sha256`, and put `abc` and
`abcd` in the same folder on your `PATH`. abc looks for abcd beside itself.
The binaries aren't signed: on macOS, one downloaded in a browser is
quarantined, so run `xattr -d com.apple.quarantine abc abcd` once, or
download with `curl`.

From source, with [Rust](https://rustup.rs) (the toolchain is pinned in
`rust-toolchain.toml`) and [just](https://just.systems):

```sh
git clone https://github.com/snehilvj/abc && cd abc
just install    # abc and abcd into ~/.cargo/bin
```

## Use

```sh
abc repo add ~/code/my-app                  # a repository on this machine
abc repo add https://github.com/owner/repo  # cloned into ~/code, then added
abc                                          # open abc
```

In abc, `space` starts a command and a popup lists what can follow:

| Keys | |
|---|---|
| `ctrl-p` | Switch to a worktree or file |
| `space f` · `space w` | Find a file · a worktree |
| `space /` | Find in files |
| `space r a` · `space r d` | Add · remove a repository |
| `tab` | Back to the last worktree |
| `h` `l` · `← →` | Move between the sidebar, explorer and viewer |
| `j` `k` · `enter` | Move · open |
| `?` | Every key for the pane you're in |
| `q` | Quit (abcd keeps running) |

The viewer moves like Vim: `h j k l`, words, `gg` `G` and counts, `{ }`, `%`,
`f t ; ,`, `/ n N * #` to find, `:42` to go to a line, `v` and `V` to select,
`y` to copy, and `g y` to copy an `@path:12-20` reference for Claude Code.

Other commands:

```sh
abc repo list | add | remove      # the repositories abc knows
abc server status | restart | stop
abc logs [--client] [--follow]    # abcd's log, or abc's
abc config path | check | template
```

## Settings

Settings live in `~/.config/abc/<profile>/config.toml`, and every key is
optional. `abc config template` prints every option with its docs and
default; `abc config check` validates the file, keys included. Changes apply
within a second, in abc and abcd both; a file with a mistake says where and
keeps the last good settings.

```toml
theme = "auto"          # or "dark", "light", "ansi", or one of yours

[sidebar]
style = "compact"       # one line per worktree

[add_repo]
clone_dir = "~/src"     # where git URLs are cloned

[keys.global]
"space x" = "quit"      # add or change keys; "none" unbinds one

[icons.glyphs]
branch = "\ue0a0"       # any icon, one by one

[mouse]
enabled = false         # leave the mouse to the terminal
```

Your own theme: `abc config theme dark > ~/.config/abc/<profile>/themes/mine.toml`
writes the dark theme as a file to change, then `theme = "mine"`. A theme
file names the theme it starts from (`inherit`) and the colours it changes,
by role: `[ui]`, `[git]`, `[pr]` and `[syntax]`.

abc scrolls by keys alone. It takes the mouse so the wheel scrolls nothing:
not the terminal's history, nor abc, which many terminals would send the wheel
to as the up and down keys. Most terminals then select text with a key held
(Shift; Option in iTerm2); `[mouse] enabled = false` gives the mouse back.

## How it works

abc is two programs. `abcd`, the server, owns all state and does all git and
file access; it runs in the background and serves every abc over a Unix
socket. `abc` starts it when it isn't running, and quitting abc leaves it
running, so several terminals see the same repositories. When you rebuild
abc, it restarts an abcd from an older build.

Profiles keep separate setups apart: `--profile <name>` or `ABC_PROFILE`,
`stable` by default. Each has its own settings in
`$XDG_CONFIG_HOME/abc/<profile>/` and its own state, logs and socket in
`$XDG_STATE_HOME/abc/<profile>/` (`~/.config` and `~/.local/state` unless
set, on macOS too). Profiles share nothing.

## Development

```sh
just check       # what CI runs: lock file, formatting, clippy, tests, docs
just deny        # dependency advisories, licences and sources (cargo-deny)
just fixtures    # fixture repositories in the dev profile
just dev         # this checkout's abc, in the dev profile
just gallery frame --state sidebar   # a screen on fake data, no server
just tape viewer                     # record a screen's GIF with VHS
```

`just check` needs [cargo-insta](https://insta.rs); the gallery and tapes
need a Nerd Font, and tapes need [VHS](https://github.com/charmbracelet/vhs).

The workspace has three crates: `abc-core` (the model, protocol, profiles,
settings and client connection), `abcd` (the server) and `abc` (the terminal
client and CLI). Screens are built UI-first: designed, then built in the
gallery on fake data, then snapshot-tested at 120x40 and recorded as a tape,
and only then wired to abcd. Decisions are recorded in `docs/decisions/`;
`AGENTS.md` has the rules for coding agents working here.

To release, set the version in `Cargo.toml`, give it a section in
`CHANGELOG.md`, then push a tag such as `v0.1.0`: CI builds abc and abcd for
macOS and Linux and publishes them, with that section as the release notes.

## Licence

MIT
