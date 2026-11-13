# Ticket: Port C's `(int)` cast of a `double` once, and refuse the bare `as` in its place

## What to build

MapServer converts a `double` to an `int` with a C cast in many places Forager ports, and Rust's
`as` saturates where the x86-64 build at the pin gives `INT_MIN` (`cvttsd2si`). #153 first held
`MAXLENGTH` with a saturating cast; review caught it. The spec `labels-2` then wrote the conversion
as `int_cast`, into which #154 merged a `trunc_int`; 10f136a, which landed
`revision-2026-10-31/10-label-binding-stage`, moved it to `crates/engine/src/label_binding.rs`.
[ADR-0017](../../adr/0017-c-numbers-in-style-ir.md), accepted (6eaa200), ports C's `atof`,
`strtod`, `atoi`, `lrint`, and `MS_NINT` once, in `forager_style_ir::c` (`lrint` gives `LONG_MIN`
past a `long`, `ms_nint` its low 32 bits), but names no `(int)` cast.

Bare casts and truncations that stand for a C `(int)` remain:

- `crates/engine/src/labels.rs`: a label's scaled `OFFSET` in `Labels::matched`, and its scaled
  `BUFFER` in `Labels::def`.
- `crates/engine/src/label_binding.rs`: a label's `OFFSET`, `number(o, 0) as i32`, in `bind`.
- `crates/engine/src/lib.rs`: a layer's `OPACITY`, in `layer_opacity`.
- `crates/label/src/place.rs`: the offset from a marker, in `placed_with_buffer`.
- `crates/label/src/layout.rs`: a line's length, `pen_x as i32`, in `Layout::line`; and the line
  height, an `int` at the pin, as `as i64` in `Layout::new`.
- `crates/expr/src/lib.rs`: `modulo`, C's `(int)a % (int)b`, written inline with a range check
  that makes an operand past an `int` undefined.
- `crates/mapfile/src/translate/mod.rs`: a layer's `MAXFEATURES`, truncated to an `int`, as
  `n as i32` in `max_features` (44cc731).
- `crates/symbolize/src/hatch.rs`: the size of a hatch's box, MapServer's `(int)`, as `.trunc()`
  in `hatch_polygon`.
- `crates/symbolize/src/geom_transform.rs`: the padding of `bbox`, in `bbox` (5c17154), which
  writes the pin's conversion inline, as `int_cast` does, with the `int` sums around it wrapping.
- `crates/symbolize/src/symbol.rs`: a marker's size, C's assignment to an `int`, as `.trunc()` in
  `marker_size_at` and in `pixmap_marker_size` (a26a539); and AGG's `uround`,
  `unsigned(v + 0.5)`, as `as u32` in `agg_ellipse`.
- `crates/symbolize/src/bounds.rs`: a marker's size held by `MINSIZE` and `MAXSIZE`, an `int` at
  the pin, as `.trunc()` in `marker_pixels`.

Each diverges from the pin only past an `int`, which no corpus case reaches, so only a check would
see the next one.

Inline roundings that the test of ADR-0017's *Enforcement* (`crates/arch-test/tests/c_numbers.rs`)
cannot see, as it refuses only functions by name, are sites to judge here too, each against the C
it follows and `forager_style_ir::c::{lrint, ms_nint}`: the clip buffer of a hatch
(`round_ties_even` in `crates/symbolize/src/hatch.rs`), the glyph size and the pixel size
(`glyph_size` and `pixel_size` in `crates/label/src/size.rs`), and the dash lengths of the AGG
renderer (`add_dashes` in `crates/render-agg/src/lib.rs`), which are clamped on purpose (Q-3,
Q-9), as a tile's side is (`bounded_side` in `crates/symbolize/src/tile.rs`).

