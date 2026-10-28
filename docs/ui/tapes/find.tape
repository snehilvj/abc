# Find in files in the gallery, for PRs: `just tape find` writes target/vhs/find.gif.
# Needs FiraCode Nerd Font Mono. 1134x716 at 14px gives a 120x40 terminal.

Output target/vhs/find.gif

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
Type "target/debug/abc gallery find --state empty --theme dark"
Enter
Sleep 1.5s
Show
Sleep 1.5s

# Results arrive a file at a time once typing pauses.
Type "gstin"
Sleep 2s

# Down to client.ts line 4 and open it: the cursor lands on the match.
Down 7
Sleep 1s
Enter
Sleep 2s

# n goes on through the file's matches.
Type "n"
Sleep 1s
Type "n"
Sleep 1.2s

# space / again shows the last search.
Type " /"
Sleep 1.5s
Escape
Sleep 800ms

# Too many lines: the search stops at 1,000 and says so.
Type " /"
Sleep 500ms
Ctrl+U
Type "row_"
Sleep 2.5s

# The other states.
Type "]"
Sleep 1.5s
Type "]"
Sleep 2s

Type "q"
Sleep 500ms
