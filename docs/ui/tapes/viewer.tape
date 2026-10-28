# The file viewer in the gallery, for PRs: `just tape viewer` writes target/vhs/viewer.gif.
# Needs FiraCode Nerd Font Mono. 1134x716 at 14px gives a 120x40 terminal.
# It never presses y or g y: those would put text on your clipboard.

Output target/vhs/viewer.gif

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
Type "target/debug/abc gallery viewer --theme dark"
Enter
Sleep 1.5s
Show
Sleep 2s

# Down the file, then along a line by words.
Type "jjjj"
Sleep 800ms
Type "www"
Sleep 1s

# To the line's end, which scrolls the view sideways, and back.
Type "$"
Sleep 1.5s
Type "0"
Sleep 1s

# Half a page down, to the end, to the top, then a count: 12 lines down.
Ctrl+D
Sleep 1.2s
Type "G"
Sleep 1.2s
Type "gg"
Sleep 1.2s
Type "12j"
Sleep 1.5s

# Select three lines, then let go.
Type "Vjj"
Sleep 1.5s
Escape
Sleep 800ms

# Find gstin as it's typed, then the next matches; esc hides them.
Type "/gstin"
Sleep 1s
Enter
Sleep 1.2s
Type "n"
Sleep 1s
Type "n"
Sleep 1.2s
Escape
Sleep 1s

# Go to line 42, then back where it was.
Type ":42"
Sleep 800ms
Enter
Sleep 1.2s
Ctrl+O
Sleep 1.5s

# The other states, from the last one back.
Type "["
Sleep 1.5s
Type "["
Sleep 1.5s
Type "["
Sleep 1.5s
Type "["
Sleep 1.5s
Type "["
Sleep 1.5s
Type "["
Sleep 1.5s
Type "["
Sleep 2s

Type "q"
Sleep 500ms
