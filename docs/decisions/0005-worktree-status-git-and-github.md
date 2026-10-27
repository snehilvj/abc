# 0005: Worktree status from git and GitHub

- Status: accepted
- Date: 2026-10-28

## Context

Each worktree shows its folder name, branch, uncommitted changes, how far it is
ahead of or behind its upstream, and the state of its pull request: open,
draft, merged or closed, with its checks and review decision. GitHub was out of
scope in the original brief; PR status is now in slice 1. PR review is not.

## Decision

abcd computes all status; clients only display what it sends.

**git.** `git status --porcelain=v2 --branch -z` per worktree gives the branch,
upstream, ahead/behind counts and changed files in one call. abcd runs it every
`git.status_interval` (default 5 s) while at least one client is subscribed,
and pushes a change event only when something differs.

**GitHub.** The `gh` CLI, which already holds your login:
`gh pr list --state all --limit 100 --json number,title,state,isDraft,headRefName,statusCheckRollup,reviewDecision,url`
per repository, every `github.pr_interval` (default 60 s). Each worktree takes
the most recently updated PR whose head branch matches its branch. abc stores no
GitHub token.

- If `gh` is missing or logged out, PR status turns off and the status line says
  so once. `github.enabled = false` turns it off on purpose.
- A failed or slow call keeps the last known state; PR status is never a reason
  to block the UI.

## Consequences

- No authentication code in abc, and it keeps working offline with the last
  known state.
- abc depends on the `gh` CLI and the JSON fields it emits.
- One `git status` per worktree per interval. Very large repos can raise the
  interval.
- Matching by branch name can pick the wrong PR when a fork uses the same
  branch name; abcd prefers PRs whose head repository is the worktree's own.
