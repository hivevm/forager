# Ticket: Every proj4 string the .prj reader writes parses in proj

## What to build

ADR-0039 holds the `.prj` reader to "what `proj` implements". Nothing holds the two ends equal:
`method_proj4` (`crates/adapter-shapefile/src/prj.rs:330-414`) writes a proj4 string per branch,
and only one end-to-end case (`crates/cli/tests/shapefile.rs:394`) feeds one to `proj`. The adapter
may not depend on `proj` under ADR-0008. A test in a crate that may reach both, such as the CLI's,
reads a `.prj` fixture per branch of `method_proj4`. It parses each string with
`proj::Projection::parse`, and asserts that it reads.

## Acceptance criteria

- [ ] Given a `.prj` fixture for each branch of `method_proj4`, when the test runs, then each
  declared proj4 string parses in `proj`.
- [ ] Given a `.prj` of each projection name `method_proj4` matches, the names read from its
  `match`, when the test runs, then the test holds a fixture for each name and fails naming one it
  lacks.

## Read first

- Touches: `crates/adapter-shapefile/src/prj.rs`, `crates/cli/tests/shapefile.rs` or a new test
  file beside it, `crates/adapter-shapefile/tests/` fixtures
- `docs/adr/0039-declared-projection.md`, `docs/adr/0008-ports-and-adapters-crates.md`.

## Blocked by

- None (can start immediately)

## Mode

unattended: the test decides it; one revert. Stop when a branch's string does not parse. That is a
defect for its own ticket, not this one's to fix. It touches nothing ADR-0006 keeps with the human,
and it counts against ADR-0006's cap of three open agent branches.
