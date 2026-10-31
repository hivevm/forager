# Ticket: Decide a line layer's passes and its shape cache in one type

## What to build

`draw::passes` takes a slice of flags, `bound_outline`, which `Drawing::draw` in
`crates/engine/src/lib.rs` rebuilds for every feature from the unbound class, while
`Drawing::cached` reads `Bound::Binding` on the outline width again: two files encode the one rule
of `msDrawVectorLayer`, which styles are written and which bound, and the exception of a first pass
under `GEOMTRANSFORM` is written in both. Build one type per class from its written styles that
answers whether the layer's shapes are cached and gives the passes, so that `passes` takes no flags.

## Acceptance criteria

- [ ] Given a line layer whose outline width is bound, and one whose first style has a
  `GEOMTRANSFORM`, when they are drawn, then the display list is what it is today, the cases of
  `crates/engine/tests/render.rs` unchanged.
- [ ] Given `draw::passes`, when its signature is read, then it takes no slice of flags.

## Read first

- `passes` in `crates/engine/src/draw.rs`; `Drawing::draw` and `Drawing::cached` in
  `crates/engine/src/lib.rs`; `msDrawVectorLayer` at the pin.

## Blocked by

- None (can start immediately)

## Mode

unattended: the existing display-list tests and reference images decides its criteria; its scope is
the engine's line layers, and a wrong result costs one revert; it touches none of the parts
[ADR-0007](../../adr/0007-harness.md) keeps with the maintainer and counts against its cap of three;
a wrong result shows as a moved reference image; stop: after two attempts that leave
`scripts/check-all.sh` red, or when ticket 01 of this revision has started, the session leaves its
state in this file and hands off.
