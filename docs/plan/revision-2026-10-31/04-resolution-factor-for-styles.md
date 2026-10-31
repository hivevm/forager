# Ticket: Scale styles by `RESOLUTION` over `DEFRESOLUTION`, as labels are

## What to build

The engine computes MapServer's scale and resolution factor, `RESOLUTION` over `DEFRESOLUTION`, in
one place only, `crates/engine/src/labels.rs`, and applies it to a label's size, offset, buffer,
`MINDISTANCE`, and the size of its marker; every style path takes a factor of 1
(`computeSymbolStyle` "at a scale factor of 1" in `symbol.rs`, the hatch, the outline width), and
nothing says so. A map with `RESOLUTION 144` draws its markers at their unscaled size, while its
labels move away from a marker box twice the size of the one drawn. Compute the factor once per
layer beside its scale denominator, and apply it on the style paths where MapServer applies it, read
in its source at the pin (`msDrawMap`, `computeSymbolStyle`, `msDrawShadeSymbol`), each branch
tested, as the porting convention of [`docs/CONVENTIONS.md`](../../CONVENTIONS.md) asks.

## Acceptance criteria

- [ ] Given a point layer with a vector marker and `RESOLUTION 144` over the default
  `DEFRESOLUTION`, when it is drawn, then the marker is twice its size at 72, as MapServer draws it.
- [ ] Given a label with a marker under the same map, when it is drawn, then the label's distance
  from the marker is that of the marker drawn.
- [ ] Given a map without `RESOLUTION`, when the engine's tests run, then nothing moves.

## Read first

- the factor in `crates/engine/src/labels.rs`; `crates/engine/src/symbol.rs`, `hatch.rs`, `draw.rs`;
  the porting entry of [`docs/CONVENTIONS.md`](../../CONVENTIONS.md).

## Blocked by

- None (can start immediately)

## Mode

unattended: a test per scaled style path, against values read from MapServer's source, decides its
criteria; its scope is the engine's style paths, and a wrong result costs one revert; it touches
none of the parts [ADR-0007](../../adr/0007-harness.md) keeps with the maintainer and counts against
its cap of three; a wrong result shows as a reference image of `crates/engine/tests/render.rs` that
moves without a scaled map, or as a conformance case that falls; stop: after two attempts that leave
`scripts/check-all.sh` red, or when a case of the harness falls that passed before, the session
leaves its state in this file and hands off.
