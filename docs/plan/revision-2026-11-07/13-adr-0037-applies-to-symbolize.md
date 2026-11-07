# Ticket: Let ADR-0037's *Applies to* name `crates/symbolize`

## What to build

[ADR-0037](../../adr/0037-c-number-reading-in-style-ir.md) applies to the ports of C's number
reading "in `crates/style-ir`, `crates/expr`, `crates/mapfile`, and `crates/engine`". 67b3e39
moved the symbolization of a style, with its calls of `ms_nint`, into `crates/symbolize`, so
`scripts/sensor-binding-adrs.sh` no longer lists ADR-0037 for a change there, and the session that
next writes a rounding in `symbolize` is not sent to it. The test of its *Enforcement*
(`crates/arch-test/tests/c_numbers.rs`) covers the whole workspace and is not affected.

An accepted ADR's *Applies to* changes only by a supersession or a normalization
([`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules), rule 5). The normalization that takes up NUC
v0.7.0 restates every ADR in force; carry the change there, or in the revision of ADR-0037 that
`docs/plan/revision-2026-10-31c/03-c-int-cast-once.md` drafts, whichever lands first. Check every
other ADR whose *Applies to* names `crates/engine` for a part 67b3e39 moved.

## Acceptance criteria

- [ ] Given a change to `crates/symbolize/src/draw.rs`, when `scripts/sensor-binding-adrs.sh` runs
  on it, then it lists ADR-0037.
- [ ] Given the ADRs in force whose *Applies to* names `crates/engine`, when they are read against
  the tree, then each part they name lies where they say, or the pull request names those that do
  not.

## Read first

- [ADR-0037](../../adr/0037-c-number-reading-in-style-ir.md),
  [ADR-0028](../../adr/0028-text-stack.md), [ADR-0032](../../adr/0032-hatch-cut-by-i-overlay.md),
  [ADR-0040](../../adr/0040-a-decoded-pixmap-is-held-once-per-render.md); the `normalize` skill;
  `scripts/sensor-binding-adrs.sh`.

## Blocked by

- None (can start immediately)

## Mode

with the human: it rewrites an accepted ADR's header, which only a supersession or a normalization
the maintainer instructs may do.
