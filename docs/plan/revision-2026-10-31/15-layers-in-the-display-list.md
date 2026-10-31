# Ticket: Make a layer with an opacity part of the display list

## What to build

A layer with an opacity is drawn by a protocol of two calls on the renderer trait, `begin_layer` and
`end_layer`; that layers do not nest is only a `debug_assert` in `render-agg`, and in a release
build a second `begin_layer` overwrites the image saved beneath it. The recording renderers of the
engine's tests stub both calls, so a recorded display list loses the layer's opacity. One item or
call that carries a layer's items and its opacity.

## Acceptance criteria

- [ ] Given a layer with `OPACITY 50`, when the engine draws it into a recording renderer, then the
  record holds the opacity and the layer's items.
- [ ] Given the reference images with opacity, when they render, then they are unchanged.

## Read first

- the `Renderer` trait in `crates/render/src/lib.rs`; `begin_layer` in
  `crates/render-agg/src/lib.rs`; the recording renderers of `crates/engine/tests/`.

## Blocked by

- None (can start immediately)

## Mode

unattended: a recording-renderer test and the reference images decides its criteria; its scope is
`render`, `render-agg`, and the engine's call, and a wrong result costs one revert; it touches none
of the parts [ADR-0007](../../adr/0007-harness.md) keeps with the maintainer and counts against its
cap of three; a wrong result shows as a moved reference image; stop: after two attempts that leave
`scripts/check-all.sh` red, or when a reference image moves, the session leaves its state in this
file and hands off.
