# Ticket: smoothsia's port of mapsmoothing.c lives in a module of its own

## What to build

`crates/symbolize/src/layer_transform.rs` has grown to some 990 lines. Its last quarter, about
240 lines, is one concept: the port of MapServer's `src/mapsmoothing.c`. It consists of `fp_eq`,
`Step`, `Window` and its `lineWindow` walk, `smooth_sia` with the `Buffers` that ticket 06 of
backlog-deviations added, and `preprocess`, which is `processShapePathDistance`. The rest of the
file is the transform tree, its budget, the shape, and the GEOS-shaped nodes. Ticket 06 changed
only the smoothing, and ticket 02 below will change it again, so the reader has to work through
the whole file for a change confined to one C file.

`geometry/topology.rs` shows the precedent: one port of one source family, in a submodule beside
the code that calls it. Move the port into `layer_transform/smoothsia.rs`. Leave in the parent
only what the tree's evaluation calls: `smooth_sia` and `preprocess`, `pub(super)`, with the
budget and the shape passed in as today. The tests of the smoothing in
`layer_transform/tests.rs` go along where they test only the port. Tests that go through
`transformed` stay where they are.

This is an interface-preserving refactor (`refactor` commits only). No assertion changes, so the
refactor tests sensor lists nothing.

## Acceptance criteria

- [ ] Given the tree after the change, when `scripts/check-all.sh` runs, then it passes, and
  `scripts/sensor-refactor-tests.sh` lists no commit of the branch.
- [ ] Given the tree after the change, when `layer_transform.rs` is searched for `Window`, `Step`,
  `Buffers`, and `fp_eq`, then none is defined there; each is defined in
  `layer_transform/smoothsia.rs`, which names `src/mapsmoothing.c` as the source it follows.
- [ ] Given the test suite before and after the change, when the tests of `symbolize` run, then
  the same tests pass, by name, with none removed. Only their module paths change
  (`scripts/sensor-tests-kept.sh`).
- [ ] Given `docs/ARCHITECTURE.md`, when it is read against the tree, then it names
  `layer_transform/smoothsia.rs` wherever it lists the modules of `symbolize` and the order of
  their imports.

## Read first

- Touches: `crates/symbolize/src/layer_transform.rs`, `crates/symbolize/src/layer_transform/`,
  `docs/ARCHITECTURE.md` (the paragraph of `symbolize`)
- `crates/symbolize/src/geometry/topology.rs` and its place under `geometry.rs`: the precedent
- MapServer's notice: `scripts/check-notice.sh` reads a file that names a MapServer source, so the
  new file holds the notice that `layer_transform.rs` holds for `src/mapsmoothing.c`
- `docs/CONVENTIONS.md`: *A commit typed `refactor` changes nothing a test can see*

## Blocked by

- None (can start immediately)

## Mode

unattended. The suite decides it, and a wrong result costs one revert. It touches nothing that
ADR-0006 keeps with the human. Stop if moving the port needs a change of signature that a test
sees, or that widens what the parent exposes beyond `smooth_sia` and `preprocess`. Hand off
with the signature that blocked it.
