# ADR-0033: CI builds MapServer from the pinned tag to measure its noise, and reads its label cache with a C reader linked to that build

- **Status:** 🟢 accepted
- **Date:** 2026-10-29
- **Deciders:** Markus Brigl
- **Applies to:** `scripts/build-mapserver.sh`, `.github/workflows/noise.yml`, every run of MapServer itself, `crates/cli/tests/conformance/mapserver_labels.c`, `scripts/check-notice.sh`, the records of MapServer's labels under `crates/cli/tests/conformance/labels/` and the mark `labels-hold` of `crates/cli/tests/conformance/cases.txt`, and every comparison of label positions with MapServer's

## Context

G-6 and G-13 hold a rendering to its msautotest reference within the tolerance of its category, and
the specification assumes that the references are reproducible enough for those tolerances to lie
above their platform noise. The noise is how far MapServer itself, run off the platform that
produced the references, lands from them; so the measurement needs MapServer at the release the
suite is pinned to, `rel-8-6-6` at the commit `scripts/fetch-msautotest.sh` names. Nothing of the
product links it: it is a measuring instrument, run over the in-scope cases, whose renderings the
harness compares by its metric.

MapServer is C with C dependencies (PROJ, GDAL, FreeType, libpng, Cairo), which the dev container
([ADR-0009](0009-dev-container.md)) does not carry; adding them would grow the container of every
session for a job that runs only when its inputs change. The run depends on MapServer, the scope
list, and the harness that compares, not on the rest of Forager's code. MapServer's own CI builds
the tag from source in an `ubuntu:24.04` container, with Ubuntu's packages, the ubuntugis-unstable
PPA, and Kitware's CMake repository, and runs the suite there, with the suite served by a local
HTTP server for the symbols a case names by a URL.

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
`map2img` links exports `msLoadMap`, `msDrawMap`, and `msSaveImage`. The specification's licence
constraint keeps MapServer's notice on a file that follows its source.

MapServer is not run where every push is checked: the push harness compares Forager's labels with
a record of MapServer's, taken once from the reader's output and kept in the tree. The libraries
MapServer draws text with, freetype and harfbuzz among them, come unpinned from the runner's
Ubuntu archive, so a package update can move a box by a fraction of a pixel that no rendering
shows.

The maintainer settled three questions. A record is not held byte for byte, since the packages
under it are unpinned: its boxes are held within 1e-6 px, far below a pixel, so an update of the
packages that moves no label does not fail the noise measurement. The refusal of a patched
checkout is not decided by a test: a text match of the build script names the common tools and
passes others, so it is a tripwire. G-10's "the same labels" asks for the priority slot: a label
drawn at the same box from another slot is another label.

## Decision

We will build MapServer from the pinned commit on a GitHub-hosted runner to measure its noise, and
read its label cache there with a C reader linked to that build, whose output the tree keeps as
records the push harness compares Forager's labels with.

**The build.**

- **Where.** MapServer is built and run only on a GitHub-hosted `ubuntu-24.04` runner, in the
  workflow `.github/workflows/noise.yml`, and nowhere else.
- **What.** The build takes the commit `scripts/fetch-msautotest.sh` names, with MapServer's
  default build options, SVG symbols through librsvg, and curl, as its own CI builds the
  references. It takes the Ubuntu archive's packages that `scripts/build-mapserver.sh` names.
- **When.** The workflow runs on demand, and on a push that changes `crates/cli/tests/conformance/`,
  `crates/cli/tests/metric/`, `crates/cli/tests/msautotest.rs`, `scripts/fetch-msautotest.sh`,
  `scripts/build-mapserver.sh`, or the workflow.
- **The pin.** The build script must read the tag and the commit from the two assignments in
  `scripts/fetch-msautotest.sh`, the one place that names the pin. It must check out that commit
  and refuse to build when its checkout is another.
- **Sources.** The build script must add no apt source. Nothing may patch MapServer's source.
- **The dev container.** The build script must refuse to run outside GitHub Actions, so it never
  runs in the dev container.
- **The record of a run.** The run writes into its job summary the CMake options of the build. For
  each package whose library `map2img` links, it writes the version and the origin
  `apt-cache policy` reports.
- **The run.** The workflow caches the build by the hash of the build script and of
  `scripts/fetch-msautotest.sh`. It serves the suite on `localhost:8000`, as MapServer's CI does,
  and compares the renderings in the same run.

**The label cache.**

- **The reader.** `crates/cli/tests/conformance/mapserver_labels.c` renders a case as `map2img`
  renders it and writes out each label of the cache. It follows `main` of `src/apps/map2img.c` at
  the pin for the options the harness applies: `msSetup`, `msLoadConfig` without a file,
  `msProjDataInitFromEnv`, `msDebugInitFromEnv`, `msLoadMap`, `msApplyDefaultSubstitutions`,
  `msApplyStyleItemsToLayers`, the options `-i`, `-l`, `-e`, and `-s` as `map2img` applies them,
  `msDrawMap`, and `msSaveImage` to the file `-o` names. It keeps the notice of `map2img.c`. It
  refuses a map one of whose layers flushes the label cache
  (`PROCESSING "FORCE_DRAW_LABEL_CACHE=FLUSH"`).
