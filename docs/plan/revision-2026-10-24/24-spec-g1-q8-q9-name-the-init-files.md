# Ticket: Draft — G-1, Q-8, and Q-9 name the init files and the resolver the work added

## What to build

A specification change for the human to decide ([`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules),
rule 7). Since the spec `ir-completeness`, the texts of three criteria no longer say what their
tests decide:

- **G-1** names a resource loader for `INCLUDE`, `SYMBOLSET`, and `FONTSET`. The test
  (`crates/cli/tests/msautotest.rs`) also serves the init files a projection names by a path,
  hands the CLI a projection resolver with stand-in definitions for the eleven EPSG codes beyond
  the built-in ones and, in-process, for the init file named bare; and it skips thirteen files —
  include fragments, one malformed on purpose, one binary — each named with its reason.
- **Q-8** lists `INCLUDE`, `SYMBOLSET`, `FONTSET`, `SHAPEPATH`, and pixmap paths. ADR-0023 adds
  the init files named by a path, and the frontend's tests refuse one outside the base context.
- **Q-9** bounds "the total size of a mapfile with its `INCLUDE`s"; the frontend counts the
  symbol file, the font set, and the init files into the same bound.

Drafted text:

> **G-1** — When a mapfile of the msautotest suite is loaded with a resource loader for its
> `INCLUDE`, `SYMBOLSET`, and `FONTSET` files and the init files it names by a path, and with a
> projection resolver for the codes it names by an authority, the system shall produce a style IR
> and no diagnostic of severity error; warnings are allowed. Files of the suite that are not
> mapfiles — include fragments, malformed and binary test data — are named by the test with a
> reason each.

> **Q-8** — Containment: `INCLUDE`, `SYMBOLSET`, `FONTSET`, `SHAPEPATH`, pixmap paths, and the
> init files a projection names by a path resolve only inside the base context the host gives.
> …(rest unchanged)

> **Q-9** — … the total size of a mapfile with its `INCLUDE`s and the files it names (16 MiB of
> text) … (rest unchanged)

Accepted ADRs whose basis the change may move, for the human to confirm rather than for this
draft to decide: [ADR-0023](../../adr/0023-projection-resolver-and-init-files.md) (it decides the
resolver, the init files, and their bounds, and its Enforcement cites the G-1 test);
[ADR-0007](../../adr/0007-harness.md) (it keeps Q-8 and Q-9 with the maintainer, so their scope
grows).

## Acceptance criteria

- [ ] Given the human's decision, when the specification change lands, then G-1, Q-8, and Q-9
  read as decided, and each ADR it names is left as is or superseded in a pull request of its
  own.
- [ ] Given the change, when `scripts/check-all.sh` runs, then the traceability check still finds
  the tests that cite G-1, Q-8, and Q-9.

## Read first

- `docs/SPECIFICATION.md` (G-1, Q-8, Q-9),
  [ADR-0023](../../adr/0023-projection-resolver-and-init-files.md).
- `crates/cli/tests/msautotest.rs`, `crates/mapfile/src/lib.rs` (`Reading`).

## Blocked by

- None (can start immediately)

## Mode

with the human: the specification is the human's; an agent drafts and never lands it.
