# Ticket: Move the symbolization of a style out of the engine into `symbolize`

## What to build

[ADR-0009](../../adr/0009-ports-and-adapters-crates.md)'s crate map gives the symbolization of a
style a core crate of its own, `symbolize`, beside `label`, and the engine the orchestration of a
request. No `symbolize` crate exists: the engine holds `draw.rs`, `bounds.rs`, `symbol.rs`,
`hatch.rs`, `line_marker.rs`, `offset.rs`, and `geom_transform.rs`, about 2,400 lines of synchronous
code without I/O that turn a style and a geometry into display items and never await a port. The
style bounds are already a leaf module (`bounds.rs`). The other modules import each other in a
cycle: `symbol.rs` and `hatch.rs` take `crate::draw::fixed`, `hatch.rs` takes `draw::GAMMA`, and
`line_marker::draw` takes the whole `&mut draw::Canvas` for three of its `pub(crate)` fields (the
image, the factor, the marker budget), while `draw.rs` takes all three. `draw.rs` has grown to 1,491
lines, and it holds msDrawShape's clip of a hatched polygon (`hatch_rings`) and its reference point
apart from the clip buffer and `clip_polygon_rect` in `hatch.rs`. Move them into
`crates/symbolize`, with `fixed` and `GAMMA` beside the bounds in a leaf, `line_marker::draw` handed
what it reads instead of the canvas, the hatch's clip in one module, and `DEG_TO_RAD` (no bound)
out of `bounds.rs`, and leave the engine its `Drawing`: the order of the shape cache, the budgets,
and the diagnostics. A wide mechanical refactor: expand, migrate in batches,
contract, every step green ([`AGENTS.md` §4](../../../AGENTS.md#4-working-style)). The overview's
engine entry moves with it.

## Acceptance criteria

- [ ] Given the workspace, when `crates/arch-test` runs, then `forager-symbolize` exists with the
  edges ADR-0009 allows it and the engine depends on it.
- [ ] Given the engine's tests and `crates/engine/tests/render.rs`'s reference images, when they run
  after the move, then they pass unchanged, no image re-blessed.
- [ ] Given `crates/symbolize`, when its module graph is read, then no module imports a module that
  imports it back.

## Read first

- [ADR-0009](../../adr/0009-ports-and-adapters-crates.md); the engine entry of
  `docs/ARCHITECTURE.md`; `crates/engine/src/lib.rs` and the seven modules named above; the
  convention on reference images in [`docs/CONVENTIONS.md`](../../CONVENTIONS.md).

## Blocked by

- None (can start immediately)

## Mode

with the human: it moves the code of resource limits (the marker budget of Q-9) that ADR-0007 keeps
with the maintainer, and it re-cuts the crate boundaries every open ticket of `crates/engine` works
against, so it is scheduled when few are open.
