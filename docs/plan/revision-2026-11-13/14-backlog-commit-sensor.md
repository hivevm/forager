# Ticket: A sensor lists a commit that adds a backlog entry beside other changes

## What to build

ADR-0005 has a session create a backlog entry in a commit of its own, so a dropped branch can hand
the entry on. Reviews in this scope asked for that split three times as a must-fix (aff2b08,
537b6b5, 82a2435) and once more as lesser (16ce837). In 7f36bf6 a stray `git rm` put code into the
backlog commit. The fold of 2026-11-13 undid the splits: b16f53f, c6e9e1a, 7f36bf6, and 122bb82 now
add or change an entry beside code. Write a sensor, beside `scripts/sensor-refactor-tests.sh`, that
lists every non-merge commit of a range that adds a file under `docs/backlog/` and changes a path
outside it. Run it in `scripts/check-all.sh` and CI as the other sensors run, with its self-test.

## Acceptance criteria

- [ ] Given a fixture whose commit adds `docs/backlog/x.md` and changes `src/a.rs`, when the sensor
  runs over it, then it names that commit and exits 0.
- [ ] Given a commit that adds only the entry, or deletes an entry beside its ticket's work, when
  the sensor runs, then it names nothing.

## Read first

- Touches: `scripts/sensor-backlog-commits.sh`, `scripts/test-sensor-backlog-commits.sh`,
  `scripts/check-all.sh`, `.github/workflows/checks.yml`, `.agents/skills/review/SKILL.md`
- `docs/adr/0005-feature-layer.md`; `scripts/sensor-refactor-tests.sh` and its self-test, the
  pattern to copy; `docs/CONVENTIONS.md`, *A self-test asserts what it builds*.

## Blocked by

- None (can start immediately)

## Mode

unattended: the self-test decides it; a sensor fails nothing, so a wrong one costs one revert. Stop
when the review skill's list of sensors cannot take the line without ticket 19's decision.
