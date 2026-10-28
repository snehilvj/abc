# The switcher in the gallery, for PRs: `just tape switcher` writes target/vhs/switcher.gif.
# Needs FiraCode Nerd Font Mono. 1134x716 at 14px gives a 120x40 terminal.

Output target/vhs/switcher.gif

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
Type "target/debug/abc gallery switcher --state on-open --theme dark"
Enter
Sleep 1.5s
Show
Sleep 2s

# Most recent first, starting on the last worktree: enter goes back to it.
Enter
Sleep 1.5s

# Back with ctrl-p enter, then find a worktree by name.
Ctrl+P
Sleep 1s
Enter
Sleep 1.5s
Ctrl+P
Sleep 800ms
Type "ocr"
Sleep 1.2s
Escape
Sleep 1s

# Files: tab changes mode and keeps the query; enter opens the file in the viewer.
Ctrl+P
Sleep 800ms
Type "verif"
Sleep 1s
Tab
Sleep 1.2s
Down
Sleep 800ms
Enter
Sleep 2s

# The other states.
Type "]"
Sleep 1.5s
Type "]"
Sleep 1.5s
Type "]"
Sleep 1.5s
Type "]"
Sleep 2s

Type "q"
Sleep 500ms
