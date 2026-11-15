# Ticket: The labels hold the render's pixmaps instead of passing them down

## What to build

`Labels` (`crates/engine/src/labels.rs:221-251`) holds `map`, `view`, `factors`, and `files`. The
render's `pixmaps: &Pixmaps` and the `binder` are passed through about nine functions instead
(`labels.rs:433,494,582,1230`, `label_styles.rs:159,186`). `marker_bounds` takes `(map, pixmaps)`
as one tuple (`label_styles.rs:161`) to stay under clippy's limit on arguments, and its callee
`bound_marker` takes seven parameters. Hold the pixmaps in `Labels`, or in one context the label
functions take, and narrow the signatures.

## Acceptance criteria

- [ ] Given `labels.rs` and `label_styles.rs`, when the change lands, then no function takes the
  render's pixmaps beside a value that already holds them, and no tuple bundles arguments.
- [ ] Given the engine's tests and the label records of G-10, when they run, then they pass with
  no expected value changed.

## Read first

- Touches: `crates/engine/src/labels.rs`, `crates/engine/src/label_styles.rs`,
  `crates/engine/src/lib.rs`
- `docs/adr/0029-pixmaps-held-once-per-render.md`, `docs/adr/0032-label-cache.md`.

## Blocked by

- docs/plan/revision-2026-11-15/10-engine-layer-transform-module.md

## Mode

unattended: the unchanged suite decides it; one revert. Stop when a test's expected value moves. It
touches nothing ADR-0006 keeps with the human, and it counts against ADR-0006's cap of three open
agent branches.
