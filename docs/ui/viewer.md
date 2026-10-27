# File viewer

The right pane of the Code tab: a read-only view of one file, highlighted with
tree-sitter for Rust, TypeScript, TSX and Python.

`abc gallery viewer [--state <state>]`, default `highlighted`.

## Layout

```text
┬─ packages/integrations/gst/verify.ts · ts ─────────────┤
│  1  import { normalise } from "../../shared/text"      │
│  2  import type { GstClient } from "./client"          │
│  3                                                     │
│  4  export type GstInput = {                           │
│  5    gstin: string                                    │
│  6    legalName: string                                │
│  7    state?: string                                   │
│  8  }                                                  │
│  9                                                     │
│ 10  export type Result =                               │
```

…and the bottom of the pane, with the visible line range on the separator:

```text
│ 32    return {                                         │
│ 33      kind: "mismatch",                              │
│ 34      reason: `${a.gstin}: ${reason}`,               │
┴─────────────────────────────────────────── 1–34 of 58 ─┤
p find · tab last worktree · ? help · q quit             │
```

- **Header**: the file's path in the worktree, then its language (`rust`, `ts`,
  `tsx`, `python`, or `plain`).
- **Gutter**: line numbers, right-aligned to the width of the last line's
  number (at least 2), then two spaces.
- **Text**: no wrapping. Lines wider than the pane end in `…`. Tabs show as 4 spaces.
- **Line range** on the bottom separator: `1–34 of 58`.
- **Colours** (ANSI): keywords magenta, strings green, comments dim, types
  yellow, functions blue, numbers and constants cyan, properties and the rest
  in the default colour.
- Only the visible lines are drawn. Highlighting runs once when the file loads,
  off the UI thread; the text shows uncoloured until it finishes. A
  10,000-line file scrolls without delay.

## Keys

| Key                     | Action                           |
|-------------------------|----------------------------------|
| `↑` `↓` / `j` `k`       | Scroll one line                  |
| `ctrl-d` `ctrl-u`       | Half a page down / up            |
| `pgdn` `pgup` / `space` | A page down / up                 |
| `g` `G`                 | Top / end                        |
| `esc` / `←` / `h`       | Move focus to the explorer       |

Plus the [global keys](README.md#keys-everywhere). Key bar:

```text
│ ↑↓ scroll · ^d ^u half page · g G top/end · esc explorer · ^p find · tab last worktree · ? help · q quit             │
```

## States

### `highlighted`

Above.

### `none`: no file open

```text
┬────────────────────────────────────────────────────────┤
│ No file open.                                          │
│                                                        │
│ Pick one in the explorer, or press ctrl-p              │
│ then tab to search files.                              │
│                                                        │
│                                                        │
│                                                        │
│                                                        │
│                                                        │
│                                                        │
```

### `loading`

```text
┬─ packages/integrations/gst/verify.ts · ts ─────────────┤
│ Loading verify.ts…                                     │
│                                                        │
│                                                        │
│                                                        │
│                                                        │
│                                                        │
│                                                        │
│                                                        │
│                                                        │
│                                                        │
```

### `plain`: a language without highlighting

```text
┬─ README.md · plain ────────────────────────────────────┤
│  1  # privue-platform                                  │
│  2                                                     │
│  3  Monorepo for the platform services.                │
│  4                                                     │
│  5  ## Layout                                          │
│  6                                                     │
│  7  - `packages/`: shared libraries                    │
│  8  - `apps/`: deployable apps                         │
│                                                        │
│                                                        │
```

### `binary`

```text
┬─ assets/logo.png ──────────────────────────────────────┤
│ Binary file · 24.1 KB · not shown                      │
│                                                        │
│                                                        │
│                                                        │
│                                                        │
│                                                        │
│                                                        │
│                                                        │
│                                                        │
│                                                        │
```

### `too-large`: over 5 MB

```text
┬─ data/dump.sql ────────────────────────────────────────┤
│ Too large to show · 12.4 MB (limit 5 MB)               │
│                                                        │
│                                                        │
│                                                        │
│                                                        │
│                                                        │
│                                                        │
│                                                        │
│                                                        │
│                                                        │
```

### `error`

```text
┬─ packages/integrations/gst/verify.ts · ts ─────────────┤
│ ✗ Can't read verify.ts                                 │
│                                                        │
│ permission denied                                      │
│                                                        │
│                                                        │
│                                                        │
│                                                        │
│                                                        │
│                                                        │
│                                                        │
```

### `large`: a 10,000-line file, scrolled to the end

The gutter widens to 5 digits.

```text
│  9994      pub const ROW_9994: u32 = 9994;             │
│  9995      pub const ROW_9995: u32 = 9995;             │
│  9996      pub const ROW_9996: u32 = 9996;             │
│  9997      pub const ROW_9997: u32 = 9997;             │
│  9998      pub const ROW_9998: u32 = 9998;             │
│  9999      pub const ROW_9999: u32 = 9999;             │
│ 10000      pub const ROW_10000: u32 = 10000;           │
┴─────────────────────────────── 9,967–10,000 of 10,000 ─┤
```
