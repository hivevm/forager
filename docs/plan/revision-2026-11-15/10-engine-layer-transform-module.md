# Ticket: The engine's layer transform in one module, and its modules named for what they hold

## What to build

- A layer's `GEOMTRANSFORM` is spread over the engine. `crates/engine/src/lib.rs:934-1033` holds
  `styles_not_drawn`, `described`, `transformed`, `transform_not_drawn`, and `transform_failed`.
  `classes.rs:328-354` holds `Selector::transform`, and `classes.rs:494` `layer_cellsize`.
- `classes.rs` also holds the scale and resolution factors (`classes.rs:460-548`) and the reading of
  colours (`classes.rs:652-690`). Its header, "Which class draws a feature...", describes none of
  them.
- `crates/engine/src/label_cache.rs`, split out by 0c11953d, holds `LabelAt` and the growth of
  vectors; the label cache lives in `crates/label/src/cache.rs`, so the name misleads.

Move the layer's transform into a module of its own, give `classes.rs` a header that says what it
holds or move the factors out, and rename `label_cache.rs` for what it holds. The table of
`crates/arch-test/tests/engine_modules.rs` and the overview's module order follow.

## Acceptance criteria

- [ ] Given `crates/engine/src/`, when the change lands, then the functions of a layer's transform
  are in one module and `lib.rs` holds none of them.
- [ ] Given `arch-test`'s `tests/engine_modules.rs`, when it runs, then it passes with a row per
  module and the direction the overview states.
- [ ] Given the engine's tests, when they run, then they pass with no expected value changed.

## Read first

- Touches: `crates/engine/src/lib.rs`, `crates/engine/src/classes.rs`,
  `crates/engine/src/label_cache.rs`, `crates/arch-test/tests/engine_modules.rs`,
  `docs/ARCHITECTURE.md` (the engine's module order)
- `docs/adr/0036-typed-geomtransform.md`, `docs/adr/0008-ports-and-adapters-crates.md`;
  `docs/CONVENTIONS.md`, *A commit typed `refactor` changes nothing a test can see*.

## Blocked by

- docs/plan/revision-2026-11-15/03-engine-reprojection-interface.md

## Mode

unattended: the arch test and the unchanged suite decide it; one revert. Stop when a test's expected
value moves or the module direction needs a cycle. It touches nothing ADR-0006 keeps with the human,
and it counts against ADR-0006's cap of three open agent branches.
