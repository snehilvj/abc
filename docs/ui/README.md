# UI

Every screen is designed here before it is built. Each one goes through the same
steps, stopping for approval where marked:

1. Wireframe in this folder, with keys and empty, loading and error states. **Approval.**
2. Built against fake data: `abc gallery <screen> [--state <state>]`, no server needed. **Review.**
3. insta snapshot test at 120x40 that reads like the wireframe.
4. VHS tape in `tapes/<screen>.tape`; `just tape <screen>` writes `target/vhs/<screen>.gif`.
5. Connected to real data from abcd.

## Slice 1 screens

| Screen            | Wireframe                          | Gallery                   |
|-------------------|------------------------------------|---------------------------|
| App frame         | [frame.md](frame.md)               | `abc gallery frame`       |
| File explorer     | [explorer.md](explorer.md)         | `abc gallery explorer`    |
| File viewer       | [viewer.md](viewer.md)             | `abc gallery viewer`      |
| Switcher          | [switcher.md](switcher.md)         | `abc gallery switcher`    |
| Add repository    | [add-repo.md](add-repo.md)         | `abc gallery add-repo`    |
| Remove repository | [remove-repo.md](remove-repo.md)   | `abc gallery remove-repo` |
| Help overlay      | [help.md](help.md)                 | `abc gallery help`        |
| CLI output        | [cli.md](cli.md)                   | snapshot tests only       |

Each `--state` name matches a heading under **States** in the screen's file.

## Drawing conventions

- Full-screen wireframes are exactly 120x40, the size of the snapshot tests, so
  the two compare line by line. States that change one part of the screen show
  only that part, cut from the same 120x40 grid.
- Text can't show styling, so in words:
  - the cursor row (where you are in a focused list) is **reverse video**;
  - the focused pane's header is **bold cyan**; other headers are dim;
  - matched characters in the switcher are **bold**;
  - "dim" means the ANSI DarkGray colour.
- `█` marks the terminal cursor in a text field. abc uses the real terminal
  cursor, so snapshots show the character under it rather than `█`.
- Glyphs:
  `▸` current worktree in the sidebar, or the selected row in a list ·
  `◂` the file open in the viewer ·
  `✓` done · `✗` error · `…` in progress · `◌` connecting · `↻` restarted · `!` warning
- Colours: only the 16 ANSI colours, so abc follows your terminal theme.
  Accent cyan, ok green, warning yellow, error red, secondary DarkGray.

## Layout and sizes

- The sidebar is 28 columns, the explorer 32, and the viewer takes the rest
  (56 at 120 columns).
- On narrower terminals the viewer shrinks first, down to 40 columns, then the
  explorer shrinks, down to 20.
- Below 80x20 the screen shows only `abc needs at least 80x20 (now 64x18)`.
- Dialogs are centred and keep their size. The switcher sits 3 rows from the top.

## Keys everywhere

These work in the frame whenever no dialog is open. Inside the switcher and the
dialogs, letters type text, and each one lists all of its keys in its footer.

| Key         | Action                                                             |
|-------------|--------------------------------------------------------------------|
| `ctrl-p`    | Open the switcher                                                  |
| `tab`       | Jump back to the last worktree                                     |
| `←` `→` / `h` `l` | Move focus: sidebar · explorer · viewer                      |
| `a`         | Add a repository                                                   |
| `d`         | Remove a repository: the one under the sidebar cursor when the sidebar has focus, otherwise the current one |
| `?`         | Help for the focused pane                                          |
| `q`         | Quit. abcd keeps running.                                          |

## Choices to confirm at this review

1. **Focus keys.** `tab` is taken by "last worktree", so `←` `→` (and `h` `l`)
   move focus between panes everywhere, and `enter` opens and closes folders in
   the explorer. The alternative is "spatial" arrows: `→` expands a folder and
   moves on to the viewer only from a file.
2. **Status line in the bottom border**, mirroring the title in the top border:
   messages on the left, counts on the right. The profile name sits at the
   top right (`dev` in yellow, `stable` dim) so the two are never confused.
3. **Switcher opens on the last worktree.** With an empty query the current
   worktree is listed first as `now` and the cursor starts on the next one, so
   `ctrl-p` `enter` flips back, like `tab`.
4. **Add repository starts in the current repo's parent folder**, e.g. `~/code/`.
