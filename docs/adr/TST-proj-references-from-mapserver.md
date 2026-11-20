# TST-proj-references-from-mapserver: The reference points, rectangles, and shapes of G-11 are reprojected by MapServer's own calls of PROJ at the pinned build, in the noise run, and kept as records beside the test of `proj`

- **Status:** 🟢 accepted
- **Date:** 2026-11-15
- **Deciders:** Markus Brigl
- **Applies to:** `crates/proj/tests/reference/` (the lists `points.tsv`, `rects.tsv`, and `shapes.tsv` and their records `mapserver.tsv`, `mapserver-rects.tsv`, and `mapserver-shapes.tsv`), the test `crates/proj/tests/reference.rs`, `crates/cli/tests/conformance/mapserver_proj.c`, its build in `scripts/build-mapserver.sh` and that script's self-test `scripts/test-build-mapserver.sh`, its run in `.github/workflows/noise.yml` and the harness's noise module `crates/cli/tests/conformance/noise/`, and the paths that start that workflow

## Context

G-11 asks that "when a point, a rectangle, or a shape is reprojected between the proj4 projections
the test cases use or the EPSG codes the projection resolver builds in, the core's reprojection
shall agree with MapServer at the pin, which reprojects through PROJ". It shall fail where MapServer
fails, leave a shape with the lines and the number of points MapServer leaves it, and place each
coordinate within 1 mm of MapServer's. [GEO-proj-ported-with-libm](GEO-proj-ported-with-libm.md)
ported PROJ's formulas into `crates/proj`.

The measurement began as the reprojection plan's ticket 02. It asked for the reference coordinates
over the transformations the msautotest cases in scope and the built-in codes of `adapter-epsg`
use, recorded beside the test so that the test needs no PROJ to run, and for a script that names
the PROJ version and the MapServer build that computed them. On 2026-11-14 the maintainer chose,
in four questions, MapServer's own path as the source, a fixed list in the repository, the noise
run as the place, and `crates/proj/tests/reference/` for the records.

That decision of 2026-11-14 measured the agreement for points alone: one list, `points.tsv`,
reprojected by `msProjectPoint` in a C reader linked to the pinned MapServer build, in the noise
run, into one record, `mapserver.tsv`. It left the ortho and `+over` cases to "add lines to the
list", and the tolerance to a measurement. The work of the reprojection plan grew past it:

- The area asked of a source is the map's extent as `msProjectRect` projects it (the plan's ticket
  03). `rects.tsv` and its record `mapserver-rects.tsv` hold it, one group per branch of
  `msProjectRect`.
- A shape is cut at the horizon and wrapped at the dateline as `msProjectShape` does (tickets 05
  and 06). The ortho and `+over` cases went into `shapes.tsv` and its record
  `mapserver-shapes.tsv`, not into the list of points. A point's reprojection cannot show a cut or
  a wrap.
- `msProjectRect` sets `is_polar` on its source projection from the first rectangle it projects
  and keeps it ([`docs/semantics/projection.md`](../semantics/projection.md)). A reader that kept
  one projection across rectangles would record a rectangle as the rectangles before it left
  that projection. The reader therefore loads both projections afresh for each rectangle.
- The landed G-11 (45a01425) names rectangles and shapes, failure for failure, and a shape's lines
  and points. It also states the tolerance, 1 mm, which the decision of 2026-11-14 left out of
  scope until the measured spread was in.

What that decision found still holds. PROJ is not in the dev container and must not enter the core
(Q-1, [ARCH-crates-as-ports-and-adapters](ARCH-crates-as-ports-and-adapters.md)). The GitHub-hosted
runner where [TST-noise-measured-against-mapserver](TST-noise-measured-against-mapserver.md) builds
MapServer at the pin has Ubuntu 24.04's `libproj-dev`, PROJ 9.4.0, which `map2img` links.
TST-noise-measured-against-mapserver already reads MapServer's label cache with a C reader linked to
the unmodified `libmapserver`, and keeps its output as records taken from the run's artifact, never
written by hand. What MapServer draws is not PROJ's bare formula: `msProjectPoint`
(`src/mapproject.c`) goes through `msProjectCreateReprojector` and `createNormalizedPJ`, which asks
PROJ for an operation by `proj_create_crs_to_crs_from_pj` and normalizes the axis order for
visualization. Which operation PROJ picks there, for NAD83 to WGS84 among others, is what the port
of [GEO-proj-ported-with-libm](GEO-proj-ported-with-libm.md) cannot verify alone, and what a
reference taken by `cs2cs` on the same strings would miss.

