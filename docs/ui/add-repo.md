# Add repository

`a` from the frame. Registers a git repository with abc; it never changes
anything on disk. The same thing as `abc repo add <path> [--name <n>]`.

`abc gallery add-repo [--state <state>]`, default `valid`.

## Typing a path

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
│                       ┌ Add repository ──────────────────────────────────────────────────────┐                       │
│                       │                                                                      │ing }                  │
│                       │ Path  ~/code/privue-ag█                                              │on: string }           │
│                       │       ~/code/privue-agents/                                          │                       │
│                       │       ~/code/privue-agreements/                                      │e registry and the     │
│                       │                                                                      │ us.                   │
│                       │ Name                          (the folder name)                      │(                      │
│                       │                                                                      │                       │
│                       │                                                                      │                       │
│                       ├──────────────────────────────────────────────────────────────────────┤                       │
│                       │ tab complete · ↑↓ path/name · enter add · esc cancel                 │                       │
│                       └──────────────────────────────────────────────────────────────────────┘t.lookup(a.gstin)      │
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

## Layout

```text
┌ Add repository ──────────────────────────────────────────────────────┐
│                                                                      │
│ Path  ~/code/privue-agents/█                                         │
│                                                                      │
│ Name  privue-agents           (defaults to the folder name)          │
│                                                                      │
│ ✓ git repository · 3 worktrees found                                 │
├──────────────────────────────────────────────────────────────────────┤
│ tab complete · ↑↓ path/name · enter add · esc cancel                 │
└──────────────────────────────────────────────────────────────────────┘
```

- **Path** starts as the current repo's parent folder (e.g. `~/code/`), or `~/`
  when there are no repos. `~` is expanded.
- **Completions**: after `tab`, up to 3 folders that start with what you typed
  appear under the path and push the rest down; the box grows downwards from a
  fixed top. Hidden folders appear only when the typed name starts with `.`.
  abcd lists them; the client doesn't read the disk.
- **Name** follows the folder name until you edit it. A `.git` suffix is dropped
  (`ecourts.git` → `ecourts`).
- **Check line**: abcd checks the path 150 ms after you stop typing.
- After `enter` succeeds the dialog closes, the new repo appears in the sidebar
  and the status line says `✓ added privue-agents`.

## Keys

| Key        | Action                                                                     |
|------------|----------------------------------------------------------------------------|
| typing     | Edit the focused field                                                     |
| `tab`      | In Path: complete as far as the folders agree; press again to cycle through them |
| `shift-tab`| Cycle completions backwards                                                |
| `↑` `↓`    | Move between Path and Name                                                 |
| `ctrl-u`   | Clear the focused field                                                    |
| `enter`    | Add, when the check line is ✓                                              |
| `esc`      | Cancel                                                                     |

## States

### `typing`

Above: after `tab` with two folders matching. Nothing is checked until the path
names an existing folder.

### `valid`

Above.

### `inside`: a path inside a repo, or one of its worktrees

abc registers the repository itself, never a single worktree.

```text
┌ Add repository ──────────────────────────────────────────────────────┐
│                                                                      │
│ Path  ~/code/wt/privue-agents/ocr█                                   │
│                                                                      │
│ Name  privue-agents           (defaults to the folder name)          │
│                                                                      │
│ ✓ a worktree of ~/code/privue-agents · adds that repo                │
├──────────────────────────────────────────────────────────────────────┤
│ tab complete · ↑↓ path/name · enter add · esc cancel                 │
└──────────────────────────────────────────────────────────────────────┘
```

### `not-git`

```text
┌ Add repository ──────────────────────────────────────────────────────┐
│                                                                      │
│ Path  ~/Downloads/█                                                  │
│                                                                      │
│ Name  Downloads               (defaults to the folder name)          │
│                                                                      │
│ ✗ not a git repository                                               │
├──────────────────────────────────────────────────────────────────────┤
│ tab complete · ↑↓ path/name · enter add · esc cancel                 │
└──────────────────────────────────────────────────────────────────────┘
```

### `duplicate`: the repository is already registered

Matched by the repository's git folder, so adding it again through one of its
worktrees is caught too.

```text
┌ Add repository ──────────────────────────────────────────────────────┐
│                                                                      │
│ Path  ~/code/ecourts/█                                               │
│                                                                      │
│ Name  ecourts                 (defaults to the folder name)          │
│                                                                      │
│ ✗ already added as ecourts                                           │
├──────────────────────────────────────────────────────────────────────┤
│ tab complete · ↑↓ path/name · enter add · esc cancel                 │
└──────────────────────────────────────────────────────────────────────┘
```

### `not-found`: no folder, and nothing to complete

```text
┌ Add repository ──────────────────────────────────────────────────────┐
│                                                                      │
│ Path  ~/code/privue-agnts█                                           │
│                                                                      │
│ Name                          (the folder name)                      │
│                                                                      │
│ ✗ no such folder                                                     │
├──────────────────────────────────────────────────────────────────────┤
│ tab complete · ↑↓ path/name · enter add · esc cancel                 │
└──────────────────────────────────────────────────────────────────────┘
```

### `name-taken`: a different repository already uses the name

Focus moves to Name so you can change it.

```text
┌ Add repository ──────────────────────────────────────────────────────┐
│                                                                      │
│ Path  ~/work/ecourts/█                                               │
│                                                                      │
│ Name  ecourts█                                                       │
│                                                                      │
│ ✗ the name ecourts is taken; pick another                            │
├──────────────────────────────────────────────────────────────────────┤
│ tab complete · ↑↓ path/name · enter add · esc cancel                 │
└──────────────────────────────────────────────────────────────────────┘
```

### `checking`

```text
┌ Add repository ──────────────────────────────────────────────────────┐
│                                                                      │
│ Path  ~/code/privue-agents/█                                         │
│                                                                      │
│ Name  privue-agents           (defaults to the folder name)          │
│                                                                      │
│ … checking                                                           │
├──────────────────────────────────────────────────────────────────────┤
│ tab complete · ↑↓ path/name · enter add · esc cancel                 │
└──────────────────────────────────────────────────────────────────────┘
```

### `adding`

```text
┌ Add repository ──────────────────────────────────────────────────────┐
│                                                                      │
│ Path  ~/code/privue-agents/                                          │
│                                                                      │
│ Name  privue-agents           (defaults to the folder name)          │
│                                                                      │
│ … adding                                                             │
├──────────────────────────────────────────────────────────────────────┤
│ esc cancel                                                           │
└──────────────────────────────────────────────────────────────────────┘
```

### `error`: abcd failed or isn't connected

```text
┌ Add repository ──────────────────────────────────────────────────────┐
│                                                                      │
│ Path  ~/code/privue-agents/█                                         │
│                                                                      │
│ Name  privue-agents           (defaults to the folder name)          │
│                                                                      │
│ ✗ abcd: not connected                                                │
├──────────────────────────────────────────────────────────────────────┤
│ tab complete · ↑↓ path/name · enter add · esc cancel                 │
└──────────────────────────────────────────────────────────────────────┘
```
