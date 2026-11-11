# MapServer's MS_MIN, MS_MAX, MS_DEG_TO_RAD, and C's string end are ported more than once

Observed: ADR-0017 ports C's reading of numbers once, in `forager_style_ir::c`, so that a port
that differs from the C does so in one place. Other small ports of mapserver.h and of C are
written more than once:

- `MS_MIN` and `MS_MAX`, which keep their second operand where a comparison fails (NaN): `ms_min`
  and `ms_max` in crates/symbolize/src/bounds.rs, in crates/label/src/cache.rs, and a third copy
  in the test crates/label/tests/cache_pairwise.rs; written inline in crates/label/src/size.rs
  (`glyph_size`) and, as `MS_MAX`, in crates/engine/src/labels.rs (the offset from a label's
  marker).
- `MS_DEG_TO_RAD`: `DEG_TO_RAD` in crates/symbolize/src/style.rs, which the engine imports to
  turn a label, and `PI / 180.0` in crates/label/src/place.rs. In `f64` the two are the same
  value, `0.017453292519943295`, so no value moves; only the second definition goes.
- A C string's end at its first NUL: `c_bytes` and `c_str` in crates/engine/src/classes.rs,
  beside the selection of classes, and `c_string` in crates/expr/src/lib.rs.

The design revision that closed geometry-styles found two more of the kind: `msStringSplit`, in
crates/engine/src/classes.rs and in crates/mapfile/src/translate/feature.rs, and `msRectOverlap`,
in crates/engine/src/inline.rs and crates/adapter-shapefile/src/record.rs.

Each would move into one place that every crate needing it may depend on, the sites calling it,
with no test or reference image changed. style-ir's `c` module is the obvious place: symbolize,
engine, and expr depend on style-ir already; label does not, and ADR-0008 permits the edge. The
open question is the maintainer's: do these ports go into style-ir's `c` module, which grows what
ADR-0017 places there, or into another place? Its ADR part goes with the change of ADR-0017 the
backlog entry c-int-cast-once.md describes.

Where: crates/style-ir/src/c.rs, crates/symbolize/src/bounds.rs, crates/symbolize/src/style.rs,
crates/label/src/cache.rs, crates/label/src/size.rs, crates/label/src/place.rs,
crates/label/tests/cache_pairwise.rs, crates/engine/src/labels.rs, crates/engine/src/classes.rs,
crates/expr/src/lib.rs, crates/engine/src/inline.rs,
crates/mapfile/src/translate/feature.rs, crates/adapter-shapefile/src/record.rs,
crates/label/Cargo.toml, crates/arch-test/tests/dependencies.rs; ADR-0008,
ADR-0017; src/mapserver.h at the pin.
