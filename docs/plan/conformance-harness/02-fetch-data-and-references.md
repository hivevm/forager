# Ticket: Fetch the suite's data and reference images

## What to build

The fetch of the msautotest suite takes, beyond the mapfiles, the data files and the PNG reference
images of the `misc` and `renderers` directories, at the same pinned commit, so that a test can
render a case and open its reference. The cairo references are left out.

## Acceptance criteria

- [ ] Given a fresh cache, when the suite is fetched, then each `misc` and `renderers` case's
      shapefiles and its non-cairo `expected/*.png` are present under the suite's directory.
- [ ] Given a cache from before this change, when the suite is fetched, then the sparse checkout
      widens to the new paths without a second clone.
- [ ] Given the pin, when the fetch runs, then it still refuses a tag that names another commit.
- [ ] Given the CI cache of the suite, when the fetch's patterns change, then the cache key changes
      with them.

## Read first

- The header of the fetch script and the CI job that caches its result; the Constraint on the
  pinned reference in the specification.
- The seam is the script and the G-1 test, which must stay green.

## Blocked by

- None (can start immediately)

## Mode

unattended: a test decides it (the G-1 test still runs, and a check that the reference files of
two named cases exist); the scope is one script and one cache key, undone by one revert; it
touches nothing ADR-0007 keeps with the maintainer. A wrong result shows as a missing file in the
harness. Stop after two attempts at a sparse-checkout pattern that does not take, and leave the
patterns tried here.
