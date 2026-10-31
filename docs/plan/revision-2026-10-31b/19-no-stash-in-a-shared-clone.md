# Ticket: Raise with the template that a session never uses `git stash` in a clone it shares

## What to build

The worker of #136 used a bare `git stash` and `git stash pop` in its worktree. Every worktree of a
clone shares one stash stack, and the harness runs up to three agent sessions in worktrees of one
clone at once ([ADR-0007](../../adr/0007-harness.md)), beside the maintainer's own: a bare `pop`
takes the youngest entry, which may be another session's, and applies it to the wrong branch. It
restored only its own files that time. No check sees a stash, and the hooks cannot refuse one.
[`AGENTS.md` §6](../../../AGENTS.md#6-project-rules) lists what rewrites or discards history and
waits for an instruction (`reset`, `rebase`, `merge`, `restore`, `clean`); a stash does neither but
moves work between branches unseen. `AGENTS.md` and the skill `work-tickets` are the template's
([ADR-0005](../../adr/0005-template-releases.md)), so the rule, if any, belongs there first: for
example "a session that shares a clone keeps its work on its branch, by a commit, never in the
stash", in `work-tickets`' worktree step, or `stash` beside the actions that section names. Draft
the change for the template's maintainer and take it up with a template release.

## Acceptance criteria

- [ ] Given the drafted wording, when the maintainer reads it, then it names where in the template
  it stands and why, and it is raised upstream.
- [ ] Given the template release that carries it, when it is taken up, then this ticket's file is
  deleted with it.

## Read first

- [`AGENTS.md` §6](../../../AGENTS.md#6-project-rules); the skill `work-tickets`;
  [ADR-0005](../../adr/0005-template-releases.md), ADR-0007; #136's last doubt
  (`gh pr view 136`).

## Blocked by

- None (can start immediately)

## Mode

with the human: `AGENTS.md` and the skills are the template's, changed upstream by its maintainer.