The normalization of #174 (bf348ee) restated ADR-0017 with neither the `(int)` cast nor the
change of *Applies to* below, and a normalization changes no decision. ADR-0017 is accepted, so
it changes only by a supersession ([`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules), rule 5). One
change carries every part below (triage of 2026-11-07, on the maintainer's word). Draft that
superseding ADR. Once it is settled, move `int_cast` and each site onto the port, and add the
check: for example clippy's `cast_possible_truncation` in `engine`, `label`, and `symbolize`, with
the index casts it also names written with `try_from`
([ADR-0007](../../adr/0007-rust-toolchain.md)), or a narrower test of ADR-0017's *Enforcement*.
In the library code of those three crates the lint names some 33 casts today. The loop counts
bounded on purpose (Q-9), such as `steps` in `crates/symbolize/src/hatch.rs` and `place` in
`crates/symbolize/src/line_marker.rs`, take an `#[expect]` with their reason, as
`crates/render/src/hatch.rs` and `crates/render-agg/src/lib.rs` already do.
`crates/symbolize/src/geom_transform.rs` writes `#[allow]` for the lint, which is not enabled.

`crates/label` does not depend on `style-ir`, so a port in `forager_style_ir::c` gives `label` a
new edge for `place.rs` and `layout.rs`; [ADR-0008](../../adr/0008-ports-and-adapters-crates.md)
permits it.

The same change carries the ADR part of
`docs/plan/revision-2026-11-07/08-mapserver-macros-ported-once.md`: whether ADR-0017's place
grows to hold `MS_MIN`, `MS_MAX`, `MS_DEG_TO_RAD`, `msRectOverlap`, `msStringSplit`, and a C
string's end. That ticket keeps the code. The two overlap at three sites: the padding of `bbox`
converts an `MS_MAX` with the cast, and `glyph_size` and `placed_with_buffer` are sites of both.
Whichever lands second takes the other's port there. Both tickets face the same new edge from
`label` to `style-ir`.

## Acceptance criteria

- [ ] Given the drafted superseding ADR of ADR-0017, when the maintainer reads it, then it names
  the `(int)` cast and the behaviour it gives past an `int`, and answers the placement of
  `revision-2026-11-07/08`, in an ADR-only pull request.
- [ ] Given a label whose scaled `OFFSET` lies past an `int` and a `MAXLENGTH` of 3e9, when they
  are drawn, then both convert as the one port does, each with a test that states its value.
  (`MAXLENGTH` of 3e9 has a test through `int_cast` today, in `crates/engine/tests/labels.rs`.)
- [ ] Given a new `f64 as i32` in `crates/engine`, `crates/label`, or `crates/symbolize`, when the
  checks run, then they fail and name it.

## Taken over from `docs/plan/revision-2026-11-07/13-adr-0037-applies-to-symbolize.md`

The triage of 2026-11-07 (bd2ff53) merged that ticket into this one.
[ADR-0017](../../adr/0017-c-numbers-in-style-ir.md) applies to the ports of C's number reading
"in `crates/style-ir`, `crates/expr`, `crates/mapfile`, and `crates/engine`". 1eec7a6 moved the
symbolization of a style, with its calls of `ms_nint`, into `crates/symbolize`, so
`scripts/sensor-binding-adrs.sh` does not list ADR-0017 for a change there, and the session that
next writes a rounding in `symbolize` is not sent to it. The test of its *Enforcement* covers the
whole workspace and is not affected. Carry the change of its *Applies to* in the same
supersession. Check every other ADR in force whose *Applies to* names `crates/engine` for a part
1eec7a6 moved: ADR-0016, ADR-0018, ADR-0022, ADR-0029, ADR-0031, and ADR-0032 name it today.

Its criteria, now this ticket's as well:

- [ ] Given a change to `crates/symbolize/src/draw.rs`, when `scripts/sensor-binding-adrs.sh` runs
  on it, then it lists ADR-0017.
- [ ] Given the ADRs in force whose *Applies to* names `crates/engine`, when they are read against
  the tree, then each part they name lies where they say, or the pull request names those that do
  not.

## Read first

- Touches: `docs/adr/`, `crates/style-ir/src/c.rs`, `crates/engine/src/label_binding.rs`,
  `crates/engine/src/labels.rs`, `crates/engine/src/lib.rs`, `crates/label/src/place.rs`,
  `crates/label/src/layout.rs`, `crates/label/Cargo.toml`, `crates/expr/src/lib.rs`,
  `crates/mapfile/src/translate/mod.rs`, `crates/symbolize/src/hatch.rs`,
  `crates/symbolize/src/geom_transform.rs`, `crates/symbolize/src/symbol.rs`,
  `crates/symbolize/src/bounds.rs`, `crates/arch-test/tests/c_numbers.rs`, `Cargo.toml`.
- [ADR-0017](../../adr/0017-c-numbers-in-style-ir.md),
  [ADR-0007](../../adr/0007-rust-toolchain.md) (the lints of the workspace),
  [ADR-0008](../../adr/0008-ports-and-adapters-crates.md); `int_cast` in
  `crates/engine/src/label_binding.rs`, `lrint` and `ms_nint` in `crates/style-ir/src/c.rs`; #153
  and #154's doubts.
- `docs/plan/revision-2026-11-07/08-mapserver-macros-ported-once.md`; the skill `propose-adr`.

## Blocked by

- None (can start immediately)

## Mode

with the human: it drafts the supersession of an accepted ADR, which only the maintainer decides
([ADR-0006](../../adr/0006-harness.md)), and adds a lint to the toolchain ADR's set. The question
first: does the `(int)` cast, with the placement of `revision-2026-11-07/08`, go into the ADR that
supersedes ADR-0017 now, in `forager_style_ir::c`, with the new edge from `label` to `style-ir`?