- **Its build.** `scripts/build-mapserver.sh` compiles the reader on every run, after `map2img` and
  outside the cached build, against the unmodified `libmapserver` and headers of the pinned build,
  with the compiler, include directories, and library of the same build.
- **Its output.** One line per member, tab-separated, in the order `msDrawLabelCache` walks them:
  the priority, the layer's index and name, the class index, whether it was drawn (listed in
  `rendered_text_symbols`), its box, and its texts. The box is `minx miny maxx maxy` written with
  `%.17g` for a drawn member, and nothing for a discarded one.
- **The records.** The output for each text case in scope is a record under
  `crates/cli/tests/conformance/labels/`, at the path the harness gives the case. A record must be
  taken from the noise measurement's artifact, `noise/labels/`, and never written by hand. Every
  record belongs to a text case in scope, and every such case has one.
- **The same render.** The noise measurement runs the reader beside `map2img` for every text case
  in scope. It holds the two images byte for byte equal, so that the cache read is the one of the
  rendering compared. It holds the reader's output equal to the record: the priority, the layer's
  index and name, the class, the texts, the drawn state, and the order exactly, each edge of each
  box within 1e-6 px.
- **A match.** A label of Forager's is paired with MapServer's by its priority, layer, class, and
  texts, in the cache's order among labels with the same key. A pair matches when both are drawn
  and each edge of the two boxes lies within 1 px, or when both are discarded. A label without a
  partner does not match.
- **The mark.** A text case whose labels all match is marked `labels-hold` in `cases.txt`, and only
  such a case. The push harness fails on a case whose labels and mark disagree, and on a case
  marked `labels-hold` whose render fails.

**Out of scope:** the tolerances of G-6 and G-13, which the specification states; running
MapServer on another platform; how the harness reports a mismatch; options of `map2img` beyond the
harness's; labels MapServer draws without its label cache, and those a flushing layer draws and
empties from it before the map is drawn.

## Alternatives considered

- **conda-forge's `mapserver 8.6.6`** — the exact release, prebuilt, and its dependencies can be
  locked; but it brings conda's FreeType, libpng, and GDAL, a platform further from the one that
  made the references than Ubuntu's, and a second package ecosystem into CI.
- **camptocamp's `mapserver` image** — has no `8.6.6` tag, only `8.6` and its GDAL variants beside
  `8.6.0`; a digest pins a binary of a third party, not one built from the pinned commit.
- **Ubuntu's `mapserver-bin`** — 24.04 ships 8.0.1, not the pinned release.
- **A clean `ubuntu:24.04` container on the runner** — closer to "the archive only", since the
  runner image carries preinstalled packages and apt sources of its own; but it needs a container
  job or a `docker run` step, and the cache of the build moves with it. The runner is kept, and
  the origin of every linked package is written down instead, so a package from a source other
  than the archive is seen.
- **MapServer's own CI environment** — that container with the ubuntugis-unstable PPA and
  Kitware's repository; the PPA moves faster than the archive and is a third party's.
- **A build with only what `map2img` and AGG need** — a smaller build, but an option off that a
  case reaches, such as GEOS for a geometry transform, would count as noise what is a
  configuration difference.
- **The default options alone** — `WITH_RSVG` and `WITH_CURL` are off by default, while
  MapServer's CI builds the references with both, so the cases with SVG symbols or a symbol named
  by a URL would fail on the configuration, not on noise.
- **Building in the dev container** — a supersession of [ADR-0009](0009-dev-container.md), for a
  job that runs rarely.
- **On every push** — a build and a run per push for a result that only the pin, the scope, and
  the harness change.
- **Python mapscript** — the same unmodified build, but SWIG hides the cache's slots, so no label's
  box or drawn state can be read.
- **A patch to `msDrawLabelCache`** — writes the same data from inside MapServer, but the
  instrument would no longer be the unpatched release the suite is pinned to.
- **`status` as drawn or discarded** — wrong for labels along lines and for leader labels.
- **Comparing the boxes of discarded labels** — MapServer gives most of them none.
- **Pairing in the cache's order alone** — one label added or missing would shift every pair after
  it.
- **Pairing without the priority** — a label drawn from another slot at the same box would match.
- **The reader under `scripts/`** — a change to it alone would not run the noise measurement, whose
  push paths the Decision fixes.
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

