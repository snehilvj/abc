# 0004: Settings, keys, themes and icons

- Status: accepted
- Date: 2026-10-28
- Amends: 0002 (the client reads its own settings file)

## Context

Everything in abc must be configurable through settings files, using an
existing settings system rather than a home-grown one. abc must look right on
light and dark terminals, and show icons.

## Decision

**Settings.** [confique](https://crates.io/crates/confique) with TOML. Each
concern (`ui`, `keys`, `sidebar`, `explorer`, `viewer`, `server`, `git`,
`github`, ...) is a struct whose defaults and documentation live on its fields.
Layers, later ones winning: built-in defaults, then
`$XDG_CONFIG_HOME/abc/<profile>/config.toml`, then `ABC_*` environment variables
where a field declares one. Unknown keys are errors.

- `abc config template` prints every option with its docs and default,
  generated from the code, so the documentation can't drift.
- `abc config check` validates the file. `abc config path` prints where it is.
- Both abc and abcd load the same file with the same types and use their own
  parts. Each notices a changed file within a second; an invalid file keeps the
  last good settings and reports the error (status line, log).

**Keys.** `[keys]` maps actions to key combinations such as `"ctrl-p"`, parsed
and printed by [crokey](https://crates.io/crates/crokey). The key bar and the
help overlay render from the same map. Conflicting bindings are a load error.

**Themes.** `theme = "auto" | "dark" | "light" | "ansi" | "<name>"`, default
`auto`, which asks the terminal for its background colour with
[terminal-colorsaurus](https://crates.io/crates/terminal-colorsaurus) and picks
`dark` or `light` (falling back to `dark` if the terminal doesn't answer within
100 ms). `dark` and `light` are built in and use exact colours; `ansi` uses the
16 terminal colours so it follows the terminal's palette. Your own themes live
in `themes/<name>.toml` beside `config.toml` and may `inherit` a built-in one.
A theme names colours for roles: the interface (text, dim, border, accent,
selection, ok, warning, error), git (changed, ahead, behind), pull requests
(open, draft, merged, closed), checks (passing, failing, pending), and syntax
(tree-sitter highlight names). Colours are anything ratatui's `Color` parses:
names, `#rrggbb`, or indexes.

**Icons.** `icons = "nerd" | "unicode"`, default `nerd`. File-type icons and
their colours come from [devicons](https://crates.io/crates/devicons). Git, pull
request and check glyphs come from a built-in table that `[icons]` can override
one by one. `unicode` uses plain symbols that render in any font.

## Consequences

- Every option is discoverable from `abc config template`, and themes are plain
  TOML files you can copy and edit.
- The client reads one file directly, in addition to `abc logs`. It still never
  touches repositories or runs git; abcd does.
- Nerd Font glyphs need a Nerd Font, or a terminal that bundles the symbols;
  `icons = "unicode"` is the escape hatch.
- The settings structs grow with every feature. That is the price of
  "everything configurable", and the template keeps it visible.
