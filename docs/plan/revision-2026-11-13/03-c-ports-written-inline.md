# Ticket: Call ADR-0036's ports where a C conversion or overlap is still written inline

## What to build

ADR-0036 decides that every crate calls C's and MapServer's small ports in `style-ir::c`. Its
*Enforcement* says review catches a port written inline under another name. These sites do so:

- `crates/adapter-geojson/src/lib.rs:62` writes `msRectOverlap` as four comparisons;
  `crates/symbolize/src/shade.rs:140-151` (`meets_image`) does the same, and changes the NaN case
  that `c::rect_overlap` lets through.
- `crates/mapfile/src/translate/mod.rs:856` (a colour channel, `c.trunc() as i32`),
  `mod.rs:266` (`SIZE`, `max(0.0) as u32`), and `crates/mapfile/src/translate/symbol.rs:171`
  (`n as u32`) cast as `as` where C casts with `(int)`.
- `crates/label/src/size.rs:41` writes `MS_NINT` as `round_ties_even`, not through `c::lrint`.
- The alpha of an `OPACITY`, `MS_NINT(opacity * 2.55)`, is written three times and narrowed three
  ways: `crates/mapfile/src/translate/style.rs:244` saturates, `crates/engine/src/classes.rs:359`
  wraps with `rem_euclid(256)`, and `crates/engine/src/lib.rs:479` clamps. Port it once, after
  reading the C of each site at the pin.

## Acceptance criteria

- [ ] Given each site above, when the change lands, then it calls the port of `style-ir::c`, by a
  test of the site that states the value C computes. The `round_ties_even` of
  `crates/render-agg/src/lib.rs:156` and the colour channels' `rem_euclid(256)` of
  `crates/engine/src/classes.rs:603` and `crates/mapfile/src/translate/mod.rs:1083` narrow to a
  byte as AGG's `int8u` does, and stay out of scope.
- [ ] Given an `OPACITY` of 100.4 and of -1, when a style and a layer of it render, then each
  alpha is what MapServer's site computes, by a test that states it.
- [ ] Given a NaN edge, when the geojson adapter and `meets_image` test overlap, then they decide
  as `c::rect_overlap` does, by a test.

## Read first

- Touches: `crates/adapter-geojson/src/lib.rs`, `crates/symbolize/src/shade.rs`,
  `crates/mapfile/src/translate/mod.rs`, `crates/mapfile/src/translate/symbol.rs`,
  `crates/mapfile/src/translate/style.rs`, `crates/label/src/size.rs`,
  `crates/engine/src/classes.rs`, `crates/engine/src/lib.rs`, `crates/style-ir/src/c.rs`
- `docs/adr/0036-c-ports-in-style-ir.md`; `docs/CONVENTIONS.md`, *Behaviour ported from MapServer
  or PROJ is read in their source*.

## Blocked by

- None (can start immediately)

## Mode

unattended: each site's test states the value C computes; a wrong result is one revert. Stop where
the C of a site differs from what this ticket says, and hand off with what the C reads.
