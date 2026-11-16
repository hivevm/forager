# Ticket: A marker's size is asked with the canvas's resources, not seven parameters

## What to build

`marker_size_with` (`crates/symbolize/src/symbol.rs`) takes seven parameters. Two of them are an
`Option<&Pixmaps>` and an `Option<&dyn Glyphs>`, the pair the canvas carries, which ADR-0044's
port added. `marker_size_of` only wraps it. Its lookup of a pixmap copies `Canvas::pixmap`, its
error texts included.

The canvas's pixmaps and glyph port become one value. The canvas holds it, and
`marker_size_with` takes it, so the engine's label cache hands it the same value. The lookup of a
pixmap is one function both call. This is a `refactor`.

## Acceptance criteria

- [ ] Given `marker_size_with`, when it is read, then it takes the resources as one parameter,
  and the error texts of a pixmap's lookup ("names no image", "is not loaded for this render")
  are written once in the library code of `symbolize`.
- [ ] Given the label tests of the engine (`marker_size_of` and `marker_size_with`) and the
  msautotest harness, when they run, then all pass unchanged.

## Read first

- Touches: `crates/symbolize/src/{symbol,canvas}.rs`, `crates/engine/src/{labels,label_styles}.rs`
- ADR-0044, ADR-0029.

## Blocked by

- docs/plan/revision-2026-11-16/05-engine-render-resources.md (the engine's side of it)

## Mode

with the human: it changes how `symbolize`'s glyph port (ADR-0044) is handed to the label
cache, and ADR-0006 keeps the port traits from unattended work.
