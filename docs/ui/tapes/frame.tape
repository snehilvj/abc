# The app frame in the gallery, for PRs: `just tape frame` writes target/vhs/frame.gif.
# Needs FiraCode Nerd Font Mono. 1134x716 at 14px gives a 120x40 terminal.

Output target/vhs/frame.gif

Set Shell zsh
Set FontFamily "FiraCode Nerd Font Mono"
Set FontSize 14
Set Width 1134
Set Height 716
Set Padding 12
Set TypingSpeed 120ms

Hide
Type "target/debug/abc gallery frame --theme dark"
Enter
Sleep 1.5s
Show
Sleep 2s

# Into the sidebar, down to another worktree, open it.
Type "h"
Sleep 800ms
Type "jj"
Sleep 800ms
Enter
Sleep 1.5s

# tab flips back to the last worktree, and again.
Tab
Sleep 1.5s
Tab
Sleep 1.5s

# The leader waits for the rest of a sequence; an unbound key says so.
Space
Sleep 1s
Type "x"
Sleep 2s

# Step through some of the other states.
Type "]"
Sleep 2s
Type "]"
Sleep 2s
Type "]]"
Sleep 2s
Type "]]"
Sleep 2s
Type "]]"
Sleep 2s

Type "q"
Sleep 500ms
