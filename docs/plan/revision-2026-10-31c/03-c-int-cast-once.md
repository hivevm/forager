# Ticket: Port C's `(int)` cast of a `double` once, and refuse the bare `as` in its place

## What to build

MapServer converts a `double` to an `int` with a C cast in many places Forager ports, and Rust's
`as` saturates where the x86-64 build at the pin gives `INT_MIN` (`cvttsd2si`). #153 first held
`MAXLENGTH` with a saturating cast; review caught it. The spec `labels-2` then wrote the conversion
in `crates/engine/src/labels.rs` as `int_cast`, into which #154 merged a `trunc_int`.
[ADR-0037](../../adr/0037-c-number-reading-in-style-ir.md), proposed, ports C's `atof`, `strtod`,
`atoi`, `lrint`, and `MS_NINT` once, in `forager_style_ir::c` (`lrint` gives `LONG_MIN` past a
`long`, `ms_nint` its low 32 bits), but names no `(int)` cast.

Bare casts that stand for a C `(int)` remain: a label's scaled `OFFSET` and `BUFFER` (`labels.rs`,
in `feature`, `def`, and `bind`, the last of which
`docs/plan/revision-2026-10-31/10-label-binding-stage.md` moves into a module of its own), the
offset from a marker (`crates/label/src/place.rs`, `placed_with_buffer`), a line's length
(`crates/label/src/layout.rs`, `pen_x as i32`), and a layer's `OPACITY`
(`crates/engine/src/lib.rs`). Each diverges from the pin only past an `int`, which no corpus case
reaches, so only a check would see the next one.

Inline roundings that the test of ADR-0037's *Enforcement* cannot see, as it refuses only
functions by name, are sites to judge here too, each against the C it follows and
`forager_style_ir::c::{lrint, ms_nint}`: the clip buffer of a hatch
(`crates/symbolize/src/hatch.rs:73`), the glyph size and the pixel size
(`crates/label/src/size.rs:34` and `:40`), and the dash lengths of the AGG renderer
(`crates/render-agg/src/lib.rs:151`), which are clamped on purpose (Q-3, Q-9).

The revision of 2026-11-07 found further sites written inline: a range-checked `(int)` in
`modulo` (`crates/expr/src/lib.rs`), `.trunc()` standing for an `(int)` in
`crates/symbolize/src/hatch.rs` and for C's assignment to an `int` in
`crates/symbolize/src/symbol.rs`, and AGG's `uround` written `as u32` in `symbol.rs`. Its ticket
`docs/plan/revision-2026-11-07/08-mapserver-macros-ported-once.md` places `MS_MIN`, `MS_MAX`, and
`MS_DEG_TO_RAD` in the same module; fold the two drafts of ADR-0037 into one where both are open.

Draft for the maintainer a revision of ADR-0037 that adds the `(int)` cast to the ports it places,
with the sites above; once it is settled, move `int_cast` and each site onto it, and add the check:
for example clippy's `cast_possible_truncation` in `engine` and `label`, with the index casts it
also names written with `try_from`, or a narrower test of ADR-0037's *Enforcement*.

## Acceptance criteria

- [ ] Given the drafted revision of ADR-0037, when the maintainer reads it, then it names the
  `(int)` cast and the behaviour it gives past an `int`, in an ADR-only pull request.
- [ ] Given a label whose scaled `OFFSET` lies past an `int` and a `MAXLENGTH` of 3e9, when they
  are drawn, then both convert as the one port does, each with a test that states its value.
- [ ] Given a new `f64 as i32` in `crates/engine` or `crates/label`, when the checks run, then
  they fail and name it.

## Read first

- [ADR-0037](../../adr/0037-c-number-reading-in-style-ir.md),
  [ADR-0008](../../adr/0008-rust-toolchain.md) (the lints of the workspace),
  [ADR-0009](../../adr/0009-ports-and-adapters-crates.md); `int_cast` in
  `crates/engine/src/labels.rs`, `lrint` and `ms_nint` in `crates/style-ir/src/c.rs`; #153 and
  #154's doubts (`gh pr view 153`, `gh pr view 154`).

## Blocked by

- None (can start immediately)

## Mode

with the human: it revises a proposed ADR, whose open question on the behaviour past a `long` the
maintainer answers first ([`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules), rule 4), and adds a
lint to the toolchain ADR's set.
