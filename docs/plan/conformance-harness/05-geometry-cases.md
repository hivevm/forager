# Ticket: Render the geometry cases against their references

## What to build

The harness: an integration test that renders each case of the scope list through the CLI with
the suite's directory as the base context, compares it with its reference image by the metric,
and holds the outcome the list expects for it. The list names, for `misc` and `renderers`, every
case: in scope with the category geometry and symbols, or out of scope with a reason. Until
ticket 04 lands, the threshold and tolerance are provisional and say so in the list.

## Acceptance criteria

- [ ] Given the fetched suite, when the harness runs, then each in-scope case is rendered and
      compared, and a case whose outcome differs from the list's fails the test with its share.
- [ ] Given a case of `misc` or `renderers` that the list does not name, when the harness runs,
      then it fails and names the case, so that no case is dropped silently.
- [ ] Given a failing case, then its rendering and difference image are left in the test's output
      directory.
- [ ] Given the suite is not fetched, when `cargo test` runs, then the harness is skipped as the
      G-1 test is.
- [ ] The test cites G-6 with a `Verifies:` marker for the category geometry and symbols.

## Read first

- The G-1 test of the CLI (its skip, its list of skipped files with reasons) is the golden path.
- G-6 and the spec's *Scope* decision; the glossary's *msautotest*.

## Blocked by

- docs/plan/conformance-harness/01-shapefile-source.md
- docs/plan/conformance-harness/02-fetch-data-and-references.md
- docs/plan/conformance-harness/03-image-metric.md

## Mode

unattended: the test decides every criterion; the change is the test and its list, undone by one
revert; it touches nothing ADR-0007 keeps with the maintainer. A wrong outcome shows as a case
whose rendering contradicts the list. Stop when a case would pass only by a change outside the
tests, and list it as out of scope with that reason instead.
