# ADR-0038: The reference coordinates of G-11 are reprojected by MapServer's own call of PROJ at the pinned build, in the noise run, and kept as records beside the test of `proj`

- **Status:** ⚪ superseded by ADR-draft-proj-references-points-rects-shapes
- **Date:** 2026-11-14
- **Deciders:** Markus Brigl
- **Applies to:** `crates/proj/tests/reference/` (the list of points and the records), the test of `crates/proj` that reads them, `crates/cli/tests/conformance/mapserver_proj.c`, its build in `scripts/build-mapserver.sh`, its run in `.github/workflows/noise.yml` and the harness's noise module, and the paths that start that workflow

## Context

G-11 asks that "the core's reprojection shall agree with PROJ within a tolerance stated in metres".
[ADR-0037](0037-reprojection-ported-in-proj.md) ported PROJ's formulas into `crates/proj` and left
the tolerance to a measurement against PROJ. The plan's ticket 02 asks for that measurement over
the transformations the msautotest cases in scope and the built-in codes of `adapter-epsg` use, with
the reference coordinates recorded beside the test, so that the test needs no PROJ to run, and a
script that names the PROJ version and the MapServer build that computed them.

PROJ is not in the dev container and must not enter the core (Q-1,
[ADR-0008](0008-ports-and-adapters-crates.md)). It is on the GitHub-hosted runner where
[ADR-0033](0033-noise-measurement.md) builds MapServer at the pin: Ubuntu 24.04's `libproj-dev`,
PROJ 9.4.0, which `map2img` links. There ADR-0033 already reads MapServer's label cache with a C
reader, `mapserver_labels.c`, linked to the unmodified `libmapserver`, and keeps its output as
records taken from the run's artifact, never written by hand.

What MapServer draws is not PROJ's bare formula. `msProjectPoint` (`src/mapproject.c`) goes
through `msProjectCreateReprojector` and `createNormalizedPJ`, which asks PROJ for an operation by
`proj_create_crs_to_crs_from_pj` and normalizes the axis order for visualization. Which operation
PROJ picks there, for NAD83 to WGS84 among others, is what ADR-0037's port could not verify. A
reference taken by `cs2cs` on the same strings would measure PROJ without that choice.

The maintainer chose on 2026-11-14, in four questions: MapServer's own path as the source, a fixed
list of points in the repository, the noise run as the place, and `crates/proj/tests/reference/`
for the records.

## Decision

We will compute the reference coordinates of G-11 by a C reader linked to the pinned MapServer
build, which reprojects a committed list of points through `msLoadProjectionString` and
`msProjectPoint`, in the noise run, and keep its output as records beside the test of `proj`.

- **The list.** `crates/proj/tests/reference/points.tsv` must name, line by line, a source and a
  target projection as proj4 strings, as Forager's frontend writes them, and the points to
  reproject from the source. It must cover the transformations of the cases in scope, EPSG:4326 to
  and from EPSG:3857 and each built-in UTM zone that a point names. Its strings are those the
  resolver gives, not `init=` references, so that MapServer and `proj` read the same text.
- **The reader.** `crates/cli/tests/conformance/mapserver_proj.c` must load each pair with
  `msInitProjection` and `msLoadProjectionString` and reproject each point with `msProjectPoint`,
  as MapServer reprojects a shape's point. It writes one line per point: the line of the list, the
  point, and the result with `%.17g`, or a mark where `msProjectPoint` fails. Its first lines name
  the PROJ release (`proj_info().release`) and the MapServer commit it was built from.
- **Its build.** `scripts/build-mapserver.sh` must compile it as it compiles `mapserver_labels.c`:
  on every run, outside the cached build, against the unmodified `libmapserver` and headers, with
  the same compiler, include directories, and library.
- **The run.** The noise run must run the reader over the list and leave its output in the
  artifact, `noise/proj/`. It must fail when the output and the record differ in any line, the
  header included, or a coordinate by more than 1 mm, and when the list holds a line the record
  lacks or the record one the list lacks.
- **The records.** `crates/proj/tests/reference/mapserver.tsv` must be taken from the artifact and
  never written by hand. It is taken again when the list changes or the runner's PROJ moves.
