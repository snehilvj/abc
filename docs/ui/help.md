# Help overlay

`?` from the frame. Lists every key for the focused pane, then the keys that
work everywhere. It comes from the same key table the key bar uses, so it can't
drift from what the keys do.

The switcher and the dialogs list all their keys in their footers; there `?`
types a `?`.

`abc gallery help [--state <state>]`, default `explorer`.

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
│   main                    ┌ Keys · Explorer ─────────────────────────────────────────────┐                           │
│                           │                                                              │                           │
│                           │ Explorer                                                     │                           │
│                           │   ↑ ↓  j k          move                                     │ string }                  │
│                           │   g G               first · last                             │reason: string }           │
│                           │   enter             open file · expand or collapse folder    │                           │
│                           │                                                              │t the registry and the     │
│                           │ Everywhere                                                   │gave us.                   │
│                           │   ctrl-p            find a worktree or file                  │rify(                      │
│                           │   tab               last worktree                            │                           │
│                           │   ← →  h l          move focus between panes                 │                           │
│                           │   a                 add repository                           │                           │
│                           │   d                 remove repository                        │put)                       │
│                           │   ?                 this help                                │lient.lookup(a.gstin)      │
│                           │   q                 quit (abcd keeps running)                │                           │
│                           │                                                              │not registered")           │
│                           ├──────────────────────────────────────────────────────────────┤                           │
│                           │ esc or ? close                                               │== a.legalName) {          │
│                           └──────────────────────────────────────────────────────────────┘name differs")             │
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

## Keys

| Key           | Action |
|---------------|--------|
| `esc` / `?`   | Close  |

## States

There is no empty, loading or error state: the overlay is static. One state per
pane.

### `explorer`

Above.

### `viewer`

```text
┌ Keys · Viewer ───────────────────────────────────────────────┐
│                                                              │
│ Viewer                                                       │
│   ↑ ↓  j k          scroll a line                            │
│   ctrl-d ctrl-u     half a page down · up                    │
│   pgdn pgup space   a page down · up                         │
│   g G               top · end                                │
│   esc               back to the explorer                     │
│                                                              │
│ Everywhere                                                   │
│   ctrl-p            find a worktree or file                  │
│   tab               last worktree                            │
│   ← →  h l          move focus between panes                 │
│   a                 add repository                           │
│   d                 remove repository                        │
│   ?                 this help                                │
│   q                 quit (abcd keeps running)                │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ esc or ? close                                               │
└──────────────────────────────────────────────────────────────┘
```

### `sidebar`

```text
┌ Keys · Sidebar ──────────────────────────────────────────────┐
│                                                              │
│ Sidebar                                                      │
│   ↑ ↓  j k          move                                     │
│   g G               first · last                             │
│   enter             open worktree (on a repo: its main one)  │
│                                                              │
│ Everywhere                                                   │
│   ctrl-p            find a worktree or file                  │
│   tab               last worktree                            │
│   ← →  h l          move focus between panes                 │
│   a                 add repository                           │
│   d                 remove repository                        │
│   ?                 this help                                │
│   q                 quit (abcd keeps running)                │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ esc or ? close                                               │
└──────────────────────────────────────────────────────────────┘
```
