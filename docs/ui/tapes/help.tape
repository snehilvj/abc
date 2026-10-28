# The help overlay in the gallery, for PRs: `just tape help` writes target/vhs/help.gif.
# Needs FiraCode Nerd Font Mono. 1134x716 at 14px gives a 120x40 terminal.

Output target/vhs/help.gif

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
Type "target/debug/abc gallery help --theme dark"
Enter
Sleep 1.5s
# Start from the explorer, to open help the way you would.
Escape
Sleep 500ms
Show
Sleep 1s

# ? shows the explorer's keys; ? again closes.
Type "?"
Sleep 2.5s
Type "?"
Sleep 800ms

# Each pane lists its own keys: the viewer's.
Type "l"
Sleep 500ms
Type "?"
Sleep 3s
Escape
Sleep 800ms

# space ? does the same from the leader: the sidebar's. esc leaves the viewer, h the explorer.
Escape
Sleep 300ms
Type "h"
Sleep 500ms
Type " ?"
Sleep 2.5s
Escape
Sleep 1s

Type "q"
Sleep 500ms
