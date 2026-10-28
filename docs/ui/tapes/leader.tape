# The which-key popup in the gallery, for PRs: `just tape leader` writes target/vhs/leader.gif.
# Needs FiraCode Nerd Font Mono. 1134x716 at 14px gives a 120x40 terminal.

Output target/vhs/leader.gif

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
Type "target/debug/abc gallery leader --theme dark"
Enter
Sleep 1.5s
# Start with nothing typed.
Escape
Sleep 500ms
Show
Sleep 1s

# Typed straight through, a sequence shows nothing: space f opens the switcher.
Type@80ms " f"
Sleep 1.5s
Escape
Sleep 800ms

# Left waiting, space lists what can follow; r opens the repos group.
Type " "
Sleep 2s
Type "r"
Sleep 2s

# backspace takes back a key; esc cancels.
Backspace
Sleep 1.5s
Escape
Sleep 1s

# A key that can't follow cancels and says so.
Type " x"
Sleep 2s

# Any sequence, such as the viewer's g.
Type "l"
Sleep 500ms
Type "g"
Sleep 2s
Type "g"
Sleep 1.5s

Type "q"
Sleep 500ms
