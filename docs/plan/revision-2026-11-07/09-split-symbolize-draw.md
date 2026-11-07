# Ticket: Split `symbolize`'s `draw.rs` by concern, with a style's outline pass prepared once

## What to build

`crates/symbolize/src/draw.rs` holds some 620 lines of code and 1,030 of tests: the layer's canvas
with its pixmap lookup and refusals (`Canvas`), the dispatch (`draw_ops`), lines, a polygon's
`shade`, markers with pixmaps and the stand-in square, and a style's `GEOMTRANSFORM`. The style of
an outline pass is prepared twice, word for word, in `line_symbol` and `transformed`. `Canvas`'s
`pub(crate)` fields are read only inside `draw.rs`.

Split it into modules by concern, the canvas, lines, and markers at least, keeping the direction
of imports the overview describes (`draw` on top, the leaves importing nothing of the crate), and
write the outline pass's preparation once. A refactor: nothing a test can see moves.

## Acceptance criteria

- [ ] Given the tests of `symbolize` and `engine`, when they run after the split, then they pass
  unchanged, moved with the code they test.
- [ ] Given the code, when it is read, then the preparation of an outline pass is written once and
  no module imports one above it.

## Read first

- [ADR-0009](../../adr/0009-ports-and-adapters-crates.md); the `symbolize` bullet of
  [`docs/ARCHITECTURE.md`](../../ARCHITECTURE.md); `crates/symbolize/src/draw.rs`;
  `docs/plan/revision-2026-11-07/10-line-pass-conditions-in-transformed.md`, which reads the same
  passes.

## Blocked by

- docs/plan/revision-2026-11-07/10-line-pass-conditions-in-transformed.md

## Mode

unattended: the unchanged tests decide it; its scope is one crate, and a wrong result costs one
revert; it touches none of the parts [ADR-0007](../../adr/0007-harness.md) keeps with the
maintainer and counts against its cap of three; stop: when a test has to change, the session
records why here and hands off.
