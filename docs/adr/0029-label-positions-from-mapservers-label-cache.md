# ADR-0029: The noise measurement reads MapServer's label cache with a C reader linked to its library

- **Status:** 🟢 accepted
- **Date:** 2026-10-29
- **Deciders:** Markus Brigl
- **Applies to:** `crates/cli/tests/conformance/mapserver_labels.c`, `scripts/build-mapserver.sh`, `scripts/check-notice.sh`, the records of MapServer's labels under `crates/cli/tests/conformance/labels/` and the mark `labels-hold` of `crates/cli/tests/conformance/cases.txt`, and every comparison of label positions with MapServer's

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
- **Box.** A drawn member's `bbox` is the bounds of its text paths and of its `labelpoint`
  styles' markers in pixels, as `msDrawLabelCache` forms it, final after the collision tests. A
  member discarded before that is given no box: `msAddLabel` takes it from reallocated memory and
  never sets one, and `msDrawLabelCache` computes it only for a label not yet discarded.
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

MapServer is not run where every push is checked: the push harness compares Forager's labels with
a record of MapServer's, taken once from the reader's output and kept in the tree. The libraries
MapServer draws text with, freetype and harfbuzz among them, come unpinned from the runner's
Ubuntu archive (`scripts/build-mapserver.sh`), so a package update can move a box by a fraction of
a pixel that no rendering shows.

Questions the maintainer settled:

1. Is a record held byte for byte, though the packages under it are unpinned? No: its boxes are
   held within 1e-6 px, far below a pixel, so an update of the packages that moves no label does
   not fail the noise measurement.
2. Is the refusal of a patched checkout decided by a test? No: a text match of the build script
   names the common tools and passes others, so it is a tripwire, and the rule is not mechanically
   decidable.
3. Does G-10's "the same labels" ask for the priority slot? Yes: a label drawn at the same box
   from another slot is another label.

Open, a reason to revisit and not a condition of this decision: whether the reader, compiled anew
on every run against `gdal-config`, can drift in ABI from a cached `libmapserver` whose key is the
two build scripts alone.

## Decision

We will read MapServer's label cache with a C program kept beside the conformance harness,
`crates/cli/tests/conformance/mapserver_labels.c`, compiled by `scripts/build-mapserver.sh` on
every run against the unmodified `libmapserver` and headers of the pinned build, which renders a
case as `map2img` renders it and writes out each label of the cache; and keep its output for every
text case in scope as a record beside the list, which the push harness compares Forager's labels
with.

- **The reader.** It follows `main` of `src/apps/map2img.c` at the pin for the options the harness
  applies: `msSetup`, `msLoadConfig` without a file, `msProjDataInitFromEnv`, `msDebugInitFromEnv`,
  `msLoadMap`, `msApplyDefaultSubstitutions`, `msApplyStyleItemsToLayers`, the options `-i`, `-l`,
  `-e`, and `-s` as `map2img` applies them, `msDrawMap`, and `msSaveImage` to the file `-o` names.
  It keeps the notice of `map2img.c`. It refuses a map one of whose layers flushes the label cache
  (`PROCESSING "FORCE_DRAW_LABEL_CACHE=FLUSH"`).
- **Its output.** One line per member, tab-separated, in the order `msDrawLabelCache` walks them:
  the priority, the layer's index and name, the class index, whether it was drawn (listed in
  `rendered_text_symbols`), its box as `minx miny maxx maxy` written with `%.17g` for a drawn
  member and nothing for a discarded one, and its texts.
- **The records.** The output for each text case in scope is a record under
  `crates/cli/tests/conformance/labels/`, at the path the harness gives the case. A record is
  taken from the noise measurement's artifact, `noise/labels/`, never written by hand; every record
  belongs to a text case in scope, and every such case has one.
- **The same render.** The noise measurement runs the reader beside `map2img` for every text case
  in scope, holds the two images byte for byte equal, so that the cache read is the one of the
  rendering compared, and holds its output equal to the record: the priority, the layer's index
  and name, the class, the texts, the drawn state, and the order exactly, each edge of each box
  within 1e-6 px.
- **A match.** A label of Forager's is paired with MapServer's by its priority, layer, class, and
  texts, in the cache's order among labels with the same key. A pair matches when both are drawn
  and each edge of the two boxes lies within 1 px, or when both are discarded; a label without a
  partner does not match.
- **The mark.** A text case whose labels all match is marked `labels-hold` in `cases.txt`, and only
  such a case; the push harness fails on a case whose labels and mark disagree, and on a case
  marked `labels-hold` whose render fails.