- MapServer at `rel-8-6-6`: `.github/workflows/build.yml` (runs `start.sh` in `ubuntu:24.04`),
  `.github/workflows/start.sh`, `ci/ubuntu/setup.sh` (Ubuntu packages, the ubuntugis-unstable
  PPA, and Kitware's repository), `ci/ubuntu/build.sh`, the `cmakebuild_nocoverage` target of
  its `Makefile` with `-DWITH_RSVG=1` and `-DWITH_CURL=1`, the `python -m http.server` that
  `ci/ubuntu/build.sh` starts in `msautotest/`, and the option defaults of `CMakeLists.txt`; read
  2026-10-29.
- MapServer at `rel-8-6-6`: `src/mapserver.h` (`labelCacheObj`, `labelCacheSlotObj`,
  `labelCacheMemberObj`, `textSymbolObj`, the `#ifndef SWIG` guards), `src/mapdraw.c`
  (`msPrepareImage`, `msDrawLabelCache`, `insertRenderedLabelMember`), `src/maplabel.c`
  (`msAddLabel`, `msAddLabelGroup`), `src/apps/map2img.c` (`main`), `src/mapscript/swiginc/`
  (no accessor of the cache's members), `msautotest/renderers/expected/labels.svg` (text as
  paths), its `CMakeLists.txt` (`libmapserver` shared); read 2026-10-29.
- conda-forge `mapserver`, latest 8.6.6 (anaconda.org API, checked 2026-10-29).
- Docker Hub `camptocamp/mapserver` tags `8.6`, `8.6-gdal3.12`, `8.6-gdal3.13`, `8.6.0`, no
  `8.6.6` (checked 2026-10-29).
- Ubuntu noble `mapserver` 8.0.1-4ubuntu2 (Launchpad, checked 2026-10-29).
- The reader, the records, and the harness as built: `crates/cli/tests/conformance/`,
  `crates/cli/tests/msautotest.rs`, `scripts/build-mapserver.sh`; read 2026-11-07.

## Consequences

- Positive: the instrument is the pinned release, built from the commit the suite comes from,
  with no binary from a third party and no patch; moving the pin rebuilds it in the same change.
- Positive: nothing of it reaches the dev container or the checks of every push.
- Positive: G-10 is measured label by label against MapServer's own decisions, and every push
  holds Forager to them without building MapServer; the byte equality with `map2img` shows that
  the reader renders the map the references were rendered from.
- Negative: Ubuntu's packages float with the archive and the runner image, so two runs months
  apart may link other versions; the job summary names them, so a shift in the noise can be
  traced. A package update that moves a box by more than 1e-6 px fails the noise measurement until
  the record is taken again.
- Negative: every change that flips an outcome in the scope list runs it; the cached build keeps
  that to the rendering and the comparison. An uncached build takes minutes, and the runner label
  must be moved by hand when GitHub retires `ubuntu-24.04`.
- Negative: comparing all four edges holds a label's size too, so a text laid out more than 1 px
  wider or narrower fails G-10 as well as the category text of G-13.
- Negative: the reader depends on structures MapServer declares but does not promise; moving the
  pin may need it changed, and its build fails if a field it reads is gone. It is a C file in the
  repository, outside every crate's build, compiled only in the noise measurement.
- Follow-ups: whether a weekly run is worth its cost, should the archive's drift show in the
  noise; whether the reader, compiled anew on every run against `gdal-config`, can drift in ABI
  from a cached `libmapserver` whose key is the two build scripts alone.

## Enforcement

`scripts/test-build-mapserver.sh`, run by `scripts/check-all.sh` and by CI, cites this ADR with
`Verifies:`. It fails when:

- `scripts/build-mapserver.sh` names a tag or a commit of its own instead of reading the
  assignments of `scripts/fetch-msautotest.sh`;
- the build script, run without `GITHUB_ACTIONS=true`, does not refuse;
- the build script or the workflow adds an apt source (`add-apt-repository`, `apt-add-repository`,
  or a write under `/etc/apt/sources.list`);
- the workflow runs on another runner than `ubuntu-24.04`, uses a container (`container:`,
  `services:`, `docker`, or `podman`), triggers on anything but `workflow_dispatch` and a push
  filtered to the paths of the Decision, or caches the build under a key other than the hash of
  the two scripts;
- the build script does not compile `crates/cli/tests/conformance/mapserver_labels.c`, or compiles
  it inside the cached build.

`scripts/check-notice.sh` holds the reader to MapServer's notice, which it reaches through the
reader's mention of `src/apps/map2img.c`. A test of the harness that runs on every push without the
suite holds the records and the text cases in scope to each other; the harness's comparison cites
G-10 for the cases marked `labels-hold`. Unit tests of the harness in
`crates/cli/tests/conformance/tests.rs` cite this ADR with `Verifies:`: labels of another priority
at the same box do not match (`labels::compare`), and the reader's output holds the record within
1e-6 px in its boxes and exactly in every other field (`labels::beyond_noise`). The noise
measurement, the ignored test of `crates/cli/tests/msautotest.rs` that `noise.yml` runs, cites
this ADR with `Verifies:`. It fails when the reader's image of a text case in scope differs from
`map2img`'s, or its output from the record: a box by more than 1e-6 px, or any other field at all.

**Not mechanically decidable:** no test runs the build or reads a run, so a reviewer holds:

- that the build keeps MapServer's default options with `WITH_RSVG` and `WITH_CURL` on, and that
  the job summary lists the linked packages with their origins, short of a run; a reviewer reads
  the script and a run's summary;
- that nothing patches MapServer's checkout: the self-test's match of `patch`, `git apply`,
  `git am`, and `sed -i` is a tripwire for the common tools, not a proof;
- that a record is taken from the noise run's artifact and never written by hand;
- that the reader writes every field of every member, short of the comparison the harness makes
  with its records.
