# Ticket: Hold the engine's modules to one direction

## What to build

From `docs/backlog/engine-modules-import-each-other.md`. Two modules of the engine import each
other: `labels` uses `label_styles`, and `label_styles` calls `crate::labels::push` and
`crate::labels::make_room` and takes `crate::labels::LabelAt`. Its modules also take helpers from
the crate's root (`layer_opacity`, `points_in_view`, `quoted`, `Selected`, `draw_items`,
`refused_pixmap`), which leaves no leaf module for them. The maintainer chose on 2026-11-12 that no
ADR is due: a crate holds the order of its modules by a test of its own, as `symbolize` does. Break
the cycle, by moving `push`, `make_room`, and `LabelAt` where tickets 07 to 09 leave them, and the
root's helpers into a leaf module; state the direction in the engine's `lib.rs`; and hold it by a
test beside `crates/arch-test/tests/symbolize_modules.rs`, sharing its reading of the imports. No
behaviour changes.

## Acceptance criteria

- [ ] Given the engine at the change, when the arch test runs, then it passes, and given an import
  of `labels` added to `label_styles`, then it fails naming the two modules.
- [ ] Given the suite at the base, when the modules are moved, then it passes with every expected
  value unchanged.

## Read first

- Touches: `crates/engine/src/`, `crates/arch-test/tests/symbolize_modules.rs`,
  `crates/arch-test/tests/engine_modules.rs`, `docs/ARCHITECTURE.md`
- `docs/adr/0008-ports-and-adapters-crates.md`; the `symbolize` rule in
  `crates/symbolize/src/lib.rs`.

## Blocked by

- docs/plan/revision-2026-11-13/09-refusals-one-value.md

## Mode

unattended: the arch test and the suite's expected values decide it; one revert. Stop when a test's
expected value moves, or when the direction asks a module the engine does not have a reason for.
