# ADR-0027: CI builds MapServer from the pinned tag to measure its noise

- **Status:** 🟡 proposed
- **Date:** 2026-10-29
- **Deciders:** Markus Brigl
- **Applies to:** `scripts/build-mapserver.sh`, `.github/workflows/noise.yml`, and every run of MapServer itself

## Context

G-6 holds a rendering to its msautotest reference within the tolerance of its category, and the
specification assumes that the references are reproducible enough for those tolerances to lie
above their platform noise, measured before the first rendering criterion is held. The noise is
how far MapServer itself, run off the platform that produced the references, lands from them; so
the measurement needs MapServer at the release the suite is pinned to, `rel-8-6-6` at the commit
`scripts/fetch-msautotest.sh` names. Nothing of the product links it: it is a measuring
instrument, run over the in-scope cases, whose renderings the harness compares by its metric.

MapServer is C with C dependencies (PROJ, GDAL, FreeType, libpng, Cairo), which the dev container
does not carry; adding them would grow the container of every session for a job that runs only
when its inputs change. The run depends on MapServer, the scope list, and the harness that
compares, not on the rest of Forager's code. MapServer's own CI builds the tag from source in an
`ubuntu:24.04` container, with Ubuntu's packages, the ubuntugis-unstable PPA, and Kitware's CMake
repository, and runs the suite there.

## Decision

We will build MapServer from the commit `scripts/fetch-msautotest.sh` names, with its default
build options and the Ubuntu archive's packages that `scripts/build-mapserver.sh` names, on a
GitHub-hosted `ubuntu-24.04` runner in the workflow `.github/workflows/noise.yml`, run on demand
and on a push that changes `crates/cli/tests/conformance/`, `scripts/fetch-msautotest.sh`,
`scripts/build-mapserver.sh`, or the workflow, and nowhere else.

The build script reads the tag and the commit from the two assignments in
`scripts/fetch-msautotest.sh`, the one place that names the pin, checks out that commit and
refuses to build when its checkout is another, adds no apt source, and refuses to run outside
GitHub Actions, so it never runs in the dev container. The run writes into its job summary the
CMake options of the build and, for each package it installed, the version and the origin
`apt-cache policy` reports. The workflow caches the build by the hash of the build script and of
`scripts/fetch-msautotest.sh`, and compares the renderings in the same run.

**Out of scope:** the set of thresholds, the report, and the margin that sets G-6's numbers, which
ticket 08 of the conformance harness and the maintainer's specification change decide; running
MapServer on another platform; other categories than geometry and symbols.

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
- **Building in the dev container** — the change ADR-0010 would have to take, for a job that runs
  rarely.
- **On every push** — a build and a run per push for a result that only the pin, the scope, and
  the harness change.

## Sources / Prior art

- MapServer at `rel-8-6-6`: `.github/workflows/build.yml` (runs `start.sh` in `ubuntu:24.04`),
  `.github/workflows/start.sh`, `ci/ubuntu/setup.sh` (Ubuntu packages, the ubuntugis-unstable
  PPA, and Kitware's repository), `ci/ubuntu/build.sh`, the `cmakebuild_nocoverage` target of
  its `Makefile`, and the option defaults of `CMakeLists.txt`; read 2026-10-29.
- conda-forge `mapserver`, latest 8.6.6 (anaconda.org API, checked 2026-10-29).
- Docker Hub `camptocamp/mapserver` tags `8.6`, `8.6-gdal3.12`, `8.6-gdal3.13`, `8.6.0`, no
  `8.6.6` (checked 2026-10-29).
- Ubuntu noble `mapserver` 8.0.1-4ubuntu2 (Launchpad, checked 2026-10-29).

## Consequences

- Positive: the instrument is the pinned release, built from the commit the suite comes from,
  with no binary from a third party; moving the pin rebuilds it in the same change.
- Positive: nothing of it reaches the dev container or the checks of every push.
- Negative: Ubuntu's packages float with the archive and the runner image, so two runs months
  apart may link other versions; the job summary names them, so a shift in the noise can be
  traced.
- Negative: every change that flips an outcome in the scope list runs it; the cached build keeps
  that to the rendering and the comparison.
- Negative: an uncached build takes minutes, and the runner label must be moved by hand when
  GitHub retires `ubuntu-24.04`.
- Negative: whether the pinned release builds against the archive's GDAL and PROJ is known only
  once the first run succeeds.
- Follow-ups: whether a weekly run is worth its cost, should the archive's drift show in the noise.

## Enforcement

`scripts/test-build-mapserver.sh`, run by `scripts/check-all.sh`, cites this ADR with
`Verifies:`. It fails when `scripts/build-mapserver.sh` names a tag or a commit of its own
instead of reading the assignments of `scripts/fetch-msautotest.sh`; when the script, run without
`GITHUB_ACTIONS=true`, does not refuse; when the script or the workflow adds an apt source
(`add-apt-repository`, `apt-add-repository`, or a write under `/etc/apt/sources.list`); when the
workflow runs on another runner than `ubuntu-24.04`, uses `container:` or `docker`, triggers on
anything but `workflow_dispatch` and a push filtered to the paths of the Decision, or keys its
cache on anything but the hash of the two scripts.

**Not mechanically decidable:** that the build keeps MapServer's default options and that the
job summary lists the packages with their origins, short of a run; a reviewer reads the script
and the first run's summary.
