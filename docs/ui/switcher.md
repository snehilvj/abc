# Switcher

`ctrl-p` from anywhere in the frame. One fuzzy palette with two modes:
**Worktrees** (every worktree of every repo) and **Files** (the files of the
current worktree). `tab` switches mode and keeps the query. Matching uses
nucleo; matched characters are bold.

`abc gallery switcher [--state <state>]`, default `worktrees`.

## Worktrees mode

```text
┌ abc · privue-platform / feat/gst-check ───────────────────────────────────────────────────────────────────────── dev ┐
│ WORKTREES                  │ Code                                                                                    │
│ privue-platform            ├─ …t/privue-platform/gst-check ─┬─ packages/integrations/gst/verify.ts · ts ─────────────┤
│ ▸ feat/gst-check      ┌ Switch ──────────────────────────────────────────────────────────────┐./../shared/text"      │
│   fix/gst-rounding    │ [Worktrees]  Files                                     tab to change │om "./client"          │
│   fix/ledger-round    │ > gst█                                                        3 of 7 │                       │
│   main                ├──────────────────────────────────────────────────────────────────────┤                       │
│ ecourts               │ ▸ privue-platform   feat/gst-check                               now │                       │
│   feat/gst-lookup     │   privue-platform   fix/gst-rounding                          2h ago │                       │
│   feat/ocr-retry      │   ecourts           feat/gst-lookup                        yesterday │                       │
│   main                │                                                                      │                       │
│                       │                                                                      │                       │
│                       │                                                                      │                       │
│                       │                                                                      │ing }                  │
│                       │                                                                      │on: string }           │
│                       │                                                                      │                       │
│                       │                                                                      │e registry and the     │
│                       ├──────────────────────────────────────────────────────────────────────┤ us.                   │
│                       │ enter switch · ↑↓ move · ctrl-u clear · esc close                    │(                      │
│                       └──────────────────────────────────────────────────────────────────────┘                       │
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

- Each row: repo, branch (or `(detached a1b2c3d)`), and when you last opened it.
  The current worktree says `now`. Never opened: blank.
- Matches against `repo branch`. Best match first; ties go to the most recent.
- Missing worktrees and missing repos are not listed.
- `enter` opens the worktree and closes the switcher, just like `enter` in the
  sidebar.
- The list area is always 10 rows, so the box doesn't change size as you type.

## Files mode

```text
┌ Switch ──────────────────────────────────────────────────────────────┐
│ Worktrees  [Files]                                     tab to change │
│ > verif█                                                    3 of 412 │
├──────────────────────────────────────────────────────────────────────┤
│ ▸ packages/integrations/gst/verify.ts                                │
│   packages/scoring/verifier.py                                       │
│   apps/web/src/pages/verify-gst.tsx                                  │
│                                                                      │
│                                                                      │
│                                                                      │
│                                                                      │
│                                                                      │
│                                                                      │
│                                                                      │
├──────────────────────────────────────────────────────────────────────┤
│ enter open · ↑↓ move · ctrl-u clear · esc close                      │
└──────────────────────────────────────────────────────────────────────┘
```

- Paths are relative to the worktree, from abcd's file list (the same one the
  explorer uses, so `.gitignore` applies).
- `enter` opens the file in the viewer, expands its folders in the explorer and
  moves focus to the viewer.

## Keys

| Key               | Action                                       |
|-------------------|----------------------------------------------|
| typing            | Filter                                       |
| `backspace`       | Delete a character                           |
| `ctrl-u`          | Clear the query                              |
| `↑` `↓`           | Move the selection                           |
| `tab`             | Switch between Worktrees and Files           |
| `enter`           | Open the selected worktree or file           |
| `esc`             | Close                                        |

`?` and `q` type into the query here; they don't open help or quit.

## States

### `worktrees`

Above (query `gst`).

### `worktrees-empty-query`: what `ctrl-p` opens to

All worktrees, most recent first. The current one is listed first as `now` and
the selection starts on the next one, so `ctrl-p` `enter` flips back.

```text
┌ Switch ──────────────────────────────────────────────────────────────┐
│ [Worktrees]  Files                                     tab to change │
│ > █                                                                7 │
├──────────────────────────────────────────────────────────────────────┤
│   privue-platform   feat/gst-check                               now │
│ ▸ privue-platform   fix/gst-rounding                          2h ago │
│   ecourts           feat/gst-lookup                        yesterday │
│   privue-platform   main                                      3d ago │
│   ecourts           feat/ocr-retry                            5d ago │
│   ecourts           main                                      2w ago │
│   privue-platform   fix/ledger-round                                 │
│                                                                      │
│                                                                      │
│                                                                      │
├──────────────────────────────────────────────────────────────────────┤
│ enter switch · ↑↓ move · ctrl-u clear · esc close                    │
└──────────────────────────────────────────────────────────────────────┘
```

### `worktrees-no-match`

```text
┌ Switch ──────────────────────────────────────────────────────────────┐
│ [Worktrees]  Files                                     tab to change │
│ > zoho█                                                       0 of 7 │
├──────────────────────────────────────────────────────────────────────┤
│ No worktrees match "zoho".                                           │
│                                                                      │
│                                                                      │
│                                                                      │
│                                                                      │
│                                                                      │
│                                                                      │
│                                                                      │
│                                                                      │
│                                                                      │
├──────────────────────────────────────────────────────────────────────┤
│ enter switch · ↑↓ move · ctrl-u clear · esc close                    │
└──────────────────────────────────────────────────────────────────────┘
```

### `worktrees-none`: no repositories yet

```text
┌ Switch ──────────────────────────────────────────────────────────────┐
│ [Worktrees]  Files                                     tab to change │
│ > █                                                                0 │
├──────────────────────────────────────────────────────────────────────┤
│ No worktrees yet. Close this with esc and press a                    │
│ to add a repository.                                                 │
│                                                                      │
│                                                                      │
│                                                                      │
│                                                                      │
│                                                                      │
│                                                                      │
│                                                                      │
│                                                                      │
├──────────────────────────────────────────────────────────────────────┤
│ enter switch · ↑↓ move · ctrl-u clear · esc close                    │
└──────────────────────────────────────────────────────────────────────┘
```

### `files`

Above (query `verif`).

### `files-empty-query`

All files in path order.

```text
┌ Switch ──────────────────────────────────────────────────────────────┐
│ Worktrees  [Files]                                     tab to change │
│ > █                                                              412 │
├──────────────────────────────────────────────────────────────────────┤
│ ▸ .gitignore                                                         │
│   README.md                                                          │
│   apps/web/package.json                                              │
│   apps/web/src/main.tsx                                              │
│   apps/web/src/pages/home.tsx                                        │
│   apps/web/src/pages/verify-gst.tsx                                  │
│   docs/gst.md                                                        │
│   package.json                                                       │
│   packages/integrations/gst/client.ts                                │
│   packages/integrations/gst/verify.ts                                │
│   packages/scoring/score.py                                          │
│   packages/scoring/verifier.py                                       │
├──────────────────────────────────────────────────────────────────────┤
```

### `files-loading`: abcd is still listing the files

```text
┌ Switch ──────────────────────────────────────────────────────────────┐
│ Worktrees  [Files]                                     tab to change │
│ > verif█                                                    loading… │
├──────────────────────────────────────────────────────────────────────┤
│ Loading files…                                                       │
│                                                                      │
│                                                                      │
│                                                                      │
│                                                                      │
│                                                                      │
│                                                                      │
│                                                                      │
│                                                                      │
│                                                                      │
├──────────────────────────────────────────────────────────────────────┤
│ enter open · ↑↓ move · ctrl-u clear · esc close                      │
└──────────────────────────────────────────────────────────────────────┘
```

### `files-no-match`

```text
┌ Switch ──────────────────────────────────────────────────────────────┐
│ Worktrees  [Files]                                     tab to change │
│ > zzz█                                                      0 of 412 │
├──────────────────────────────────────────────────────────────────────┤
│ No files match "zzz" in feat/gst-check.                              │
│                                                                      │
│                                                                      │
│                                                                      │
│                                                                      │
│                                                                      │
│                                                                      │
│                                                                      │
│                                                                      │
│                                                                      │
├──────────────────────────────────────────────────────────────────────┤
│ enter open · ↑↓ move · ctrl-u clear · esc close                      │
└──────────────────────────────────────────────────────────────────────┘
```

### `files-error`

```text
┌ Switch ──────────────────────────────────────────────────────────────┐
│ Worktrees  [Files]                                     tab to change │
│ > █                                                                  │
├──────────────────────────────────────────────────────────────────────┤
│ ✗ Can't list files: permission denied                                │
│                                                                      │
│                                                                      │
│                                                                      │
│                                                                      │
│                                                                      │
│                                                                      │
│                                                                      │
│                                                                      │
│                                                                      │
├──────────────────────────────────────────────────────────────────────┤
│ enter open · ↑↓ move · ctrl-u clear · esc close                      │
└──────────────────────────────────────────────────────────────────────┘
```

### `files-no-worktree`

```text
┌ Switch ──────────────────────────────────────────────────────────────┐
│ Worktrees  [Files]                                     tab to change │
│ > █                                                                  │
├──────────────────────────────────────────────────────────────────────┤
│ Open a worktree first.                                               │
│                                                                      │
│                                                                      │
│                                                                      │
│                                                                      │
│                                                                      │
│                                                                      │
│                                                                      │
│                                                                      │
│                                                                      │
├──────────────────────────────────────────────────────────────────────┤
│ enter open · ↑↓ move · ctrl-u clear · esc close                      │
└──────────────────────────────────────────────────────────────────────┘
```