- **The build.** The reader is compiled after `map2img`, outside the cached build, with the
  compiler, include directories, and library of the same build; nothing patches MapServer's source.

**Out of scope:** how the harness reports a mismatch; options of `map2img` beyond the harness's;
labels MapServer draws without its label cache, and those a flushing layer draws and empties from
it before the map is drawn.

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
- **Pairing without the priority** — a label drawn from another slot at the same box would match.
- **The reader under `scripts/`** — a change to it alone would not run the noise measurement, whose
  push paths ADR-0027 fixes.
- **Positions read from the references, or from the SVG output** — images and paths show drawn
  pixels, not labels, and nothing of a discarded label.
- **Running the reader on every push** — the push would build MapServer; the record carries its
  output instead.
- **Records held byte for byte** — a package update that moves a box by 1e-12 px would fail the
  noise measurement though no label moved.
- **A line in each record naming the package versions it was taken under** — keeps byte equality,
  at the cost of a header in every record and a second rule for reading it.
- **Taking the records again whenever the packages drift** — keeps byte equality as a rule of
  work, with every such update a manual step.
- **Wider patterns for a patched checkout** — `perl -i`, a redirect into the checkout, a script in
  another language: each pattern added leaves another out, and comments trip them.

## Sources / Prior art

- MapServer at `rel-8-6-6`: `src/mapserver.h` (`labelCacheObj`, `labelCacheSlotObj`,
  `labelCacheMemberObj`, `textSymbolObj`, the `#ifndef SWIG` guards), `src/mapdraw.c`
  (`msPrepareImage`, `msDrawLabelCache`, `insertRenderedLabelMember`), `src/maplabel.c`
  (`msAddLabel`, `msAddLabelGroup`), `src/apps/map2img.c` (`main`), `src/mapscript/swiginc/`
  (no accessor of the cache's members), `msautotest/renderers/expected/labels.svg` (text as
  paths), its `CMakeLists.txt` (`libmapserver` shared); read 2026-10-29.
- The reader, the records, and the harness as built: `crates/cli/tests/conformance/`,
  `crates/cli/tests/msautotest.rs`, `scripts/build-mapserver.sh`; read 2026-11-07.

## Consequences

- Positive: G-10 is measured label by label against MapServer's own decisions, with the release
  unmodified, and every push holds Forager to them without building MapServer.
- Positive: the byte equality with `map2img` shows that the reader renders the map the references
  were rendered from.
- Negative: comparing all four edges holds a label's size too, so a text laid out more than 1 px
  wider or narrower fails G-10 as well as the category text of G-13.
- Negative: the reader depends on structures MapServer declares but does not promise; moving the
  pin may need it changed, and its build fails if a field it reads is gone.
- Negative: a C file in the repository, outside every crate's build, compiled only in the noise
  measurement.
- Negative: a record is only as current as the noise run it was taken from; a package update that
  moves a box by more than 1e-6 px fails the noise measurement until the record is taken again.
- Follow-ups: the notice check reaching the reader through `src/apps/map2img.c`.

## Enforcement

`scripts/test-build-mapserver.sh` cites this ADR with `Verifies:` and fails when
`scripts/build-mapserver.sh` does not compile `crates/cli/tests/conformance/mapserver_labels.c` or
compiles it inside the cached build. `scripts/check-notice.sh` holds the reader to MapServer's
notice; it reaches the reader through its mention of `src/mapdraw.c`, not yet through
`src/apps/map2img.c`. A test of the harness that runs on every push without the suite holds the
records and the text cases in scope to each other; the harness's comparison cites G-10 for the
cases marked `labels-hold`. Unit tests of the harness cite this ADR with `Verifies:`: labels of
another priority at the same box do not match (`labels::compare`), and the reader's output holds
the record within 1e-6 px in its boxes and exactly in every other field (`labels::beyond_noise`).
The noise measurement, the ignored test of the harness that `noise.yml` runs, cites this ADR with
`Verifies:` and fails when the reader's image of a text case in scope differs from `map2img`'s, or
its output from the record: a box by more than 1e-6 px, or any other field at all.

**Not mechanically decidable:** that nothing patches MapServer's checkout — the self-test's match
of `patch`, `git apply`, `git am`, and `sed -i` is a tripwire for the common tools, not a proof;
that a record is taken from the noise run's artifact and never written by hand; and that the
reader writes every field of every member, short of the comparison the harness makes with its
records.
