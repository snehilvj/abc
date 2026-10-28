# Removing a repository in the gallery, for PRs: `just tape remove-repo` writes target/vhs/remove-repo.gif.
# Needs FiraCode Nerd Font Mono. 1134x716 at 14px gives a 120x40 terminal.

Output target/vhs/remove-repo.gif

Set Shell zsh
Set FontFamily "FiraCode Nerd Font Mono"
Set FontSize 14
Set Width 1134
Set Height 716
Set Padding 12
Set TypingSpeed 150ms

Hide
# Give the shell time to start, or the first keys are lost.
Sleep 1.5s
Type "target/debug/abc gallery remove-repo --theme dark"
Enter
Sleep 1.5s
# Start from the sidebar, on an ecourts worktree.
Escape
Sleep 500ms
Show
Sleep 1s

# space r d asks about the repository under the cursor; n keeps it.
Type " rd"
Sleep 1.5s
Type "n"
Sleep 1s

# y removes it: it leaves the sidebar, and the cursor stays on the nearest row.
Type " rd"
Sleep 1.2s
Type "y"
Sleep 2s

# From the explorer, the open worktree's repository: the last worktree opens in its place.
Hide
Type "]]]]"
Sleep 300ms
Show
Sleep 1.5s
Type "y"
Sleep 2s

# A repository whose folder is gone (stepping past the states between unseen).
Hide
Type "]]"
Sleep 300ms
Show
Sleep 1.5s
Type "y"
Sleep 2s

Type "q"
Sleep 500ms
