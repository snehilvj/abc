# Adding a repository in the gallery, for PRs: `just tape add-repo` writes target/vhs/add-repo.gif.
# Needs FiraCode Nerd Font Mono. 1134x716 at 14px gives a 120x40 terminal.

Output target/vhs/add-repo.gif

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
Type "target/debug/abc gallery add-repo --theme dark"
Enter
Sleep 1.5s
# Start from the frame, to open the dialog the way you would.
Escape
Sleep 500ms
Show
Sleep 1s

Type " ra"
Sleep 1.5s

# A folder: abcd checks it once typing pauses, and the name is the folder's.
Type@40ms "~/code/privue-agents"
Sleep 2s

# A git URL clones into ~/code under the repository's name.
Ctrl+U
Type@30ms "https://github.com/privue/privue-docs"
Sleep 2s

# Naming it decides where the clone goes.
Down
Type "docs"
Sleep 2s

# enter clones and adds it: it joins the sidebar.
Enter
Sleep 2.5s

# A folder whose name is taken can be added under another.
Type " ra"
Sleep 800ms
Type@40ms "~/work/ecourts"
Sleep 2s
Down
Type@80ms "ecourts-work"
Sleep 2s

# Not a repository at all.
Up
Ctrl+U
Type@40ms "~/Downloads"
Sleep 2s

Escape
Sleep 1s
Type "q"
Sleep 500ms
