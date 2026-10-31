# Ticket: Move the symbolization of a style out of the engine into `symbolize`

## What to build

[ADR-0009](../../adr/0009-ports-and-adapters-crates.md)'s crate map gives the symbolization of a
style a core crate of its own, `symbolize`, beside `label`, and the engine the orchestration of a
request. No `symbolize` crate exists: the engine holds `draw.rs`, `symbol.rs`, `hatch.rs`,
`line_marker.rs`, `offset.rs`, and `geom_transform.rs`, about 2,100 lines of synchronous code
without I/O that turn a style and a geometry into display items and never await a port. The modules
import each other in a cycle (`symbol.rs` and `hatch.rs` take `crate::draw::fixed`, `draw.rs` takes
both), and `draw.rs` has grown to 1,242 lines. Move them into `crates/symbolize`, with the style
bounds and `fixed` in a leaf module, and leave the engine its `Drawing`: the order of the shape
cache, the budgets, and the diagnostics. A wide mechanical refactor: expand, migrate in batches,
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
  `docs/ARCHITECTURE.md`; `crates/engine/src/lib.rs` and the six modules named above; the convention
  on reference images in [`docs/CONVENTIONS.md`](../../CONVENTIONS.md).

## Blocked by

- None (can start immediately)

## Mode

with the human: it moves the code of resource limits (the marker budget of Q-9) that ADR-0007 keeps
with the maintainer, and it re-cuts the crate boundaries every open ticket of `crates/engine` works
against, so it is scheduled when few are open.
