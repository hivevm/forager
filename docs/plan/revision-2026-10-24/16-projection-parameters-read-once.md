# Ticket: Read a PROJECTION's parameters in one place, for the init files and for the translation

## What to build

A `PROJECTION` block's parameters are read twice. `init_file::named`
(`crates/mapfile/src/init_file.rs:39-70`) finds the init files to read before translation, and
`Translator::projection` and `Translator::resolve` (`crates/mapfile/src/translate.rs:842-915`)
read the same parameters again to build the proj4 string. Each writes its own split into
parameters, its own `+` trim, its own `init=` prefix, and its own split at the last `:`; the
comment "The code follows the last `:`, as PROJ splits it" stands at both
(`init_file.rs:55`, `translate.rs:895`). The fix to the last `:` in the review of #52 had to be
made in both. The two also disagree already: `named` collects the init file of a projection
whose first parameter is `AUTO`, which `projection` then never resolves, so that file is read
and counted toward the size bound of Q-9 for nothing.

One function reads a `PROJECTION`'s parameters into what they are: `AUTO`, or the explicit
parameters and at most one `init=` reference split into file and code. `named` and the
translation both take that, and a projection that is `AUTO` names no init file. The projection
code may move to a module of its own (`projection.rs`) beside `init_file.rs`. Behaviour does not
change otherwise.

## Acceptance criteria

- [ ] Given the existing tests of `crates/mapfile`, when they run, then they pass unchanged.
- [ ] Given a layer `PROJECTION "AUTO" "init=./data/epsg2:1" END` whose init file the loader does
  not hold, when it is loaded, then the layer's projection is `from_source` and no diagnostic
  names the init file.
- [ ] Given the tree, when the split of an `init=` reference at its last `:` is searched for,
  then it is written once.

## Read first

- [ADR-0023](../../adr/0023-projection-resolver-and-init-files.md),
  [ADR-0009](../../adr/0009-ports-and-adapters-crates.md).
- `docs/semantics/projection.md`; the convention on behaviour ported from MapServer or PROJ in
  `docs/CONVENTIONS.md`.
- `crates/mapfile/src/init_file.rs`, `crates/mapfile/src/translate.rs`, the projection tests of
  `crates/mapfile/src/tests.rs`.

## Blocked by

- None (can start immediately)

## Mode

with the human: the init files are resolved against the base context (Q-8) and counted toward
the size bound (Q-9), both kept with the maintainer ([ADR-0007](../../adr/0007-harness.md)).
