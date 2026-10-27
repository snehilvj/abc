# Remove repository

`d` from the frame. Unregisters a repository; it never deletes files. The same
thing as `abc repo remove <name>`.

`d` acts on the repo under the sidebar cursor when the sidebar has focus, and on
the current worktree's repo otherwise. With no repos, `d` does nothing.

`abc gallery remove-repo [--state <state>]`, default `confirm`.

## Layout

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
│                            │┌ Remove repository ───────────────────────────────────────┐                             │
│                            ││                                                          │nst the registry and the     │
│                            ││ Remove ecourts from abc?                                 │r gave us.                   │
│                            ││                                                          │verify(                      │
│                            ││ Files stay on disk; only the registration goes.          │                             │
│                            ││                                                          │                             │
│                            │├──────────────────────────────────────────────────────────┤                             │
│                            ││ y remove · n keep                                        │input)                       │
│                            │└──────────────────────────────────────────────────────────┘ client.lookup(a.gstin)      │
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

After `y` succeeds the dialog closes, the repo leaves the sidebar and the status
line says `✓ removed ecourts`. If the current worktree belonged to it, abc
switches to the last worktree, or shows the first-run screen when none are left.

## Keys

| Key           | Action  |
|---------------|---------|
| `y`           | Remove  |
| `n` / `esc`   | Keep    |

## States

### `confirm`

Above.

### `missing`: the repository's folder is already gone

```text
┌ Remove repository ───────────────────────────────────────┐
│                                                          │
│ Remove old-scraper from abc?                             │
│                                                          │
│ Its folder ~/code/old-scraper is already gone.           │
│                                                          │
├──────────────────────────────────────────────────────────┤
│ y remove · n keep                                        │
└──────────────────────────────────────────────────────────┘
```

### `removing`

```text
┌ Remove repository ───────────────────────────────────────┐
│                                                          │
│ Remove ecourts from abc?                                 │
│                                                          │
│ Files stay on disk; only the registration goes.          │
│ … removing                                               │
├──────────────────────────────────────────────────────────┤
│                                                          │
└──────────────────────────────────────────────────────────┘
```

### `error`

```text
┌ Remove repository ───────────────────────────────────────┐
│                                                          │
│ Remove ecourts from abc?                                 │
│                                                          │
│ Files stay on disk; only the registration goes.          │
│ ✗ abcd: not connected                                    │
├──────────────────────────────────────────────────────────┤
│ esc close                                                │
└──────────────────────────────────────────────────────────┘
```
