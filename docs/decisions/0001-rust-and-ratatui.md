# 0001: Rust and Ratatui

- Status: accepted
- Date: 2026-10-27

## Context

abc is a long-lived background server plus a terminal client that must stay
responsive. It runs on macOS first and Linux later, and the file viewer must
scroll 10,000-line files smoothly. Agents will write much of the code, so strong
compile-time checks and screens that can be tested as text matter.

## Decision

- Rust, on the toolchain pinned in `rust-toolchain.toml`, edition 2024.
- Ratatui on the crossterm backend for the client; tokio for async I/O in both
  binaries.
- Screens are immediate-mode: each frame is a pure function of the app state,
  and state changes only by handling messages.
- Every screen is snapshot-tested with Ratatui's `TestBackend` at 120x40 using
  insta, so a snapshot reads like its wireframe.

Alternatives considered: Go with Bubble Tea (weaker types), TypeScript with Ink
(Node runtime, GC pauses on large files), Python with Textual (startup time and
throughput).

## Consequences

- Single native binaries with fast startup and no runtime to install.
- Screens are testable as plain text.
- Compile times are the main cost. We keep dependencies lean and hold a warm
  `just check` under two minutes.
- Ratatui is a library, not a framework: we own the app architecture, focus
  handling and widgets.
- Building needs a C compiler for the tree-sitter grammars (see 0003).
