# Ticket: MapServer's is_polar memo ported or its divergence stated

## What to build

`msProjectRect` (`src/mapproject.c:1981-2040` at the pin) enters its two polar branches only while
`in->is_polar < 0`, and sets it to 0 or 1 on the first call. `in` is the map's projection, which
every layer of the map shares. So only the first layer drawn takes the polar search, and every
later layer takes the generic path with `+over`. `crates/proj/src/rect.rs:43-71` decides the polar
case afresh for each layer. In a map in a polar projection, the area Forager asks of the second
and later layers' sources can therefore differ from MapServer's. The reader
`crates/cli/tests/conformance/mapserver_proj.c` loads both projections afresh for each rectangle
so that its records match Forager's reading. That choice is the divergence, written down only
there.

With the maintainer: port the memo through state the render holds per map, or keep Forager's
reading and state the divergence in `docs/semantics/projection.md`. Find first whether any case in
scope reaches it.

## Acceptance criteria

- [ ] Given a map in a polar projection with two layers in longitude and latitude, when it is
  rendered, then the areas asked of the two sources are those the maintainer's choice says,
  held by an engine test with sources in memory.
- [ ] Given `docs/semantics/projection.md`, when the change lands, then it says how Forager reads
  `is_polar`.

## Read first

- Touches: `crates/proj/src/rect.rs`, `crates/engine/src/reproject.rs`,
  `crates/engine/tests/reproject.rs`, `docs/semantics/projection.md`
- `docs/adr/0037-reprojection-ported-in-proj.md`, `docs/adr/0038-proj-references-from-mapserver.md`;
  the pinned `src/mapproject.c`, `msProjectRect`, and `src/mapdraw.c`'s call of it.

## Blocked by

- docs/plan/revision-2026-11-15/02-proj-repeated-logic.md (both touch `rect.rs`)

## Mode

with the human: following MapServer's order-dependent result, or departing from it, is a choice
about fidelity the maintainer makes.
