# ADR-0029: The noise measurement reads MapServer's label cache with a C reader linked to its library

- **Status:** 🟡 proposed
- **Date:** 2026-10-29
- **Deciders:** Markus Brigl
- **Applies to:** `crates/cli/tests/conformance/mapserver_labels.c`, `scripts/build-mapserver.sh`, `scripts/check-notice.sh`, and every comparison of label positions with MapServer's

## Context

G-10 asks that Forager place and discard the same labels as the reference, each within 1 px of its
reference position. The msautotest references are images: they show where text was drawn, not
which label stands where, nor which labels MapServer discarded. MapServer decides that in its label
cache, read from its source at `rel-8-6-6`:

- **Members.** Each label of a feature is a `labelCacheMemberObj` in one of the cache's slots, one
  slot per priority, with its layer and class indexes and its text symbols, each with its text
  (`annotext`). `msDrawLabelCache` in `src/mapdraw.c` walks the priorities from the highest down
  and, within a slot, the members from the last added to the first.
- **Drawn.** A member is drawn exactly when `insertRenderedLabelMember` lists it in the cache's
  `rendered_text_symbols`. Its `status` does not say so: a label along a line is set to
  `MS_DELETE` whether it was drawn or not, and a label waiting for a leader line can end as
  `MS_OFF`.
- **Box.** A drawn member's `bbox` is the bounds of its text paths in pixels, final after the
  collision tests. A member discarded before that is given no box: `msAddLabel` takes it from
  reallocated memory and never sets one, and `msDrawLabelCache` computes it only for a label not
  yet discarded.
- **Lifetime.** The cache stays filled after `msDrawMap` returns, until the next map is prepared
  (`msPrepareImage`).

MapServer writes none of this out: no output format carries it, its debug output does not name a
label's position, and its SVG output draws text as paths. Python mapscript, built from the same
source, hides the cache's slots and its list of drawn members from SWIG (`#ifndef SWIG` in
`src/mapserver.h`), so no member is reachable from it. `mapserver.h`, with the
`mapserver-config.h` the build generates, declares the cache, and the shared `libmapserver` that
`map2img` links exports `msLoadMap`, `msDrawMap`, and `msSaveImage`. ADR-0027 builds that library
unpatched and runs the noise measurement on a push to the conformance harness; the spec's licence
constraint keeps MapServer's notice on a file that follows its source.

## Decision

We will read MapServer's label cache with a C program kept beside the conformance harness,
`crates/cli/tests/conformance/mapserver_labels.c`, compiled by `scripts/build-mapserver.sh` on
every run against the unmodified `libmapserver` and headers of the pinned build, which renders a
case as `map2img` renders it and writes out each label of the cache.

- **The reader.** It follows `main` of `src/apps/map2img.c` at the pin for the options the harness
  applies: `msSetup`, `msLoadConfig` without a file, `msProjDataInitFromEnv`, `msDebugInitFromEnv`,
  `msLoadMap`, `msApplyDefaultSubstitutions`, `msApplyStyleItemsToLayers`, the options `-i`, `-l`,
  `-e`, and `-s` as `map2img` applies them, `msDrawMap`, and `msSaveImage` to the file `-o` names.
  It keeps the notice of `map2img.c`.
- **Its output.** One line per member, tab-separated, in the order `msDrawLabelCache` walks them:
  the priority, the layer's index and name, the class index, whether it was drawn (listed in
  `rendered_text_symbols`), its box as `minx miny maxx maxy` for a drawn member and nothing for a
  discarded one, and its texts.
- **The same render.** The noise measurement runs the reader beside `map2img` for every case in
  scope and holds the two images byte for byte equal, so that the cache read is the one of the
  rendering compared.
- **A match.** A label of Forager's is paired with MapServer's by its layer, class, and texts, in
  the cache's order among labels with the same key. A pair matches when both are drawn and each
  edge of the two boxes lies within 1 px, or when both are discarded; a label without a partner
  does not match.
- **The build.** The reader is compiled after `map2img`, outside the cached build, with the
  compiler, include directories, and library of the same build; nothing patches MapServer's source.

**Out of scope:** the harness's report and how it names a mismatch, which ticket 09 of the labels
spec builds; options of `map2img` beyond the harness's; labels MapServer draws without its label
cache, and those a layer's `PROCESSING "FORCE_DRAW_LABEL_CACHE=FLUSH"` draws and empties from it
before the map is drawn, which the reader refuses.

## Alternatives considered

- **Python mapscript** — the same unmodified build, but SWIG hides the cache's slots, so no label's
  box or drawn state can be read.
- **A patch to `msDrawLabelCache`** — writes the same data from inside MapServer, but the
  instrument would no longer be the release the suite is pinned to, and ADR-0027 builds it
  unpatched.
- **`status` as drawn or discarded** — wrong for labels along lines and for leader labels.
- **Comparing the boxes of discarded labels** — MapServer gives most of them none.
- **Pairing in the cache's order alone** — one label added or missing would shift every pair after
  it.
- **The reader under `scripts/`** — a change to it alone would not run the noise measurement, whose
  push paths ADR-0027 fixes.
- **Positions read from the references, or from the SVG output** — images and paths show drawn
  pixels, not labels, and nothing of a discarded label.

## Sources / Prior art

- MapServer at `rel-8-6-6`: `src/mapserver.h` (`labelCacheObj`, `labelCacheSlotObj`,
  `labelCacheMemberObj`, `textSymbolObj`, the `#ifndef SWIG` guards), `src/mapdraw.c`
  (`msPrepareImage`, `msDrawLabelCache`, `insertRenderedLabelMember`), `src/maplabel.c`
  (`msAddLabel`, `msAddLabelGroup`), `src/apps/map2img.c` (`main`), `src/mapscript/swiginc/`
  (no accessor of the cache's members), `msautotest/renderers/expected/labels.svg` (text as
  paths), its `CMakeLists.txt` (`libmapserver` shared); read 2026-10-29.

## Consequences

- Positive: G-10 is measured label by label against MapServer's own decisions, with the release
  unmodified.
- Positive: the byte equality with `map2img` shows that the reader renders the map the references
  were rendered from.
- Negative: comparing all four edges holds a label's size too, so a text laid out more than 1 px
  wider or narrower fails G-10 as well as the category text of G-13.
- Negative: the reader depends on structures MapServer declares but does not promise; moving the
  pin may need it changed, and its build fails if a field it reads is gone.
- Negative: a C file in the repository, outside every crate's build, compiled only in the noise
  measurement.
- Follow-ups: the harness's report of the labels, with ticket 09.

## Enforcement

`scripts/test-build-mapserver.sh` cites this ADR with `Verifies:` and fails when
`scripts/build-mapserver.sh` does not compile `crates/cli/tests/conformance/mapserver_labels.c`,
compiles it inside the cached build, or patches the checkout (`patch`, `git apply`, `git am`, or
`sed -i` under it). `scripts/check-notice.sh` holds the reader to the notice of `map2img.c`. The
noise measurement, the ignored test of the harness that `noise.yml` runs, cites it and fails when
the reader's image of a case in scope differs from `map2img`'s.

**Not mechanically decidable:** that the reader writes every field of every member, short of the
comparison ticket 09 builds on it.
