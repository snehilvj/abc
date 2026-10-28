# abc

Personal, local-first terminal workspace for many repos and git worktrees.
Decisions: `docs/decisions/`. Screen designs: https://claude.ai/artifact/6oZWb4B7QKA4PiGid8GNJ6

## Commands

- `just check`: fmt, clippy `-D warnings`, all tests. Run before every commit.
- `just gallery <screen> [--state s] [--theme dark|light] [--icons unicode]`: a screen on fake data.

## Crates

- `crates/abc-core`: model types, protocol, profile paths, client connection.
- `crates/abcd`: the server. Owns all state; the only code that touches files or git.
- `crates/abc`: Ratatui client and CLI. Acts only through abcd.

## UI first

Per screen, in order: wireframe on the screen-designs page, every state at 120x40 with keys
(**stop for approval**) → build on fake data in `abc gallery <screen>` (**stop for
review**) → insta snapshot (`TestBackend` 120x40) that reads like the wireframe → VHS tape
in `docs/ui/tapes/<screen>.tape` → only then wire to real data.

## Hard rules

- One small step per commit; `just check` passes after each.
- The client never reads repos or runs git/gh; it asks abcd. It reads only its settings and `abc logs`.
- Everything is configurable: new options go in the confique settings structs with docs and defaults.
- Never change the `hello` message's shape. Bump `PROTOCOL` on any other wire change.
- Crates: ratatui, crossterm, tokio, serde, serde_json, toml, anyhow, thiserror, clap,
  tracing(-subscriber, -appender), tree-sitter(-highlight, -rust, -typescript, -javascript,
  -python), ignore, nucleo, gix, directories, tui-tree-widget, tui-input, crokey, confique,
  terminal-colorsaurus, devicons; insta, tempfile in tests. Ask before others.
- When something breaks, write the failing test first.
- Never edit or delete an existing test to make it pass. Stop and ask.
- Out of scope; don't build, stub or prepare: Claude sessions, tmux, services, diffs,
  PR review (PR status via `gh` is in), SQLite, editing, language servers, SCIP, symbol
  search, remote access.
- If something is ambiguous, ask instead of guessing.
