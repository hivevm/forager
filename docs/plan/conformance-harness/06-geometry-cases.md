# Ticket: Render the geometry cases against their references

## What to build

The harness: an integration test that renders each case of the scope list in-process, with the
suite's directory as the base context, compares it with its reference by the metric, and holds
the outcome the list expects. A case is one `RUN_PARMS` line as the spec defines it. The list
names every case of `misc` and `renderers`: in scope with the category geometry and symbols, or
out of scope with a reason. Cases with a `map2img` option are out of scope here, for ticket 07.
The threshold and tolerance are provisional, say so in the list, and the test cites no criterion
yet.

## Acceptance criteria

- [ ] Given the fetched suite, when the harness runs, then each in-scope case is rendered and
      compared, and a case whose outcome differs from the list's fails the test with its share.
- [ ] Given a `RUN_PARMS` line of `misc` or `renderers` that the list does not name, when the
      harness runs, then it fails and names the case, so that no case is dropped silently.
- [ ] Given a failing case, then its rendering and difference image are left in the test's output
      directory.
- [ ] Given `cargo test` alone, then the harness is ignored as the G-1 test is; given the Rust
      checks, then it runs after the fetch.

## Read first

- The G-1 test of the CLI (its ignore, its list of skipped files with reasons) is the golden path.
- G-6 and the spec's *Case* and *Scope* decisions; the glossary's *msautotest*.
- docs/plan/revision-2026-10-24/25-validation-defaults-at-load.md: a case whose `DATA` holds a
  `%name%` is out of scope until it lands.

## Blocked by

- docs/plan/conformance-harness/03-shapefile-from-data.md
- docs/plan/conformance-harness/04-fetch-data-and-references.md
- docs/plan/conformance-harness/05-image-metric.md

## Mode

unattended: the test decides every criterion; the test and its list, undone by one revert; nothing
ADR-0007 keeps with the maintainer. A wrong outcome shows as a case whose rendering contradicts the
list. It counts against the cap of three open agent branches of
[ADR-0007](../../adr/0007-harness.md), and is dispatched only while that cap allows. Stop when a
case would pass only by a change outside the tests, and list it as out of scope with that reason
instead.
