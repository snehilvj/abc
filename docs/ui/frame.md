# App frame

Always on screen: the title bar, the worktree sidebar, the tab bar (only **Code**
in slice 1), the Code tab with the [explorer](explorer.md) and
[viewer](viewer.md), the key bar and the status line.

`abc gallery frame [--state <state>]`, default `populated`.

## Populated

Focus is on the explorer; the cursor is on `verify.ts`, which is open in the viewer.

```text
┌ abc · privue-platform / feat/gst-check ───────────────────────────────────────────────────────────────────────── dev ┐
│ WORKTREES                  │ Code                                                                                    │
│ privue-platform            ├─ …t/privue-platform/gst-check ─┬─ packages/integrations/gst/verify.ts · ts ─────────────┤
│ ▸ feat/gst-check           │ packages/                      │  1  import { normalise } from "../../shared/text"      │
│   fix/gst-rounding         │ ├─ integrations/               │  2  import type { GstClient } from "./client"          │
│   fix/ledger-round         │ │  └─ gst/                     │  3                                                     │
│   main                     │ │     ├─ verify.ts  ◂          │  4  export type GstInput = {                           │
│ ecourts                    │ │     └─ client.ts             │  5    gstin: string                                    │
│   feat/gst-lookup          │ ├─ scoring/                    │  6    legalName: string                                │
│   feat/ocr-retry           │ └─ shared/                     │  7    state?: string                                   │
│   main                     │ apps/                          │  8  }                                                  │
│                            │ docs/                          │  9                                                     │
│                            │ .gitignore                     │ 10  export type Result =                               │
│                            │ package.json                   │ 11    | { kind: "ok"; gstin: string }                  │
│                            │ README.md                      │ 12    | { kind: "mismatch"; reason: string }           │
│                            │ tsconfig.json                  │ 13                                                     │
│                            │                                │ 14  // Checks a GSTIN against the registry and the     │
│                            │                                │ 15  // details the customer gave us.                   │
│                            │                                │ 16  export async function verify(                      │
│                            │                                │ 17    input: GstInput,                                 │
│                            │                                │ 18    client: GstClient,                               │
│                            │                                │ 19  ): Promise<Result> {                               │
│                            │                                │ 20    const a = normalise(input)                       │
│                            │                                │ 21    const record = await client.lookup(a.gstin)      │
│                            │                                │ 22    if (!record) {                                   │
│                            │                                │ 23      return mismatch(a, "not registered")           │
│                            │                                │ 24    }                                                │
│                            │                                │ 25    if (record.legalName !== a.legalName) {          │
│                            │                                │ 26      return mismatch(a, "name differs")             │
│                            │                                │ 27    }                                                │
│                            │                                │ 28    return { kind: "ok", gstin: a.gstin }            │
│                            │                                │ 29  }                                                  │
│                            │                                │ 30                                                     │
│                            │                                │ 31  function mismatch(a: GstInput, reason: string): R… │
│                            │                                │ 32    return {                                         │
│                            │                                │ 33      kind: "mismatch",                              │
│                            │                                │ 34      reason: `${a.gstin}: ${reason}`,               │
├────────────────────────────┴────────────────────────────────┴─────────────────────────────────────────── 1–34 of 58 ─┤
│ ↑↓ move · enter open · ^p find · tab last worktree · ←→ focus · a add repo · d remove repo · ? help · q quit         │
└─────────────────────────────────────────────────────────────────────────────────────────────── 2 repos · 7 worktrees ┘
```

- **Title** (top border): `abc · <repo> / <branch>` for the current worktree,
  or `(detached a1b2c3d)` instead of a branch. The profile is on the right:
  `dev` in yellow, `stable` dim.
- **Sidebar**: repos in the order they were added, each followed by its
  worktrees sorted by branch name. `▸` marks the current worktree. The cursor
  can rest on a repo row or a worktree row.
