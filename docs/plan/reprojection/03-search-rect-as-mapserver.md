# Ticket: The area asked of a source is the map's extent as MapServer projects it


## What to build

The engine projects the map's extent into a layer's projection as `msProjectRect` does: its
sampling of the rectangle's edges and grid, its handling of a pole inside the extent and of
polar stereographic into longitude and latitude, and its fallback where points fail. A source that
filters by extent returns the features MapServer would draw.

## Acceptance criteria

- [ ] Given a map extent and a pair of projections, when the area asked of a source is computed,
  then it equals `msProjectRect`'s rectangle at the pinned tag, within the tolerance of G-11, for a
  set of cases that covers each of its branches, the expected rectangles computed by MapServer.
- [ ] Given a pole inside the map's extent, when the area is computed, then it spans the longitudes
  `msProjectRect` gives it.
- [ ] Given every case in scope, when they are rendered, then each holds its outcome.

## Read first

- Touches: `crates/proj`, `crates/engine/src/lib.rs`
- MapServer's `msProjectRect`, `msProjectRectGrid`, and `msProjectRectAsPolygon`
  (`src/mapproject.c`) at the pinned tag
- Seam: a test of `crates/proj` with rectangles MapServer computed, and the engine's render with a
  source in memory that records the area it is asked for

## Blocked by

- docs/plan/reprojection/01-reproject-features.md

## Mode

with the human: the reference rectangles come from a MapServer run the human's CI builds.
