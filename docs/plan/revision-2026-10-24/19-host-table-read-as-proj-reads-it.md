# Ticket: Read the host's EPSG table as PROJ reads the init file format

## What to build

ADR-0023 has the frontend and `adapter-epsg` each parse the format of PROJ's former `epsg` init
file, and accepts the duplication as a trade-off. The review of #52 brought the frontend's reader
(`crates/mapfile/src/init_file.rs`, `parse`) to PROJ's `get_init_string`; the adapter's
(`crates/adapter-epsg/src/lib.rs`, `with_table` and `parse_line`) was left as it was, so the two
readers of one format now disagree:

- a code defined twice in a table: the frontend keeps the first, as PROJ does; the adapter keeps
  the last (`entries.insert`), silently;
- a definition across lines: the frontend reads it up to the next `<`, as PROJ does; the adapter
  reads line by line, refuses the continuation line as "a definition starts with <code>", and
  with it the whole table.

The grid and `init=` refusal is also written twice, word for word apart from the code's name
(`init_file.rs`, `usable`; `lib.rs`, `parse_line`).

The adapter reads its table as the frontend reads an init file: the first definition of a code
within the table holds, and a definition runs across lines to the next `<`; a table's code still
overrides a built-in one. Its errors keep their line numbers, the line where a definition starts.
Both crates' tests run one list of format cases — a duplicate, a definition across lines, a
comment inside one, a BOM, a grid, an `init=` — so that a third divergence fails a test. The
duplication itself stays, as ADR-0023 decides.

## Acceptance criteria

- [ ] Given a table `<1> +proj=longlat <>` followed by `<1> +proj=merc <>`, when the adapter
  resolves `epsg:1`, then it gets `+proj=longlat`.
- [ ] Given a table whose definition of `<1>` runs over two lines, when the adapter resolves
  `epsg:1`, then it gets both lines' parameters and no error.
- [ ] Given the list of format cases, when the frontend's and the adapter's tests run it, then
  each yields the same definitions and refusals.

## Read first

- [ADR-0023](../../adr/0023-projection-resolver-and-init-files.md),
  [ADR-0009](../../adr/0009-ports-and-adapters-crates.md); the convention on behaviour ported from
  MapServer or PROJ in `docs/CONVENTIONS.md`.
- `crates/adapter-epsg/src/lib.rs`, `crates/mapfile/src/init_file.rs`, the init file tests of
  `crates/mapfile/src/tests.rs`; PROJ 9.5.0, `get_init_string` in `src/init.cpp`.

## Blocked by

- None (can start immediately)

## Mode

with the human: the bound on a table's reported errors is a resource limit of Q-9, and what the
`--epsg` file and the bindings' table argument accept is part of the browser bindings' API, both
kept with the maintainer ([ADR-0007](../../adr/0007-harness.md)).
