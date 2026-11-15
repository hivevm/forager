# Ticket: The engine's reprojection says what it does, and takes no bound only tests need

## What to build

- The doc comment of `LayerReprojection::features_within` (`crates/engine/src/reproject.rs:108-117`)
  is two merged blocks: it says the function returns how many points do not reproject, where it
  returns `Reprojected`, and it links `[Self::features]`, which does not exist; rustdoc checks no
  link of a private item.
- `reproject_features` (`crates/engine/src/lib.rs:711`) and `features_within` take a `bound` that
  production always passes as `MAX_HORIZON_POINTS` (`lib.rs:658`, `reproject.rs:279`), a parameter
  widened for the tests. Production calls a function without it; the bounded form stays for the
  tests that push the bound.

## Acceptance criteria

- [ ] Given `reproject.rs`, when the change lands, then the doc comment of each function says what
  it returns and links only items that exist.
- [ ] Given the render path in `lib.rs`, when the change lands, then it passes no bound, and the
  test that pushes `horizon-points-limit` through a render still passes.

## Read first

- Touches: `crates/engine/src/reproject.rs`, `crates/engine/src/lib.rs`,
  `crates/engine/tests/reproject.rs`
- `docs/adr/0037-reprojection-ported-in-proj.md`; Q-9 in `docs/SPECIFICATION.md`.

## Blocked by

- docs/plan/revision-2026-11-15/01-map-projection-held-once.md (both touch `reproject.rs`)

## Mode

unattended: the engine's tests decide it; one revert. Stop when a test's expected value moves.
