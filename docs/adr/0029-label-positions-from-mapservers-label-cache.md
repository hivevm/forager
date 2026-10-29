# ADR-0029: The noise measurement reads MapServer's label cache with a C reader linked to its library

- **Status:** 🟡 proposed
- **Date:** 2026-10-29
- **Deciders:** Markus Brigl
- **Applies to:** `scripts/mapserver-labels.c`, `scripts/build-mapserver.sh`, `.github/workflows/noise.yml`, and every comparison of label positions with MapServer's

## Context

G-10 asks that Forager place and discard the same labels as the reference, each within 1 px of its
reference position. The msautotest references are images: they show where text was drawn, not
which label stands where, nor which labels MapServer discarded. MapServer decides that in its label
cache: `msDrawLabelCache` in `src/mapdraw.c` at `rel-8-6-6` walks the cache's members, one
`labelCacheMemberObj` per label of a feature, by priority, and sets each one's `status` to drawn
(`MS_ON`) or discarded (`MS_DELETE`). A member holds its layer and class indexes, its `bbox`, the
bounds of everything it draws in pixels, and its text symbols, each with its text (`annotext`) and
its point. The cache stays filled after `msDrawMap` returns, until the next map is prepared.

MapServer writes none of this out: no output format carries it, its debug output does not name a
label's position, and its SVG output draws text as paths. Python mapscript, built from the same
source, hides the cache's members from SWIG (`#ifndef SWIG` in `src/mapserver.h`) and exposes only
`num_rendered_members`. The cache's structures are declared in `mapserver.h`, the header the build
of ADR-0027 compiles `map2img` against, and its library `libmapserver` exports `msLoadMap`,
`msDrawMap`, and `msSaveImage`.

## Decision

We will read MapServer's label cache with a C program kept in the repository,
`scripts/mapserver-labels.c`, built by `scripts/build-mapserver.sh` against the unmodified
`libmapserver` and `mapserver.h` of the pinned build, which renders a case as `map2img` renders it
and writes out each label of the cache.

- **The reader.** It takes the `map2img` options the harness applies, `-m`, `-o`, `-l`, `-e`, and
  `-s`, and applies them as `src/apps/map2img.c` at the pin does, calls `msDrawMap`, and saves the
  image with `msSaveImage` where `-o` names it. It then writes, for each member of the cache in
  the order of its priorities and their slots, its layer by index and name, its class index, its
  status, drawn or discarded, its `bbox`, and each text symbol's text and point.
- **The same render.** The noise measurement runs the reader beside `map2img` for every case in
  scope and holds the two images byte for byte equal, so that the cache read is the one of the
  rendering compared.
- **A position.** A label's position is its member's `bbox`; two labels are the same when their
  layer, class, and texts agree, matched in the cache's order; a label matches when its status is
  the same and each edge of its box lies within 1 px of the reference's.
- **The build.** `scripts/build-mapserver.sh` compiles the reader after `map2img`, from the file in
  the repository, with the compiler and the flags of the same build, and nothing patches
  MapServer's source.

**Out of scope:** the harness's comparison and its report, which ticket 09 of the labels spec
builds on this output; options of `map2img` beyond the five; labels of layers MapServer draws
without its label cache.

## Alternatives considered

- **Python mapscript** — the same unmodified build, but SWIG hides the cache's members, so it
  cannot read a label's box or status.
- **A patch to `msDrawLabelCache`** — writes the same data from inside MapServer, but the
  instrument would no longer be the release the suite is pinned to, and ADR-0027 builds it
  unpatched.
- **Positions read from the references** — the images show drawn pixels, not boxes, and say
  nothing of a discarded label.
- **The SVG output** — draws text as glyph paths, with no label or box to read.
- **Pixels only, through the category text of G-13** — measures how a case looks, not which label
  is wrong or discarded, which G-10 asks.

## Sources / Prior art

- MapServer at `rel-8-6-6`: `src/mapserver.h` (`labelCacheObj`, `labelCacheSlotObj`,
  `labelCacheMemberObj`, `textSymbolObj`, the `#ifndef SWIG` guards), `src/mapdraw.c`
  (`msPrepareImage`, `msDrawLabelCache`), `src/apps/map2img.c` (its options, `msDrawMap`,
  `msSaveImage`), `src/mapscript/swiginc/` (no accessor of the cache's members),
  `msautotest/renderers/expected/labels.svg` (text as paths); read 2026-10-29.

## Consequences

- Positive: G-10 is measured label by label against MapServer's own decisions, with the release
  unmodified.
- Positive: the byte equality with `map2img` shows that the reader renders the map the references
  were rendered from.
- Negative: the reader depends on structures MapServer declares but does not promise; moving the
  pin may need it changed, and its build fails if a field it reads is gone.
- Negative: a C file in the repository, outside every crate, built only in the noise measurement.
- Follow-ups: how the harness matches and reports labels, with ticket 09.

## Enforcement

`scripts/test-build-mapserver.sh` cites this ADR with `Verifies:` and fails when
`scripts/build-mapserver.sh` does not compile `scripts/mapserver-labels.c`, or compiles it from
anywhere else. The noise measurement, the ignored test of the harness that `noise.yml` runs, cites
it and fails when the reader's image of a case in scope differs from `map2img`'s.

**Not mechanically decidable:** that the reader writes every field of every member, short of the
comparison ticket 09 builds on it.