On 2026-11-15 the maintainer left one rectangle out of `rects.tsv` and its record: the conic to the
south pole, whose corners lie some 4e10 m off at the horizon's singularity and differ by up to 3 m.
The next noise run reproduced the shortened record.

On 2026-11-15 the maintainer widened the reference to the rectangles and the shapes, as the
design revision's ticket 12 of that day asked.

## Decision

We will compute G-11's reference by a C reader linked to the pinned MapServer build, which
reprojects three committed lists of points, rectangles, and shapes through `msProjectPoint`,
`msProjectRect`, and `msProjectShape` in the noise run, and keep its output as records beside the
test of `proj`.

- **The lists.** `crates/proj/tests/reference/` must hold three lists, tab-separated, comment
  lines starting with `#`:
  - `points.tsv`: per line a source and a target projection and a point `x`, `y` in the source.
    It must cover the transformations of the cases in scope, EPSG:4326 to and from EPSG:3857, and
    each built-in UTM zone that a point names.
  - `rects.tsv`: per line a map's projection and a layer's and a map's extent in the first, as
    `minx`, `miny`, `maxx`, `maxy`.
  - `shapes.tsv`: per line a source and a target projection, the shape's type, `point`, `line`, or
    `polygon`, and its lines. A line is written `x,y x,y ...`, and the lines are separated by `|`.
- **The strings.** Every projection of a list must be a proj4 string as Forager's frontend writes
  it. It must be the resolver's expansion, not an `init=` reference. MapServer and `proj` then read
  the same text.
- **The reader.** `crates/cli/tests/conformance/mapserver_proj.c` must read the one list its
  argument names. It must load each projection with `msInitProjection` and
  `msLoadProjectionString`, and reproject:
  - a line of four fields whose third is a number, a point, with `msProjectPoint`;
  - a line of six fields, a rectangle, with `msProjectRect`, both projections loaded afresh for
    each rectangle, so that no `is_polar` carries over;
  - a line of four fields whose third is `point`, `line`, or `polygon`, a shape, with
    `msProjectShape`, both projections loaded afresh for each shape.
- **The output.** The reader must write two header lines first: `# PROJ` with
  `proj_info().release`, and `# MapServer` with the commit it was built from. Then it must write
  each line of the list, a tab, and its result:
  - a point's `x` and `y`;
  - a rectangle's `minx`, `miny`, `maxx`, and `maxy`;
  - each line the shape is left with, in order, as `x,y x,y ...`, and `-` for a line left without
    points;
  - `fail` where MapServer's function fails.
  It must write each coordinate with `%.17g`. It must end with an error on a line of neither four
  nor six fields and on a projection that does not load.
- **Its build.** `scripts/build-mapserver.sh` must compile the reader as it compiles
  `mapserver_labels.c`. It does so on every run, outside the cached build, against the unmodified
  `libmapserver` and headers, with the same compiler, include directories, and library.
- **The run.** The noise run must run the reader over each of the three lists and leave each
  output in the artifact, under `noise/proj/` by its record's name. It must fail where:
  - the reader does not run or fails;
  - a record is missing;
  - an output and its record differ in their number of lines, or in a header line;
  - a line's fields from the list differ, or one side fails and the other does not;
  - a shape differs in its number of lines or in the number of points of a line;
  - a coordinate differs by more than 1 mm, in metres for a projected target and by 9e-9 degrees,
    about 1.0 mm, for a geographic one.
