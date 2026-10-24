# Ticket: Load the whole msautotest suite in CI without an error (G-1)

## What to build

A test that loads every mapfile of the msautotest suite at MapServer `rel-8-6-6` with a resource
loader for its includes, symbol sets, and font sets, and with the built-in EPSG resolver plus a
fixture table for the codes outside it, and fails on any diagnostic of severity error. The suite
comes from a pinned sparse checkout of the MapServer repository, cached in CI and fetched by one
script locally, never vendored. Files that are not standalone mapfiles are named in the test,
each with its reason. It carries `Verifies: G-1` and runs in CI on every push.

## Acceptance criteria

- [ ] Given the pinned checkout, when the test runs, then every mapfile not named as skipped
  loads with no error diagnostic, and the test cites G-1.
- [ ] Given a skipped file, when the test lists skips, then each has a reason: include fragment,
  malformed on purpose, binary test data, or an include outside the checkout.
- [ ] Given a change that makes one corpus mapfile fail, when CI runs, then the job fails naming
  the file and the diagnostic.
- [ ] Given a second CI run, when the cache is warm, then no clone happens; given a new pin, the
  cache key changes.
- [ ] Given the fixture table, when it is read, then it holds only the codes the corpus needs, and
  it lives with the test, not in the product (ADR-0018).

## Read first

- G-1 of `docs/SPECIFICATION.md`; [ADR-0018](../../adr/0018-projection-resolver.md),
  [ADR-0008](../../adr/0008-rust-toolchain.md),
  [ADR-0011](../../adr/0011-action-references-major-tag.md),
  [ADR-0012](../../adr/0012-crate-supply-chain.md).
- `.scratch/msautotest-survey/survey.sh` (the checkout, if present) and `REPORT.md`; the
  spec next to this ticket (*Testing decisions*).
- MapServer's licence (MIT-style, OSGeo/UMN) for the note on the fetched corpus.

## Blocked by

- docs/plan/ir-completeness/03-include-base-context.md
- docs/plan/ir-completeness/04-attribute-bindings-and-colour-range.md

## Mode

unattended: the test decides it (the corpus loads, or names what fails); its scope is a test, a
fetch script, and a CI job, undone by one revert; it touches none of the harness's reserved
areas. A wrong result shows as a red or a falsely green job; the skip list with reasons is what a
reviewer checks. Stop: if a corpus file fails for a cause outside tickets 03 and 04, or the skip
list grows past the 13 files the survey named, hand off with the list instead of widening the IR.
