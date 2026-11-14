# Ticket: Refuse f64::min and f64::max in the crates that port MapServer's drawing

## What to build

The convention *A minimum or maximum over values that may be NaN decides the NaN first* is checked
by nothing. `engine`, `label`, and `symbolize` call `f64::min` and `f64::max` at about 39 sites
outside their tests. Ticket 04 names three of them that may stand for `MS_MIN` and `MS_MAX`. Make
the convention a check: a `disallowed-methods` entry for `f64::min` and `f64::max`. The workspace's
`clippy.toml` holds one list, which every crate that warns of `disallowed_methods` takes (Q-2's
crates), so the entry reaches beyond the three crates; scope it, or convert or excuse the sites of
every such crate, each remaining site either calling `ms_min` or `ms_max` or carrying an `#[expect]`
with the reason the NaN cannot reach it. Then shorten the convention to what the lint cannot see.

## Acceptance criteria

- [ ] Given a new `f64::max` in `crates/symbolize/src`, when `scripts/check-rust.sh` runs, then it
  fails naming the site.
- [ ] Given the suite at the base, when every site is converted or excused, then it passes
  unchanged.

## Read first

- Touches: `clippy.toml`, `crates/engine/src/lib.rs`, `crates/label/src/lib.rs`,
  `crates/symbolize/src/lib.rs`, the sites the lint names, `docs/CONVENTIONS.md`
- `docs/adr/0036-c-ports-in-style-ir.md`, its *The lint* clause for how a lint of three crates is
  turned on; `docs/adr/0007-rust-toolchain.md`.

## Blocked by

- docs/plan/revision-2026-11-13/04-compute-bounds-once.md

## Mode

unattended: the lint and the unchanged suite decide it; one revert. Stop when a site's change moves
a test's expected value, or when the lint cannot be scoped to the three crates without an ADR.