- **The records.** `mapserver.tsv`, `mapserver-rects.tsv`, and `mapserver-shapes.tsv` must be taken
  from the artifact and must never be written by hand. A record must be taken again when its list
  changes or the runner's PROJ moves.
- **A line left out.** A line of a list may be removed only on the maintainer's decision. The
  change that removes it must remove its line of the record too. The next noise run must reproduce
  the shortened record.
- **The test.** `crates/proj/tests/reference.rs` must hold `proj` to each record within the
  tolerance G-11 states. It must need no PROJ and no MapServer. It must fail where:
  - a record does not name the PROJ release and the MapServer build first;
  - a record holds no line for a line of its list, or a line not of its list;
  - a point, a rectangle, or a shape fails on one side only;
  - a point, a corner of a rectangle, or a point of a shape lies outside the tolerance;
  - a shape is left with other lines or another number of points per line than MapServer's.
- **The functions under test.** The test must reproject a point by `Reprojection::point` and a
  rectangle by `Reprojection::extent`. It must reproject a line or a polygon by the same
  `Reprojection::shape` the engine runs, and a point shape by `Reprojection::point_line`, as the
  engine does.
- **The trigger.** A push that changes `crates/proj/tests/reference/` or the reader must start the
  noise workflow, beside the paths TST-noise-measured-against-mapserver names.

**Out of scope:** the tolerance itself, which G-11 states; points, rectangles, and shapes drawn
from the cases' shapefiles; reference values for projections neither the cases in scope nor the
resolver's built-in codes use; the PROJ release a user's MapServer links; which line the maintainer
leaves out, decided case by case.

## Alternatives considered

- **One list of points read by `msProjectPoint` alone, as the decision of 2026-11-14 chose.** It
  measured the formulas and the operation PROJ picks. It cannot show the rectangle `msProjectRect`
  asks of a source, nor the cut at the horizon and the wrap at the dateline of `msProjectShape`.
  G-11 now names rectangles and shapes, so a points-only reference no longer covers the criterion.
- **The ortho and `+over` cases as more lines of `points.tsv`, as the decision of 2026-11-14
  foresaw.** A point carries no line to cut or wrap. It would have measured the formula and left
  the shape's behaviour unheld.
- **One list and one record for all three kinds.** One file fewer each, but the reader and the
  test would sort every line by its form. Three pairs keep each record next to the MapServer
  function that writes it, and let each test report its own spread.
- **One projection loaded per pair of strings for rectangles, as for points.** Faster and
  simpler. `msProjectRect` keeps `is_polar` on its source, so a rectangle's record would depend on
  the rectangles before it in the list.
- **A shape reprojected point by point through `msProjectPoint`.** No call of `msProjectShape`, but
  it records none of what that function does to a shape: the cut lines, the dropped points, the
  wrap.
- **PROJ directly, `cs2cs` from Ubuntu's `proj-bin`, on the same strings.** Simpler, and no C to
  maintain. It measures PROJ without MapServer's normalization and choice of operation, which is
  what Forager has to reproduce; a tolerance set on it would not bound the difference from
  MapServer's renderings.
- **Both, the reader as the reference and `cs2cs` as a cross-check.** Shows where MapServer's path
  departs from bare PROJ. A second tool and record for a difference no criterion asks about.
- **pyproj or PROJ in the dev container.** Neither is installed, and pyproj bundles its own PROJ,
  not the one the pinned build links; TST-noise-measured-against-mapserver keeps the dev container
  free of MapServer and its libraries.
- **Points from the cases' shapefiles.** Closer to what is drawn, but tied to the suite's data,
  large, and silent on EPSG:3857 and the UTM zones, which no case in scope uses.
- **A workflow of its own.** Keeps the measurement apart from the noise, but builds MapServer a
  second time or shares a cache across workflows; the noise run already builds and runs it.
- **Records under `crates/cli/tests/conformance/`.** Beside the label records, but the test that
  reads them is `proj`'s, which would then reach into the CLI's tree or move into the CLI.
