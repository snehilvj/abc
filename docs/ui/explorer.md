# File explorer

The left pane of the Code tab: the files of the current worktree as a tree.
It respects `.gitignore` (and `.ignore` and git's global excludes), shows dotfiles
such as `.github/`, and never shows `.git`.

`abc gallery explorer [--state <state>]`, default `tree`.

## Layout

```text
├─ …t/privue-platform/gst-check ─┬
│ packages/                      │
│ ├─ integrations/               │
│ │  └─ gst/                     │
│ │     ├─ verify.ts  ◂          │
│ │     └─ client.ts             │
│ ├─ scoring/                    │
│ └─ shared/                     │
│ apps/                          │
│ docs/                          │
│ .gitignore                     │
│ package.json                   │
│ README.md                      │
│ tsconfig.json                  │
│                                │
```

- Folders first, then files, each sorted by name. Folders end in `/`.
- Folders start collapsed. An expanded folder's children are drawn under it with
  tree guides; a collapsed folder shows nothing under it.
- `◂` marks the file open in the viewer. The cursor row is reverse video.
- Opening a file from the switcher expands its folders and moves the cursor to it.
- Long names are cut with `…`.
- The header shows the worktree's path, trimmed from the left.

## Keys

| Key               | Action                                                  |
|-------------------|---------------------------------------------------------|
| `↑` `↓` / `j` `k` | Move the cursor                                         |
| `g` `G`           | First / last row                                        |
| `enter`           | On a file: open it in the viewer. On a folder: expand or collapse it |
| `→` / `l`         | Move focus to the viewer                                |
| `←` / `h`         | Move focus to the sidebar                               |

Plus the [global keys](README.md#keys-everywhere). Key bar:

```text
│ ↑↓ move · enter open · ^p find · tab last worktree · ←→ focus · a add repo · d remove repo · ? help · q quit         │
```

## States

### `tree`

Above.

### `loading`

```text
├─ …t/privue-platform/gst-check ─┬
│ Loading files…                 │
│                                │
│                                │
│                                │
│                                │
│                                │
│                                │
│                                │
│                                │
│                                │
```

### `empty`: nothing to show

```text
├─ …t/privue-platform/gst-check ─┬
│ No files.                      │
│                                │
│ The worktree is empty, or      │
│ everything in it is ignored.   │
│                                │
│                                │
│                                │
│                                │
│                                │
│                                │
```

### `error`: abcd couldn't list the files

```text
├─ …t/privue-platform/gst-check ─┬
│ ✗ Can't list files             │
│                                │
│ permission denied              │
│                                │
│                                │
│                                │
│                                │
│                                │
│                                │
│                                │
```

### `missing`: the worktree's folder is gone

```text
├─ …wt/privue-platform/old-idea ─┬
│ ✗ This worktree is missing     │
│                                │
│ ~/code/wt/privue-platform/     │
│ old-idea no longer exists.     │
│                                │
│                                │
│                                │
│                                │
│                                │
│                                │
```
