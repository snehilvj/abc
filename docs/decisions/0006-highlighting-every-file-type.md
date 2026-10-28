# 0006: Highlighting every file type

- Status: accepted
- Date: 2026-10-28
- Amends: 0003 (tree-sitter is no longer the only highlighter)

## Context

Repositories hold far more than the four languages with tree-sitter grammars
in abc: Markdown, TOML, YAML, JSON, HTML, Jinja templates, shell scripts,
Dockerfiles, SQL. All of them showed plain. Ways considered:

- download prebuilt WASM grammars on demand and run them in wasmtime;
- download grammar sources and compile them with the machine's C compiler,
  loading the result as a library;
- compile many more tree-sitter grammars in;
- bundle a broad set of TextMate-style syntaxes.

## Decision

Keep tree-sitter for the languages abc has grammars for (Rust, TypeScript,
TSX, JavaScript, Python), and highlight every other file with bat's syntax
definitions, bundled by [two-face](https://crates.io/crates/two-face) and run
by [syntect](https://crates.io/crates/syntect): about 220 syntaxes, Jinja
included, built into the binary.

- A file's language comes from its name: compound extensions such as
  `.html.j2` first, then its extension, then its whole name (`Makefile`,
  `Dockerfile`). Without one, a shebang names it, and an HTML file with
  Jinja's `{% %}` or `{# #}` is highlighted as Jinja.
- syntect's scopes map onto the same kinds tree-sitter's captures do, so the
  theme colours both alike.
- A file with a line over 5,000 bytes (minified, generated) shows plain:
  syntect's regexes are slow on such lines.
- syntect is built for parsing only, with oniguruma regexes as bat uses.

Downloading grammars was turned down: it needs a network fetch on first use,
a list of grammar sources to keep up, and either wasmtime (much slower builds,
a larger binary) or a C compiler on every machine plus unsafe code to load
libraries. Compiling in more tree-sitter grammars leaves Jinja and rarer types
plain.

## Consequences

- Every common file type is highlighted offline, from the first file.
- Regex-based highlighting is a little less precise than tree-sitter's, which
  stays for the languages with grammars.
- The binary grows by the syntax set (a few megabytes), and oniguruma adds a
  C library to the build.
- syntect loads its syntaxes with bincode 1, which is unmaintained; deny.toml
  accepts the advisory, since only abc's own built-in data is read with it.
