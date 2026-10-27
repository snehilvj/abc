# CLI output

Every command acts through abcd, starting it first when it isn't running. The
exceptions are `abc server status` and `abc server stop`, which never start it,
and `abc logs`, which reads the log file directly so it works when abcd is down.
Output is plain text with paths shortened to `~`. Snapshot tests cover each
message below.

All commands take `--profile <name>` (or `ABC_PROFILE`); the default is `stable`.

## `abc`

Opens the TUI ([frame.md](frame.md)).

## Messages on stderr when a command touches the server

```text
abc: started abcd (pid 4242)
abc: restarted abcd for build 0.1.0+3f9a2c1e
abc: warning: this abc is older than the running abcd; rebuild or reinstall abc
abc: this abc can't talk to the running abcd (protocol 1 vs 2); rebuild or reinstall abc
```

The first three still run the command. The last exits with code 1.

## `abc repo list`

```text
$ abc repo list
NAME             WORKTREES  PATH
privue-platform  4          ~/code/privue-platform
ecourts          3          ~/code/ecourts
old-scraper      missing    ~/code/old-scraper
```

In the order the repos were added. With none:

```text
$ abc repo list
No repositories. Add one with: abc repo add <path>
```

## `abc repo add <path> [--name <name>]`

```text
$ abc repo add ~/code/privue-agents
added privue-agents · ~/code/privue-agents · 3 worktrees

$ abc repo add ~/code/wt/privue-agents/ocr
added privue-agents · ~/code/privue-agents · 3 worktrees
(~/code/wt/privue-agents/ocr is one of its worktrees)

$ abc repo add ~/work/ecourts --name ecourts-work
added ecourts-work · ~/work/ecourts · 1 worktree
```

Errors (exit code 1):

```text
$ abc repo add ~/Downloads
error: ~/Downloads is not a git repository

$ abc repo add ~/code/nope
error: ~/code/nope does not exist

$ abc repo add ~/code/ecourts
error: ~/code/ecourts is already added as ecourts

$ abc repo add ~/work/ecourts
error: the name ecourts is taken; choose another with --name
```

## `abc repo remove <name>`

```text
$ abc repo remove ecourts
removed ecourts · files left on disk at ~/code/ecourts

$ abc repo remove nope
error: no repository named nope (see: abc repo list)
```

## `abc server status`

```text
$ abc server status
abcd is running · profile dev
  pid       4242
  build     0.1.0+3f9a2c1e (same as this abc)
  protocol  1
  uptime    2h 14m
  socket    ~/.local/state/abc/dev/abcd.sock
  log       ~/.local/state/abc/dev/abcd.log
  repos     3 (7 worktrees)

$ abc server status
abcd is not running · profile dev
  socket    ~/.local/state/abc/dev/abcd.sock
```

When the builds differ, the build line reads
`0.1.0+3f9a2c1e (this abc is 0.1.0+77d0e5b1)`.

Exit code 0 when running, 1 when not.

## `abc server restart`

Stops abcd if it is running, then starts it and waits until it answers.

```text
$ abc server restart
restarted abcd · pid 4242 → 4317

$ abc server restart
started abcd · pid 4317
```

## `abc server stop`

```text
$ abc server stop
stopped abcd (pid 4317)

$ abc server stop
abcd is not running
```

Both exit with code 0.

## `abc logs`

Prints the last 50 lines of `abcd.log`, then follows it until `ctrl-c`. When
abcd restarts, the previous run's log moves to `abcd.log.1` and `abc logs`
carries on with the new file.

```text
$ abc logs
2026-10-27T10:14:03.512Z  INFO abcd: listening socket=~/.local/state/abc/dev/abcd.sock build=0.1.0+3f9a2c1e
2026-10-27T10:14:03.530Z  INFO abcd::registry: loaded repos=3
2026-10-27T10:15:41.002Z  INFO abcd::registry: added name=privue-agents path=~/code/privue-agents
…

$ abc logs
no log yet at ~/.local/state/abc/dev/abcd.log
```

## Exit codes

| Code | Meaning                                                      |
|------|--------------------------------------------------------------|
| 0    | Success                                                      |
| 1    | The command failed: a rejected path, an unknown name, abcd unreachable, or `server status` with abcd not running |
| 2    | Bad arguments (from clap)                                    |
