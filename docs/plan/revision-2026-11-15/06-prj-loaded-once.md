# Ticket: A shapefile's .prj read once, and whether it is loaded only for AUTO

## What to build

`ShapefileSource::open` (`crates/adapter-shapefile/src/lib.rs:91-99`) loads the `.prj` beside every
shapefile, whatever the layer's projection. Its `declared_projection` parses the bytes again on each
call (`lib.rs:265-268`). The engine asks for the declaration only for a layer of `PROJECTION AUTO`
(`crates/engine/src/lib.rs:770`). MapServer's `msSHPLayerOpen` reads the `.prj` only for such a
layer, as ADR-0039's *Context* and `docs/semantics/shapefile.md:47-50` already state. The load at
open is ADR-0039's *Decision* (`0039-declared-projection.md:64`). The CLI opens one source per
`DATA` path for all the layers that name it (`crates/cli/src/lib.rs:133,145`), so `open` cannot know
the layer. The cost is one more load through the resource loader per shapefile. The browser binds no
shapefile (ADR-0024).

Parse the `.prj` once, at open or on first use. Then ask the maintainer whether the load at open
stands. A load only for a layer of `AUTO` changes what ADR-0039 decides, so it supersedes ADR-0039
through `propose-adr` before any code. Keeping the load needs nothing more, since the semantics page
states it.

## Acceptance criteria

- [ ] Given a source whose `declared_projection` is asked twice, when the adapter's test counts
  the parses, then there is one.
- [ ] Given the maintainer's answer on the load, when the ticket closes, then either a proposed
  ADR superseding ADR-0039 is submitted in a commit of ADRs only, or the hand-off records that the
  load at open stands.

## Read first

- Touches: `crates/adapter-shapefile/src/lib.rs`, `crates/adapter-shapefile/src/prj.rs`,
  `crates/ports/src/lib.rs`, `crates/cli/src/lib.rs`, `docs/semantics/shapefile.md`
- `docs/adr/0039-declared-projection.md`, `docs/adr/0021-shapefile-adapter.md`,
  `docs/adr/0022-skipped-records.md`; the pinned `msSHPLayerOpen`.

## Blocked by

- docs/plan/revision-2026-11-15/02-proj-repeated-logic.md

## Mode

with the human: a load only on demand may change the port of ADR-0039, which is the maintainer's.
