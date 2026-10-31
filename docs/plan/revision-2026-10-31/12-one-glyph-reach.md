# Ticket: Hold a glyph's reach past the image by one predicate in `label` and `render-agg`

## What to build

`crates/label/src/layout.rs` keeps a glyph whose box lies within the image grown by twice the
outline and `MAX_GLYPH_REACH`; `crates/render-agg/src/glyphs.rs` requires every point of a label's
path within the larger side of the image and `MAX_GLYPH_REACH`, and drops the whole label, every
glyph of it, without a diagnostic when one point is past it. A glyph with an outline between the two
passes the first and silently takes its label with it in the second. `MAX_GLYPH_REACH` is agg-rust's
fixed-point limit, kept in the backend-neutral `render`.
[ADR-0015](../../adr/0015-agg-rust-rasterizer.md) leaves text out of its clipping and
[ADR-0028](../../adr/0028-text-stack.md) says nothing of this safeguard. One predicate in `render`
used by both crates, and a label the renderer cannot draw left out by `label` with the engine's
diagnostic.

## Acceptance criteria

- [ ] Given a label with an outline of 3 whose glyph lies between the two reaches today, when it is
  drawn, then either both crates keep it or `label` leaves it out with a diagnostic; the renderer
  never drops a label silently.
- [ ] Given the label tests, when they run, then they pass unchanged.

## Read first

- [ADR-0015](../../adr/0015-agg-rust-rasterizer.md), [ADR-0028](../../adr/0028-text-stack.md);
  `crates/label/src/layout.rs`; `crates/render-agg/src/glyphs.rs`; `MAX_GLYPH_REACH` in
  `crates/render/src/lib.rs`.

## Blocked by

- None (can start immediately)

## Mode

unattended: a test at each side of the reach decides its criteria; its scope is the glyph path of
`label`, `render`, and `render-agg`, and a wrong result costs one revert; it touches none of the
parts [ADR-0007](../../adr/0007-harness.md) keeps with the maintainer and counts against its cap of
three; a wrong result shows as a label left out or a failing label test; stop: after two attempts
that leave `scripts/check-all.sh` red, or when the predicate cannot be one without moving a
reference image, the session leaves its state in this file and hands off.
