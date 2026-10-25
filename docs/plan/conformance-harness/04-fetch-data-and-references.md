# Ticket: Fetch the suite's data and reference images

## What to build

The fetch of the msautotest suite takes, beyond the mapfiles, the shapefiles that the cases of
`misc` and `renderers` read, from `misc/data`, `query/data`, and `wxs/data`, and the non-cairo
PNG references under `misc/expected` and `renderers/expected`, at the same pinned commit.

## Acceptance criteria

- [ ] Given a fresh cache, when the suite is fetched, then `renderers/expected/line_simple.png`
      and the `.shp`, `.shx`, and `.dbf` of `misc/data/world_testlines` are present, and so is a
      shapefile under `query/data` and one under `wxs/data` that a `renderers` mapfile names.
- [ ] Given the fetched suite, then no `*.cairo.png` is present.
- [ ] Given the pin, when the fetch runs, then it still refuses a tag that names another commit,
      and the G-1 test stays green.

## Read first

- The header of the fetch script and the CI job that caches its result; the Constraint on the
  pinned reference in the specification.

## Blocked by

- None (can start immediately)

## Mode

unattended: a test decides every criterion; one script, undone by one revert; nothing ADR-0007 keeps
with the maintainer. A wrong result shows as a missing file in the harness. It counts against the
cap of three open agent branches of [ADR-0007](../../adr/0007-harness.md), and is dispatched only
while that cap allows. Stop after two attempts at a sparse-checkout pattern that does not take, and
leave the patterns tried here.
