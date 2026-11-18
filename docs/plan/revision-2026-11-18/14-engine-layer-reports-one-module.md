# Ticket: The engine's reports of a layer are written in one module

## What to build

The engine turns what a layer left undrawn into its diagnostics in two places. Its root,
`crates/engine/src/lib.rs` (1292 lines, the largest change of this revision's scope), writes most
of them:

- `layer_reports`, `style_reports`, `style_tree_reports`, and `described`;
- `skipped`, `unknown_symbols`, `styles_not_drawn`, and `rotation_without_projection`;
- `transform_not_drawn`, `transform_failed`, and `kind`.

`layer.rs`, whose doc lists eight things the root and its modules share, writes the rest:
`refusal_reports`, `renderer_refusals`, `refused_hatches`, `plural`, `name`, and `quoted`. The
labels call `refusal_reports` too, and a module may not take an item of the root
(`engine_modules.rs`), so it lies in `layer.rs`. Ticket 11 of revision-2026-11-18 put
`renderer_refusals`, which only the root calls, beside it (f6fd7a9d), and the reports only the root
calls before it stayed in the root. Where a report is written follows who calls it, not what it is.

The diagnostics of a layer are written in one module of the engine, a leaf of its module
direction, and the root only calls it. Each code, text, severity, and order of the diagnostics
stays as it is. `layer.rs` keeps what is not a report.

## Acceptance criteria

- [ ] Given `crates/engine/src/lib.rs`, when it is read, then it constructs no `Diagnostic` of a
  layer's drawing but by calling the new module, and the functions listed above are not defined
  there.
- [ ] Given every test of the workspace and the conformance harness, when they run, then all
  pass, no assertion changed.
- [ ] Given `crates/arch-test/tests/engine_modules.rs`, the module list of `lib.rs`, and the
  engine's entry in `docs/ARCHITECTURE.md`, when they are read, then they name the new module in
  its place in the direction.

## Read first

- Touches: `crates/engine/src/lib.rs`, `crates/engine/src/layer.rs`, a new module under
  `crates/engine/src/`, `crates/engine/src/labels.rs` (a caller of `refusal_reports`),
  `crates/arch-test/tests/engine_modules.rs`, `docs/ARCHITECTURE.md` (the `engine` entry)
- ADR-0008, ADR-0022, ADR-0049 (*Enforcement*: the engine's tests of each refusal's warning); the
  convention *A commit typed `refactor` changes nothing a test can see*.

## Blocked by

- None (can start immediately)

## Mode

unattended: the existing tests decide it, the change preserves every interface outside the crate,
and a wrong result costs one revert. Stop when a report cannot move without a module taking an
item of the root, or without a change of a diagnostic's order; hand off with the report and why.
