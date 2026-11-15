# Ticket: A shapefile's .prj loaded and read once, as MapServer loads it

## What to build

`ShapefileSource::open` (`crates/adapter-shapefile/src/lib.rs:91-99`) loads the `.prj` beside every
shapefile, whatever the layer's projection. Its `declared_projection` parses the bytes again on
each call (`lib.rs:265-268`). The engine asks for the declaration only for a layer of
`PROJECTION AUTO` (`crates/engine/src/lib.rs:770`). MapServer's `msSHPLayerOpen` reads the `.prj`
only for such a layer. Read that at the pin first. The CLI opens one source per `DATA` path for
all the layers that name it (`crates/cli/src/lib.rs:133,145`), so `open` cannot know the layer.
The cost is one more load through the resource loader per shapefile. The browser binds no
shapefile (ADR-0024).

Parse the `.prj` once, at open or on first use. With the maintainer, decide whether a `.prj` is
loaded only when a layer of `AUTO` asks for it, which changes the port or the CLI's binding. The
other choice is to keep the load and say so in `docs/semantics/shapefile.md`.

## Acceptance criteria

- [ ] Given a source whose `declared_projection` is asked twice, when the adapter's test counts
  the parses, then there is one.
- [ ] Given a layer without `PROJECTION AUTO` over a shapefile, when it is rendered, then the
  `.prj` is loaded or not as the maintainer chose, held by a test with a counting loader.

## Read first

- Touches: `crates/adapter-shapefile/src/lib.rs`, `crates/adapter-shapefile/src/prj.rs`,
  `crates/ports/src/lib.rs`, `crates/cli/src/lib.rs`, `docs/semantics/shapefile.md`
- `docs/adr/0039-declared-projection.md`, `docs/adr/0021-shapefile-adapter.md`,
  `docs/adr/0022-skipped-records.md`; the pinned `msSHPLayerOpen`.

## Blocked by

- docs/plan/revision-2026-11-15/02-proj-repeated-logic.md (both touch `prj.rs`)

## Mode

with the human: a load only on demand may change the port of ADR-0039, which is the maintainer's.
