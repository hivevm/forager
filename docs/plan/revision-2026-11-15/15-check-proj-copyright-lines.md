# Ticket: A check that each file porting PROJ carries that source's copyright lines

## What to build

Review 01 of the reprojection found the per-file copyright lines of PROJ's sources missing
(`cart.cpp`, `helmert.cpp`, `4D_api.cpp` and others), with headers that claimed there were none.
d6cdd093 fixed it by hand. `scripts/check-notice.sh` checks only PROJ's `COPYING` text in
`THIRD-PARTY-NOTICES.md`. The reviewer offered a check per file and it was not taken. A mistake a
check could have caught becomes a check
([`AGENTS.md` §5](../../../AGENTS.md#5-quality-bar--definition-of-done)). Extend
`check-notice.sh` with a table. It maps each PROJ source that a file of `crates/proj/src/` names
as ported to the copyright lines that source holds at PROJ 9.4.0. The check fails when a file
names a source the table lacks, or lacks a line the table gives.

## Acceptance criteria

- [ ] Given the tree as it is, when `scripts/check-notice.sh` runs, then it passes.
- [ ] Given a fixture where a file of `proj` names a PROJ source without its copyright line, when
  `scripts/test-check-notice.sh` runs, then the check fails and names the file and the line.
- [ ] Given a fixture where a file names a source the table lacks, when the self-test runs, then
  the check fails and names the source.

## Read first

- Touches: `scripts/check-notice.sh`, `scripts/test-check-notice.sh`, `scripts/lib/notice.sh`
- `docs/adr/0037-reprojection-ported-in-proj.md` (*Reference*),
  `docs/adr/0033-noise-measurement.md`; `docs/CONVENTIONS.md`, *A test that expects a refusal
  asserts its reason*.

## Blocked by

- None (can start immediately)

## Mode

with the human: the check is about the licences, which ADR-0006 never leaves unattended. The
self-test decides it. Stop when PROJ 9.4.0's sources cannot be read for the table.