- **Values from published worked examples alone.** `proj`'s tests already hold IOGP's examples;
  they decide the formulas, not agreement with the PROJ the reference build runs.
- **Keeping every line of a list whatever its spread.** The record stays complete, but a line at a
  singularity, such as the polar conic's rectangle, fails the test by metres that say nothing of
  what MapServer draws. The maintainer removes such a line, list and record together.

## Sources / Prior art

- MapServer at the pin, `src/mapproject.c` (`msProjectPoint`, `msProjectPointEx`,
  `msProjectRect`, `msProjectShape`, `msProjectShapeEx`, `msProjectCreateReprojector`,
  `createNormalizedPJ`) and `src/mapfile.c` (`msLoadProjectionString`), `src/mapproject.h` (their
  exports).
- [TST-noise-measured-against-mapserver](TST-noise-measured-against-mapserver.md) and its reader
  `crates/cli/tests/conformance/mapserver_labels.c`, built by `scripts/build-mapserver.sh`.
- PROJ 9.4.0, `src/4D_api.cpp` (`proj_create_crs_to_crs_from_pj`, `proj_info`), as the Ubuntu
  24.04 archive ships it (`libproj-dev`).
- [`docs/semantics/projection.md`](../semantics/projection.md), on the `is_polar` memo of
  `msProjectRect`.
- The tree at a6d7d05c: the reader, the noise module, the three lists and records, and
  `crates/proj/tests/reference.rs`; the history of the reprojection plan's tickets 03, 05, and 06
  (932a73a4, 67254a80, 556ba8a3, d963219f, e0ed507f), and its ticket 02,
  `docs/plan/reprojection/02-g11-proj4rs-against-proj.md`, deleted when its work landed.

## Consequences

- Positive: every form G-11 names is measured against what MapServer computes, the operation PROJ
  picks, the rectangle's branches, and the shape's cut and wrap included. The test runs everywhere
  without PROJ. A change of the runner's PROJ shows as a failing noise run, not as a silent drift.
- Positive: the engine and the reference run one shape function, so the test holds what is drawn.
- Positive: the reader's header and the noise run name the PROJ version and the MapServer build
  that computed each record, and reproduce it.
- Negative / trade-offs: a C reader to keep with three of MapServer's functions at the pin. Three
  lists and three records to take again by hand from the artifact after each change, as the label
  records are. The reference is as good as the archive's PROJ 9.4.0.
- Negative / trade-offs: a line the maintainer leaves out is a case the reference no longer holds.
- Follow-ups: none. The tolerance the decision of 2026-11-14 left to a measurement is G-11's, and
  the ortho and `+over` cases are lines of `shapes.tsv`.

## Enforcement

- `crates/proj/tests/reference.rs` cites G-11 and this ADR with `Verifies:` in three tests:
  - `the_core_s_reprojection_agrees_with_mapserver_s_call_of_proj_within_g_11`, over the points;
  - `the_area_asked_of_a_source_is_mapserver_s_rectangle_within_g_11`, over the rectangles;
  - `the_core_cuts_and_wraps_shapes_as_mapserver_reprojects_them_within_g_11`, over the shapes.
  Each fails as *The test* says.
- The noise module cites this ADR with `Verifies:` in two tests:
  - `mapserver_renders_the_cases_in_scope_within_its_noise` runs the reader over the three lists
    and fails as *The run* says;
  - `the_reprojection_s_output_of_points_rectangles_and_shapes_holds_its_record_within_a_millimetre`
    holds that comparison on a fixed record of each kind.
- `scripts/test-build-mapserver.sh` cites this ADR. It fails when the build script does not
  compile the reader, or compiles it inside the cached build. It also fails when the workflow
  triggers on other paths than the ones decided.
- `scripts/check-notice.sh` holds `mapserver_proj.c`, which names MapServer's sources, to
  MapServer's notice.
- **Not mechanically decidable:** two clauses are held in review, as for the label records:
  - a record is taken from the artifact and never written by hand;
  - a line is left out only on the maintainer's decision.
