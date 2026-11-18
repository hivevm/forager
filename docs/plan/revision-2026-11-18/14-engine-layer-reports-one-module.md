# Ticket: The engine's reports of what a drawn layer left undrawn are written in one module

## What to build

After a layer is drawn, the engine turns what it left undrawn into the layer's diagnostics
(`layer_reports` and `style_reports`, called one after the other in `draw_layer`). The functions
that write those reports lie in two files.

In the root, `crates/engine/src/lib.rs` (1292 lines, the largest change of this revision's scope):

- `layer_reports` and `style_reports`, the two entries;
- `style_tree_reports`, `unknown_symbols`, and `styles_not_drawn`, which `style_reports` calls;
- `described`, the text of a `NotDrawn` reason, which `styles_not_drawn` and
  `transform_not_drawn` share;
- `transform_not_drawn` and `transform_failed`, the reports of a layer's own transform tree, which
  `transformed` calls.

In `crates/engine/src/layer.rs`, whose doc lists eight things the root and its modules share:

- `refusal_reports`, which `layer_reports` and the labels (`labels.rs`) call;
- `renderer_refusals`, which only `layer_reports` calls, put beside it by ticket 11 of
  revision-2026-11-18 (f6fd7a9d);
- `refused_hatches`, which the labels call, and `plural`, which these reports use.

A module may not take an item of the root (`engine_modules.rs`). So a report lies in `layer.rs`
where a module calls it and in the root where only the root does: where it is written follows who
calls it, not what it is.

These twelve functions move into one new module that imports only `layer`, for `name` and
`quoted`. In the direction table of `crates/arch-test/tests/engine_modules.rs` it stands below
`labels`, which calls it, and above `layer`. The root and `labels.rs` call it.

**Stay where they are**, because they report before or outside a layer's drawing, or are shared
wider than these reports:

- in `lib.rs`: `view` (the map's errors), `checked_opacity`, `drawable_mode`, `asked`,
  `declared_projection`, `rotation_without_projection`, `skipped`, `kind` (which `drawable`
  computes the undrawn geometries' kinds with; `layer_reports` receives them already counted), and
  the diagnostics of the queries;
- in `layer.rs`: `name` and `quoted`, which `classes.rs`, `reproject.rs`, `labels.rs`, and the
  root import too;
- the diagnostics `bind.rs`, `classes.rs`, `labels.rs`, and `reproject.rs` write themselves.

Each code, text, severity, and order of the diagnostics stays as it is.

## Acceptance criteria

- [ ] Given `crates/engine/src/lib.rs` and `crates/engine/src/layer.rs`, when they are searched,
  then none of the twelve functions named above is defined in either, and each is defined in the
  new module.
- [ ] Given every test of the workspace and the conformance harness, when they run, then all
  pass, no assertion changed.
- [ ] Given `crates/arch-test/tests/engine_modules.rs`, the module list of `lib.rs`, and the
  engine's entry in `docs/ARCHITECTURE.md`, when they are read, then they name the new module in
  its place in the direction.

## Read first

- Touches: `crates/engine/src/lib.rs`, `crates/engine/src/layer.rs`, a new module under
  `crates/engine/src/`, `crates/engine/src/labels.rs` (its imports of `refusal_reports` and
  `refused_hatches`), `crates/arch-test/tests/engine_modules.rs`, `docs/ARCHITECTURE.md` (the
  `engine` entry). `classes.rs` and `reproject.rs` import `name` and `quoted`, which stay; they
  change only if the new module needs them to.
- ADR-0008, ADR-0022, ADR-0049 (*Enforcement*: the engine's tests of each refusal's warning); the
  convention *A commit typed `refactor` changes nothing a test can see*.

## Blocked by

- None (can start immediately)

## Mode

unattended: the existing tests decide it, the change preserves every interface outside the crate,
and a wrong result costs one revert. Stop when a report cannot move without a module taking an
item of the root, or without a change of a diagnostic's order; hand off with the report and why.
