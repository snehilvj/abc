# 0007: Following changes on disk

- Status: accepted
- Date: 2026-10-28

## Context

Files change while abc is open: you edit them, or an agent does. The
explorer listed a worktree's files once, when it opened, and the viewer read
a file once, so both went stale until abc restarted.

## Decision

**Each connection says what it has open.** A `watch` request names a
worktree and the file in its viewer; abc sends it whenever either changes,
and again after reconnecting. abcd follows that, for that connection only:

- the file, every second, by its modified time and size: `file.changed` when
  they differ from when abcd last read it for the connection, a file that
  went included;
- the worktree's files, every `git.status_interval`: `files.changed`, with
  every file, when they differ from what abcd last sent the connection.

abcd remembers what each connection was sent, so a change made while abc
looked at another worktree or file is said as soon as abc looks back.

**abc keeps its place.** The explorer rebuilds its tree from the new list and
keeps its open folders, selection and scroll. The viewer reads the file again
and swaps the new text in once it's coloured, so it never flashes plain; the
cursor and view stay where they were, within the new text.

**Polling, not file events.** Looking at one file a second and listing one
worktree every few seconds is cheap, works the same on macOS and Linux, and
needs no crate beyond those allowed. File events (inotify, FSEvents) would
need one, and differ per platform.

## Consequences

- A change to the open file shows within about a second, and new or deleted
  files within `git.status_interval`.
- Listing a very large worktree every few seconds costs some CPU while abc
  is open on it; raising `git.status_interval` trades freshness for less.
- Only the open worktree and file are followed: another worktree catches up
  when you open it.
