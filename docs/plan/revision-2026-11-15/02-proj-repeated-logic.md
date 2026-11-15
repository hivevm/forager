# Ticket: proj's and the .prj reader's repeated logic held once

## What to build

The reprojection's code writes the same logic or the same constant more than once:

- the test "the pole lies at the origin", reprojecting (0, 0) and comparing its y with ±90 within
  1e-8, three times: `crates/proj/src/shape.rs:226-232`, `crates/proj/src/rect.rs:78-86` and
  `rect.rs:109-117`;
- the wrap by ±360° of `msProjectShapeLine`, twice: `shape.rs:113-118` and `shape.rs:202-207`;
- WGS84's semi-major axis and flattening at `crates/proj/src/params.rs:135-136` and again in the
  table at `params.rs:144`, and its `es` written as a literal at `params.rs:196-197`;
- the middle column of `DATUMS` (`params.rs:150`), a `towgs84` no code reads (`params.rs:178-183`).
  Its doc comment (`params.rs:147-148`) keeps it "as PROJ's table holds it". ADR-0037's *Scope*
  holds `proj` to what a case needs, and a value no code reads is needed by none;
- `Reprojection::shape_line` and the re-export of `ShapeLines` (`crates/proj/src/lib.rs:65`,
  `shape.rs:35,84`) are public, but only `rect.rs` uses them;
- the `.prj` reader ends the C string at its first NUL inline
  (`crates/adapter-shapefile/src/prj.rs:61-62`), which `style_ir::c::c_bytes` ports and
  `dbf.rs` calls (ADR-0017).

Each becomes one function or one constant, the items `pub(crate)`, and the reader calls
`c_bytes`.

## Acceptance criteria

- [ ] Given `crates/proj/src/`, when the change lands, then the comparison of a reprojected
  origin with ±90 is written in one function and the wrap by 360° in one.
- [ ] Given WGS84's parameters, when the change lands, then they are written once and the datum
  table holds no column no code reads.
- [ ] Given the reference test `crates/proj/tests/reference.rs` and the unit tests of `proj` and
  `adapter-shapefile`, when they run, then they pass with no expected value changed.

## Read first

- Touches: `crates/proj/src/shape.rs`, `crates/proj/src/rect.rs`, `crates/proj/src/params.rs`,
  `crates/proj/src/lib.rs`, `crates/adapter-shapefile/src/prj.rs`
- `docs/adr/0017-c-ports-in-style-ir.md`, `docs/adr/0037-reprojection-ported-in-proj.md`.

## Blocked by

- docs/plan/revision-2026-11-15/01-map-projection-held-once.md

## Mode

unattended: the reference records of G-11 and the unit tests decide it; one revert. Stop when an
expected value or a record would move. It touches nothing ADR-0006 keeps with the human, and it
counts against ADR-0006's cap of three open agent branches.
