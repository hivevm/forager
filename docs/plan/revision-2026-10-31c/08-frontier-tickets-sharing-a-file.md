# Ticket: Let the frontier sensor name the tickets that would edit the same file

## What to build

The spec `labels-2` ran tickets 01, 02, and 04 in parallel, each editing
`crates/engine/src/labels.rs`, and they conflicted there three times; #155 merged `main` in twice.
The frontier lists tickets a session may claim, and nothing tells the dispatcher that two of them
change the same module. Eleven open tickets name `labels.rs` today. A ticket names the files it
changes only in prose (its *What to build* and *Read first*), and the plan forbids paths in a
spec, not in a ticket. `scripts/sensor-plan-frontier.sh` could report, beside the frontier, each
pair of frontier tickets that name the same source path, so that a dispatcher chains them or
claims one at a time; the report fails nothing. The sensor belongs to the template
([ADR-0006](../../adr/0006-feature-layer.md)), so the change is raised there first.

## Acceptance criteria

- [ ] Given a fixture plan with two frontier tickets that name `crates/x/src/a.rs`, when the
  sensor runs, then it names the two tickets and the path, and its exit code is unchanged.
- [ ] Given the drafted change, when the template's maintainer reads it, then it is raised
  upstream, and this ticket's file is deleted with the release that carries it.

## Read first

- ADR-0006, [ADR-0007](../../adr/0007-harness.md) (the cap on parallel work);
  `scripts/sensor-plan-frontier.sh` and its self-test; the skill `work-tickets`.

## Blocked by

- None (can start immediately)

## Mode

with the human: the sensor and the skills are the template's, changed upstream by its maintainer.
