# The file explorer in the gallery, for PRs: `just tape explorer` writes target/vhs/explorer.gif.
# Needs FiraCode Nerd Font Mono. 1134x716 at 14px gives a 120x40 terminal.

Output target/vhs/explorer.gif

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
Type "target/debug/abc gallery explorer --theme dark"
Enter
Sleep 1.5s
Show
Sleep 2s

# Up to the gst folder and fold it, then down into scoring and open it.
Type "kk"
Sleep 800ms
Enter
Sleep 1s
Type "j"
Sleep 800ms
Enter
Sleep 1s

# Open a file: its name turns to the accent colour.
Type "j"
Sleep 800ms
Enter
Sleep 1.5s

# To the ends of the tree and back.
Type "G"
Sleep 1.2s
Type "gg"
Sleep 1.2s

# Each worktree keeps its own explorer: away to another worktree and back with tab.
Type "hj"
Sleep 800ms
Enter
Sleep 1.5s
Tab
Sleep 2s

# The other states.
Type "]"
Sleep 1.5s
Type "]"
Sleep 1.5s
Type "]"
Sleep 2s

Type "q"
Sleep 500ms
