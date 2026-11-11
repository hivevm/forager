# C's (int) cast of a double is ported nowhere, and a bare `as` stands in its place

Observed: MapServer converts a `double` to an `int` with a C cast in many places Forager ports,
and Rust's `as` saturates where the x86-64 build at the pin gives `INT_MIN` (`cvttsd2si`). #153
first held `MAXLENGTH` with a saturating cast; review caught it. The conversion is written once as
`int_cast` in crates/engine/src/labels.rs, into which #154 merged a `trunc_int`. ADR-0017 ports
C's `atof`, `strtod`, `atoi`, `lrint`, and `MS_NINT` once, in `forager_style_ir::c`, but names no
`(int)` cast. Bare casts that stand for a C `(int)` remain:

- crates/engine/src/labels.rs: a label's scaled `OFFSET` in `Labels::new` and in `bind`, and its
  scaled `BUFFER` in `def`; docs/plan/revision-2026-10-31/10-label-binding-stage.md moves `bind`
  into a module of its own.
- crates/label/src/place.rs: the offset from a marker, in `placed_with_buffer`.
- crates/label/src/layout.rs: a line's length, `pen_x as i32`.
- crates/engine/src/lib.rs: a layer's `OPACITY`, in `layer_opacity`.
- crates/expr/src/lib.rs: `modulo`, C's `(int)a % (int)b`, written inline with a range check that
  makes an operand past an `int` undefined.
- crates/symbolize/src/hatch.rs: the size of a hatch's box, MapServer's `(int)`, as `.trunc()` in
  `hatch_polygon`.
- crates/symbolize/src/symbol.rs: a marker's size, C's assignment to an `int`, as `.trunc()` in
  `marker_size_at`; and AGG's `uround`, `unsigned(v + 0.5)`, as `as u32` in `agg_ellipse`.

Each diverges from the pin only past an `int`, which no corpus case reaches, so only a check
would see the next one. Inline roundings that the test of ADR-0017's Enforcement
(crates/arch-test/tests/c_numbers.rs) cannot see, as it refuses only functions by name, are sites
to judge too, each against the C it follows and `forager_style_ir::c::{lrint, ms_nint}`: the clip
buffer of a hatch (`round_ties_even` in crates/symbolize/src/hatch.rs), the glyph size and the
pixel size (`glyph_size` and `pixel_size` in crates/label/src/size.rs), and the dash lengths of
the AGG renderer (`add_dashes` in crates/render-agg/src/lib.rs), which are clamped on purpose
(Q-3, Q-9).

Also observed: ADR-0017 applies to the ports "in crates/style-ir, crates/expr, crates/mapfile, and
crates/engine". 67b3e39 moved the symbolization of a style, with its calls of `ms_nint`, into
crates/symbolize, so scripts/sensor-binding-adrs.sh does not list ADR-0017 for a change there,
and the session that next writes a rounding in symbolize is not sent to it. The test of its
Enforcement covers the whole workspace and is not affected. Other ADRs in force whose Applies to
names crates/engine may name a part 67b3e39 moved. The normalization of #174 restated ADR-0017
without either change.

The `(int)` cast is a new decision, so it changes ADR-0017 by a supersession, with the Applies to
and the placement question of mapserver-macros-ported-once.md in the same change (triage of
2026-11-07, on the maintainer's word); the maintainer decides it. Once settled, `int_cast` and
each site move onto the port, and a check refuses a new `f64 as i32` in engine, label, and
symbolize: for example clippy's `cast_possible_truncation`, with the index casts it also names
written with `try_from` (ADR-0007), or a narrower test of ADR-0017's Enforcement.

Where: crates/engine/src/labels.rs (`int_cast`), crates/style-ir/src/c.rs (`lrint`, `ms_nint`),
the sites above, crates/arch-test/tests/c_numbers.rs; docs/adr/0017-c-numbers-in-style-ir.md
(Applies to), ADR-0007, ADR-0008; #153 and #154's doubts.
