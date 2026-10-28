# 0002: Server/client split, protocol and profiles

- Status: accepted, amended by 0004 (the client also reads its own settings file)
- Date: 2026-10-27

## Context

State (registered repos now; sessions and services later) must outlive any one
terminal. Several clients, both TUIs and one-shot CLI commands, must see the
same state. A dev build must run beside the stable build used every day without
either touching the other.

## Decision

**Split.** `abcd` owns all state and does all filesystem and git access. `abc`
(TUI and CLI) acts only through it. The one exception is `abc logs`, which tails
the log file directly so it works when the server is down. `abc-core` holds the
model types, the protocol, profile paths and the client connection.

**Protocol.** Newline-delimited JSON over a Unix socket:

- request `{"id", "method", "params"}`, answered by `{"id", "result"}` or
  `{"id", "error": {"code", "message"}}`;
- event `{"event", "data"}`, pushed to connections that subscribed. Events carry
  full snapshots, not diffs.

The first line each way is `{"hello": {"protocol", "build", ...}}`. Its shape
never changes, so any two versions can always tell that they differ. The build
id is the crate version plus a hash of the workspace sources, so every rebuild
changes it.

**Lifecycle.** The client starts `abcd` (the binary next to its own) detached in
its own process group if the socket does not answer. Closing the client never
stops the server. `abcd` holds an exclusive lock per profile, ignores SIGHUP,
and exits cleanly on SIGTERM or `server.shutdown`.

**Mismatch.** At startup, if protocol or build differ, the client asks its
sibling `abcd --print-build-id`. If that differs from the running server, the
server is stale: the client restarts it, at most once. It stops it with
SIGTERM to the pid in the server's hello rather than a request, because an
abcd of another protocol hangs up after its hello and would never hear one. If it matches, the client
itself is stale: it keeps working with a warning when the protocol matches, and
asks to be reopened when it does not. A client that reconnects mid-session never
restarts the server, so two builds never take turns killing each other.

**Profiles.** `--profile <name>` or `ABC_PROFILE`, default `stable`. Config lives
in `$XDG_CONFIG_HOME/abc/<profile>/` and state, logs and the socket in
`$XDG_STATE_HOME/abc/<profile>/`, falling back to `~/.config` and
`~/.local/state`, on macOS too. Profiles share nothing.

## Consequences

- TUIs and the CLI always agree, and integration tests drive the real server
  over the real socket.
- Dev and stable never collide.
- Every feature needs protocol messages, and protocol changes bump the version.
- There is a second process to manage, hence `abc server status|restart|stop`
  and `abc logs`.
- The socket is protected only by its `0700` directory. That is enough while
  there is no remote access.