- **The test.** A test of `crates/proj` must read the list and the record and hold each point's
  reprojection by `proj` within the tolerance G-11 states, a failing point to a failing one. It
  must need no PROJ and no MapServer.
- **The trigger.** A push that changes `crates/proj/tests/reference/` or the reader must start the
  noise workflow, beside the paths ADR-0033 names.

**Out of scope:** the tolerance and its measure in metres, which the specification states once the
measurement is taken; points drawn from the cases' shapefiles; reference values for projections
the cases in scope do not use; the ortho and `+over` cases of the plan's late tickets, which add
lines to the list when they come.

## Alternatives considered

- **PROJ directly, `cs2cs` from Ubuntu's `proj-bin`, on the same strings.** Simpler, and no C to
  maintain. It measures PROJ without MapServer's normalization and choice of operation, which is
  what Forager has to reproduce; a tolerance set on it would not bound the difference from
  MapServer's renderings.
- **Both, the reader as the reference and `cs2cs` as a cross-check.** Shows where MapServer's
  path departs from bare PROJ. A second tool and record for a difference no criterion asks about.
- **pyproj or PROJ in the dev container.** Neither is installed, and pyproj bundles its own PROJ,
  not the one the pinned build links; the dev container is kept free of MapServer and its
  libraries by ADR-0033.
- **Points from the cases' shapefiles.** Closer to what is drawn, but tied to the suite's data,
  large, and silent on EPSG:3857 and the UTM zones, which no case in scope uses.
- **A workflow of its own.** Keeps the measurement apart from the noise, but builds MapServer a
  second time or shares a cache across workflows; the noise run already builds and runs it.
- **Records under `crates/cli/tests/conformance/`.** Beside the label records, but the test that
  reads them is `proj`'s, which would then reach into the CLI's tree or move into the CLI.
- **Values from published worked examples alone.** `proj`'s tests already hold IOGP's examples;
  they decide the formulas, not agreement with the PROJ the reference build runs.

## Sources / Prior art

- MapServer at the pin, `src/mapproject.c` (`msProjectPoint`, `msProjectPointEx`,
  `msProjectCreateReprojector`, `createNormalizedPJ`) and `src/mapfile.c`
  (`msLoadProjectionString`), `src/mapproject.h` (their exports).
- [ADR-0033](0033-noise-measurement.md) and its reader
  `crates/cli/tests/conformance/mapserver_labels.c`, built by `scripts/build-mapserver.sh`.
- PROJ 9.4.0, `src/4D_api.cpp` (`proj_create_crs_to_crs_from_pj`, `proj_info`), as the Ubuntu
  24.04 archive ships it (`libproj-dev`).
- The plan's ticket 02, `docs/plan/reprojection/02-g11-proj4rs-against-proj.md`.

## Consequences

- Positive: G-11 is measured against what MapServer computes, the operation PROJ picks included,
  and the test runs everywhere without PROJ. A change of the runner's PROJ shows as a failing noise
  run, not as a silent drift. The criterion that a script names the PROJ version and the build and
  reproduces the file is met by the reader's header and the noise run.
- Negative / trade-offs: a second C file to keep with MapServer's API at the pin. A point list and
  a record to take again by hand from the artifact after each change, as the label records are.
  The reference is as good as the archive's PROJ 9.4.0, not the PROJ release a user's MapServer
  may link.
- Follow-ups:
  - **The tolerance.** The measured spread goes to the maintainer, who states G-11's tolerance as a
    change to the specification.
  - **The late tickets.** The ortho and `+over` transformations add lines to the list.

## Enforcement

- The test of `crates/proj` that reads `crates/proj/tests/reference/` cites G-11 and this ADR with
  `Verifies:`. It fails on a point outside the tolerance, on a point that transforms on one side
  only, and on a list and a record that do not line up.
- The noise module's test of the reader cites this ADR with `Verifies:`; it fails where the
  reader's output and the record differ as the Decision says.
- `scripts/check-notice.sh` holds `mapserver_proj.c`, which names MapServer's sources, to
  MapServer's notice.
- **Not mechanically decidable:** that a record is taken from the artifact and never written by
  hand is held in review, as for the label records.
