# Ticket: Lay out a layer's labels without a `LABELCACHE` flag

## What to build

Whether a point layer's labels go through the cache is decided three times in the engine: at
`crates/engine/src/lib.rs:193`, at `crates/engine/src/labels.rs:262`, and as
`layer.at_once.is_some()` at `labels.rs:395`. `lay_out` (`labels.rs:404`) takes `at_once: bool`
under `#[allow(clippy::too_many_arguments)]` and branches on it six times. It also filters a style
the `LABELCACHE OFF` caller already passes as `None`. The `else { return 0 }` at `labels.rs:545`
cannot run, and `drawn_at_once` (`labels.rs:610`) wraps `out_of_reach` in one line. Decide the mode
once per layer and give each mode an entry of its own around a shared core without a mode flag.
No behaviour changes.

## Acceptance criteria

- [ ] Given the tests of `crates/engine` at the base, when `lay_out` has no `at_once` parameter,
  then they pass unchanged.
- [ ] Given the engine, when `rg 'at_once|label_cache' crates/engine/src` runs, then the mode is
  computed in one place.
- [ ] Given `labels.rs`, when it is read, then it holds no unreachable branch and no one-line
  wrapper of `out_of_reach`.

## Read first

- Touches: `crates/engine/src/labels.rs`, `crates/engine/src/lib.rs`
- `docs/CONVENTIONS.md` (*A commit typed `refactor` changes nothing a test can see*),
  [ADR-0032](../../adr/0032-label-cache.md).
- Best after `docs/plan/revision-2026-11-11/01-multipoint-markers-point-by-point.md`, which moves
  the `LABELCACHE OFF` path's caller.

## Blocked by

- docs/plan/revision-2026-11-11/01-multipoint-markers-point-by-point.md

## Mode

unattended: an interface-preserving refactoring decided by the unchanged suite; it stays in
`crates/engine` and costs one revert; nothing ADR-0006 keeps with the human. A wrong result shows as
a failing test. Stop when a test would have to change, and hand off with it named.
