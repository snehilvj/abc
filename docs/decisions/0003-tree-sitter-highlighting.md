# 0003: tree-sitter for highlighting

- Status: accepted, amended by 0006 (bat's syntaxes for every other file type)
- Date: 2026-10-27

## Context

The read-only file viewer highlights Rust, TypeScript, TSX and Python, and must
stay smooth on 10,000-line files.

## Decision

- `tree-sitter` and `tree-sitter-highlight`, with the highlight queries bundled
  in each grammar crate.
- TypeScript and TSX layer the JavaScript queries from `tree-sitter-javascript`
  under their own, as upstream's `tree-sitter.json` does. The TypeScript crate's
  query alone covers only TypeScript-specific syntax.
- Highlighting runs in the client, because it is presentation; the server only
  supplies file contents. It runs once per file load, off the UI thread, and
  produces per-line spans over a small fixed set of kinds (keyword, string,
  comment, type, function, ...). The theme maps kinds to ANSI colours.
- The viewer renders only the visible lines. Text shows plain until the spans
  arrive. Unknown languages, binary files and very large files render plain or
  as a notice.

Alternatives considered: syntect (regex TextMate grammars; slower on big files,
weaker TSX support, not on the allowed list) and no highlighting.

## Consequences

- Parse-based, accurate highlighting; the same parsers can serve later features.
- Five C grammars compile at build time, which costs cold build time.
- The tree-sitter ABI must match across `tree-sitter-highlight` and the grammar
  crates, so their versions move together.
