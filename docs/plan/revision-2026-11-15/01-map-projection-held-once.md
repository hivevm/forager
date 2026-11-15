# Ticket: A map's projection held once, and decided in proj alone

## What to build

A map's projection is two fields of the style IR that can contradict each other, and what they
mean is decided in two crates:

- `crates/style-ir/src/model.rs:34-45` holds `projection: Option<String>` beside
  `projection_auto: bool`, whose doc comment has to say that `projection` is not read when it is
  true; a layer's projection is already one enum, `LayerProjection`.
- `crates/engine/src/reproject.rs:39-47,66-92` rebuilds the missing enum as a local `Side`, and its
  `empty` closure (`reproject.rs:74-78`) checks again for an empty projection, which
  `proj::differ` (`crates/proj/src/lib.rs:311-315`) checks already.
- `proj` decides a map of `AUTO` by `into_auto` and the flags `from_geographic` and
  `to_geographic` (`lib.rs:319-350`), and `rect.rs:90-103` infers "a map of AUTO" again from
  `is_geographic() && !geographic`.

The map's projection becomes one enum in the model (none, a proj4 string, or `AUTO`), written to
and read from format 1.1's wire shape as today through serde. `proj` takes the two sides as that
kind of value through one entry point, which decides whether they differ, the base geographic CRS
of a map of `AUTO`, and the shortcut into longitude and latitude. The engine calls it and keeps no
`Side` of its own.

## Acceptance criteria

- [ ] Given every style document the tests read, when the change lands, then each serializes byte
  for byte as before and `schema/style-document-1.1.json` is unchanged.
- [ ] Given the engine's tests of `crates/engine/tests/reproject.rs`, when they run, then every
  expected value holds unchanged.
- [ ] Given `crates/engine/src/reproject.rs`, when the change lands, then it holds no enum or
  closure that decides whether a projection is empty or `AUTO`.

## Read first

- Touches: `crates/style-ir/src/model.rs`, `crates/style-ir/src/document.rs`,
  `crates/mapfile/src/translate/mod.rs`, `crates/proj/src/lib.rs`, `crates/proj/src/rect.rs`,
  `crates/engine/src/reproject.rs`
- `docs/adr/0013-style-document-serde.md`, `docs/adr/0014-style-document-versions.md`,
  `docs/adr/0037-reprojection-ported-in-proj.md`; `docs/semantics/projection.md`.

## Blocked by

- None (can start immediately)

## Mode

unattended: the schema test, the round trips of G-3, and the engine's tests decide it; one revert.
Stop when the schema file or a serialized document would change, or when a test's expected value
moves.
