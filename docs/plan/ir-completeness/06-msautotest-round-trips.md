# Ticket: Round-trip the whole msautotest suite through YAML and JSON (G-3)

## What to build

A test over the corpus of ticket 05: for every mapfile that loads, the IR written as YAML, read
back, written as JSON, and read back equals the first IR. It carries `Verifies: G-3`, runs in CI
with ticket 05's checkout, and makes every later IR ticket keep the round trip.

## Acceptance criteria

- [ ] Given every corpus mapfile that loads, when its IR goes mapfile → IR → YAML → JSON → IR,
  then the last equals the first, and the test cites G-3.
- [ ] Given an IR field that does not round-trip, when the test runs, then it fails naming the
  file and the first differing path.

## Read first

- G-3 of `docs/SPECIFICATION.md`; [ADR-0014](../../adr/0014-style-document-serde.md).
- Ticket 05's test and fetch script; ticket 02's fuzz property.

## Blocked by

- docs/plan/ir-completeness/02-style-document-numbers-round-trip.md
- docs/plan/ir-completeness/05-msautotest-loads.md

## Mode

unattended: the test decides it; it adds a test only, undone by one revert; it touches none of the
harness's reserved areas. A wrong result shows as a red job naming the field. Stop: if a field
fails to round-trip, hand off with the field instead of changing the document format, which stays
with the maintainer.
