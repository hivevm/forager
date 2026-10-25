# Ticket: Decide the crate that reads shapefiles

## What to build

A `proposed` ADR, through `propose-adr`, that chooses the crate the shapefile adapter reads
`.shp`, `.shx`, and `.dbf` with, in its own pull request. The `shapefile` crate of the
specification's Assumptions is the first candidate; the ADR verifies that it builds for
`wasm32-unknown-unknown` without C code, that it reads from bytes in memory rather than from the
file system, its licence, and its maintenance, and weighs the alternatives, a reader of Forager's
own among them.

## Acceptance criteria

- [ ] Given the ADR, when `scripts/check-docs.sh` runs, then it is green, with the index row and
      an Enforcement naming the adapter test that will cite it.
- [ ] Given the chosen crate, when it is built for `wasm32-unknown-unknown` in a scratch crate,
      then the build succeeds and `cargo tree` names no `-sys` crate; the ADR records the result.
- [ ] Given the chosen crate, when `cargo deny check` runs over it in that scratch crate, then
      its licences and advisories pass; the ADR records the result.

## Read first

- ADR-0016 and ADR-0017, the shape of a crate-choice ADR; ADR-0009, ADR-0012; Q-1, Q-3, Q-6, and
  the Assumptions of the specification.

## Blocked by

- None (can start immediately)

## Mode

with the human: it is an ADR, which the maintainer decides ([ADR-0007](../../adr/0007-harness.md)).
