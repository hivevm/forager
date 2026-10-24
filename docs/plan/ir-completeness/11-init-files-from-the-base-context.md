# Ticket: Resolve an `init=` that names a file inside the base context

## What to build

PROJ reads `init=<file>:<code>` from a file, and mapfiles name such files by a path relative to
the mapfile: 39 mapfiles of msautotest name `init=./data/epsg2:42304`, the suite's own
`wxs/data/epsg2`. Today the path is the resolver's authority, which no resolver knows, so these
mapfiles fail to load; the G-1 test loads them with a stand-in for the file
(`crates/cli/tests/msautotest.rs`, `INIT_FILE`). The frontend reads such a file through the
resource loader, inside the base context (Q-8) and within the size limit of Q-9, parses it as the
host table of ADR-0018 is parsed, and resolves the code from it; the stand-in goes.

## Acceptance criteria

- [ ] Given a layer with `PROJECTION "init=./data/epsg2:42304" END` and that file beside the
  mapfile, when it loads without a resolver, then the layer's projection is the file's definition.
- [ ] Given such a file outside the base context, or a code it does not hold, when it loads, then
  it is the load error `projection-unresolved`, naming the file and the code.
- [ ] Given the msautotest suite, when G-1 runs, then every mapfile loads through the CLI with no
  stand-in for the init file.

## Read first

- [ADR-0018](../../adr/0018-projection-resolver.md), which decides `init=` through the resolver
  port and needs amending for files the mapfile names; the spec next to this ticket.
- `docs/semantics/projection.md`; `projection` in `crates/mapfile/src/translate.rs`; PROJ's
  `pj_init` file lookup.

## Blocked by

- None (can start immediately)

## Mode

with the human: it amends ADR-0018 and resolves a resource path
([ADR-0007](../../adr/0007-harness.md)).