- **Tab bar**: `Code` only, in bold.
- **Pane headers**: the explorer shows the worktree's path, trimmed from the
  left with `…`; the viewer shows the file path and its language. The focused
  pane's header is bold cyan.
- **Key bar**: keys for the focused pane first, then the global ones. `?` lists
  everything.
- **Status line** (bottom border): messages on the left, counts on the right.
  The viewer's visible line range sits on the separator just above it.

## Keys

Global keys are in the [UI README](README.md#keys-everywhere). Explorer and
viewer keys are in their own files. Sidebar keys:

| Key             | Action                                                                  |
|-----------------|-------------------------------------------------------------------------|
| `↑` `↓` / `j` `k` | Move the cursor                                                       |
| `g` `G`         | First / last row                                                        |
| `enter`         | Open the worktree under the cursor and move focus to the explorer. On a repo row, open its main worktree (a bare repo: its first worktree). |

Key bar with the sidebar focused:

```text
│ ↑↓ move · enter open · a add repo · d remove repo · ^p find · tab last worktree · → explorer · ? help · q quit       │
```

Opening a worktree remembers it as the current one in abcd, which is what the
switcher's "2h ago" column and `tab` use. The explorer and viewer keep their
own state per worktree for the life of the client: coming back with `tab`
restores the open file, scroll position and expanded folders.

## States

### `populated`

Above.

### `empty`: no repositories (first run)

```text
┌ abc ──────────────────────────────────────────────────────────────────────────────────────────────────────────── dev ┐
│ WORKTREES                  │ Code                                                                                    │
│                            ├─────────────────────────────────────────────────────────────────────────────────────────┤
│ No repositories yet.       │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                   No repositories yet                                   │
│                            │                                                                                         │
│                            │                               Press a to add one, or run                                │
│                            │                                   abc repo add <path>                                   │
│                            │                                       in a shell.                                       │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
├────────────────────────────┴─────────────────────────────────────────────────────────────────────────────────────────┤
│ a add repo · ? help · q quit                                                                                         │
└───────────────────────────────────────────────────────────────────────────────────────────────────────────── 0 repos ┘
```

### `loading`: starting the server

Shown from launch until the first repo list arrives. If abcd is already running
this lasts a few milliseconds and the status reads `◌ connecting…` instead.

```text
┌ abc ──────────────────────────────────────────────────────────────────────────────────────────────────────────── dev ┐
│ WORKTREES                  │ Code                                                                                    │
│                            ├─────────────────────────────────────────────────────────────────────────────────────────┤
│ Loading…                   │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
├────────────────────────────┴─────────────────────────────────────────────────────────────────────────────────────────┤
│ ? help · q quit                                                                                                      │
└ ◌ starting abcd… ────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### `error`: the server did not start

abcd did not answer within 3 seconds of being started. The client never reads
log files itself, so it points at `abc logs`.

```text
┌ abc ──────────────────────────────────────────────────────────────────────────────────────────────────────────── dev ┐
│ WORKTREES                  │ Code                                                                                    │
│                            ├─────────────────────────────────────────────────────────────────────────────────────────┤
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                          ✗ abcd did not start within 3 seconds                          │
│                            │                                                                                         │
│                            │                          Run  abc logs  in a shell to see why.                          │
│                            │                                                                                         │
│                            │                                    r retry · q quit                                     │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
├────────────────────────────┴─────────────────────────────────────────────────────────────────────────────────────────┤
│ r retry · q quit                                                                                                     │
└ ✗ abcd did not start ────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### `sidebar`: worktree and repo variants

```text
┌ abc · privue-platform / feat/gst-check ───────────────────────────────────────────────────────────────────────── dev ┐
│ WORKTREES                  │ Code                                                                                    │
│ privue-platform            ├─ …t/privue-platform/gst-check ─┬─ packages/integrations/gst/verify.ts · ts ─────────────┤
│ ▸ feat/gst-check           │ packages/                      │  1  import { normalise } from "../../shared/text"      │
│   fix/gst-rounding  locked │ ├─ integrations/               │  2  import type { GstClient } from "./client"          │
│   fix/ledger-round         │ │  └─ gst/                     │  3                                                     │
│   main                     │ │     ├─ verify.ts  ◂          │  4  export type GstInput = {                           │
│   (detached a1b2c3d)       │ │     └─ client.ts             │  5    gstin: string                                    │
│   feat/old-idea    missing │ ├─ scoring/                    │  6    legalName: string                                │
│ ecourts                    │ └─ shared/                     │  7    state?: string                                   │
│   feat/gst-lookup          │ apps/                          │  8  }                                                  │
│   feat/ocr-retry           │ docs/                          │  9                                                     │
│ old-scraper        missing │ .gitignore                     │ 10  export type Result =                               │
│                            │ package.json                   │ 11    | { kind: "ok"; gstin: string }                  │
│                            │ README.md                      │ 12    | { kind: "mismatch"; reason: string }           │
│                            │ tsconfig.json                  │ 13                                                     │
│                            │                                │ 14  // Checks a GSTIN against the registry and the     │
│                            │                                │ 15  // details the customer gave us.                   │
│                            │                                │ 16  export async function verify(                      │
│                            │                                │ 17    input: GstInput,                                 │
│                            │                                │ 18    client: GstClient,                               │
│                            │                                │ 19  ): Promise<Result> {                               │
│                            │                                │ 20    const a = normalise(input)                       │
│                            │                                │ 21    const record = await client.lookup(a.gstin)      │
│                            │                                │ 22    if (!record) {                                   │
│                            │                                │ 23      return mismatch(a, "not registered")           │
│                            │                                │ 24    }                                                │
│                            │                                │ 25    if (record.legalName !== a.legalName) {          │
│                            │                                │ 26      return mismatch(a, "name differs")             │
│                            │                                │ 27    }                                                │
│                            │                                │ 28    return { kind: "ok", gstin: a.gstin }            │
│                            │                                │ 29  }                                                  │
│                            │                                │ 30                                                     │
│                            │                                │ 31  function mismatch(a: GstInput, reason: string): R… │
│                            │                                │ 32    return {                                         │
│                            │                                │ 33      kind: "mismatch",                              │
│                            │                                │ 34      reason: `${a.gstin}: ${reason}`,               │
├────────────────────────────┴────────────────────────────────┴────────────────────────────────────────────────────────┤
│ ↑↓ move · enter open · a add repo · d remove repo · ^p find · tab last worktree · → explorer · ? help · q quit       │
└─────────────────────────────────────────────────────────────────────────────────────────────── 3 repos · 8 worktrees ┘
```

- `locked`: the worktree is locked (`git worktree lock`). Dim suffix, otherwise normal.
- `(detached a1b2c3d)`: a worktree with a detached HEAD shows its short commit.
- `missing` on a worktree: its folder is gone (git calls it prunable). The row
  is dim and can't be opened.
- `missing` on a repo: its folder is gone. The row is dim, it has no worktrees,
  and `d` removes it.
- A bare repo lists only its linked worktrees.

### `missing`: a missing repo under the cursor

```text
┌ abc · old-scraper ────────────────────────────────────────────────────────────────────────────────────────────── dev ┐
│ WORKTREES                  │ Code                                                                                    │
│ privue-platform            ├─────────────────────────────────────────────────────────────────────────────────────────┤
│   feat/gst-check           │                                                                                         │
│   main                     │                                                                                         │
│ old-scraper        missing │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                ✗ old-scraper is missing                                 │
│                            │                                                                                         │
│                            │                          ~/code/old-scraper no longer exists.                           │
│                            │                                                                                         │
│                            │                                 d removes it from abc.                                  │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
├────────────────────────────┴─────────────────────────────────────────────────────────────────────────────────────────┤
│ ↑↓ move · enter open · a add repo · d remove repo · ^p find · tab last worktree · → explorer · ? help · q quit       │
└─────────────────────────────────────────────────────────────────────────────────────────────── 2 repos · 2 worktrees ┘
```

### `status`: status line messages

abcd rescans every 5 seconds, so a repo or worktree that disappears shows as
missing within 5 seconds, and one created outside abc appears within 5 seconds.

Normal, nothing to report:

```text
├────────────────────────────┴────────────────────────────────┴─────────────────────────────────────────── 1–34 of 58 ─┤
│ ↑↓ move · enter open · ^p find · tab last worktree · ←→ focus · a add repo · d remove repo · ? help · q quit         │
└─────────────────────────────────────────────────────────────────────────────────────────────── 2 repos · 7 worktrees ┘
```

After an action (clears after 5 seconds):

```text
├────────────────────────────┴────────────────────────────────┴─────────────────────────────────────────── 1–34 of 58 ─┤
│ ↑↓ move · enter open · ^p find · tab last worktree · ←→ focus · a add repo · d remove repo · ? help · q quit         │
└ ✓ added privue-agents ──────────────────────────────────────────────────────────────────────── 2 repos · 7 worktrees ┘
```

abcd was restarted because this abc is a newer build (clears after 5 seconds):

```text
├────────────────────────────┴────────────────────────────────┴─────────────────────────────────────────── 1–34 of 58 ─┤
│ ↑↓ move · enter open · ^p find · tab last worktree · ←→ focus · a add repo · d remove repo · ? help · q quit         │
└ ↻ abcd restarted · build 0.1.0+3f9a2c1e ────────────────────────────────────────────────────── 2 repos · 7 worktrees ┘
```

Lost the connection. The last data stays on screen, dimmed; anything that needs
abcd answers `✗ not connected`. The client reconnects every second and never
restarts abcd mid-session (ADR 0002):

```text
├────────────────────────────┴────────────────────────────────┴─────────────────────────────────────────── 1–34 of 58 ─┤
│ ↑↓ move · enter open · ^p find · tab last worktree · ←→ focus · a add repo · d remove repo · ? help · q quit         │
└ ✗ lost abcd · reconnecting… ────────────────────────────────────────────────────────────────── 2 repos · 7 worktrees ┘
```

This abc is older than the running abcd but speaks the same protocol. Everything
works; the warning stays until you reopen abc:

```text
├────────────────────────────┴────────────────────────────────┴─────────────────────────────────────────── 1–34 of 58 ─┤
│ ↑↓ move · enter open · ^p find · tab last worktree · ←→ focus · a add repo · d remove repo · ? help · q quit         │
└ ! this abc is older than the running abcd · quit and reopen abc ────────────────────────────── 2 repos · 7 worktrees ┘
```

### `stale`: this abc can't talk to the running abcd

```text
┌ abc ──────────────────────────────────────────────────────────────────────────────────────────────────────────── dev ┐
│ WORKTREES                  │ Code                                                                                    │
│                            ├─────────────────────────────────────────────────────────────────────────────────────────┤
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                       ! This abc is too old for the running abcd                        │
│                            │                                                                                         │
│                            │                     abc speaks protocol 1, abcd speaks protocol 2.                      │
│                            │                        Quit and reopen abc to use the new build.                        │
│                            │                                                                                         │
│                            │                                         q quit                                          │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
│                            │                                                                                         │
├────────────────────────────┴─────────────────────────────────────────────────────────────────────────────────────────┤
│ q quit                                                                                                               │
└ ! protocol mismatch ─────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### `small`: terminal smaller than 80x20

```text
                                                                
                                                                
                                                                
                                                                
                                                                
                                                                
                                                                
                                                                
              abc needs at least 80x20 (now 64x18)              
                                                                
                                                                
                                                                
                                                                
                                                                
                                                                
                                                                
                                                                
                                                                
```
