# Ticket: `arch-test`'s checks share one root and one walk of the tree, and the crate says what it holds

## What to build

`crates/arch-test` holds six test binaries under `tests/`, `engine_modules.rs` and
`symbolize_modules.rs` sharing `module_direction/mod.rs`, but its `src/lib.rs` reads "No code: this
crate holds the dependency test of ADR-0008 under `tests/`", and its `Cargo.toml` description
says the same. Ticket 15 of revision-2026-11-16 added a `manifest_dir()` to each test binary.
Five of the copies are in `arch-test` (`c_numbers.rs`, `dependencies.rs`, `manifest_dir.rs`,
`symbol_names.rs`, `module_direction/mod.rs`). Four `rust_files` walkers sit beside them:

- `c_numbers.rs`, `symbol_names.rs`, and `manifest_dir.rs` skip `target` and ignore what they
  cannot read;
- `module_direction/mod.rs` skips nothing, unwraps, and strips the root.

The duplication sensor lists the walkers of `c_numbers.rs` and `symbol_names.rs`, and of
`manifest_dir.rs` and `c_numbers.rs`, as clones.

`arch-test`'s `src/lib.rs` holds one `manifest_dir()` and one walk of the `.rs` files under a
directory, `target` left out, for its own tests, which need no new edge of ADR-0008. Where
`module_direction` needs its different reading, the shared walk takes it as a parameter, or the
difference is named in its doc. The lib's doc and the `Cargo.toml` description name the checks
the crate holds. The other crates' copies of `manifest_dir()` stay, under the check of
`manifest_dir.rs`: sharing them would need a crate and an edge ADR-0008 does not allow.

## Acceptance criteria

- [ ] Given `crates/arch-test/tests`, when it is searched for `fn manifest_dir` and
  `fn rust_files`, then neither is defined there.
- [ ] Given `scripts/sensor-duplication.sh`, when it runs, then it lists no clone between the
  test files of `crates/arch-test`.
- [ ] Given every test of `forager-arch-test`, `the_scan_reads_the_tree_of_the_checkout_it_runs_in`
  among them, when they run, then all pass, no assertion changed.

## Read first

- Touches: `crates/arch-test/src/lib.rs`, `crates/arch-test/Cargo.toml`,
  `crates/arch-test/tests/*.rs`, `crates/arch-test/tests/module_direction/mod.rs`,
  `docs/ARCHITECTURE.md` (the `arch-test` entry)
- ADR-0008; the merge message cb1f20fc (why each binary has a `manifest_dir()`).

## Blocked by

- docs/plan/revision-2026-11-18/06-symbolize-module-docs-and-table-match-the-code.md

## Mode

unattended: the arch tests decide it, and a wrong result costs one revert. Stop when the shared
walk changes which files a check reads, measured by the files each check saw before; hand off
with the difference.
