# Ticket: A point's buffer has the same bits on every target

<!-- Copy to docs/plan/<spec>/<NN>-<slug>.md, next to the spec it slices, if it has one. A
     vertical slice: a narrow but complete path through every layer the change touches, demoable
     or verifiable on its own, sized to one fresh agent session. Claim it by creating the branch
     ticket/<spec>/<NN>-<slug> BEFORE any work, so parallel sessions sharing the repository skip
     it. The branch that lands it deletes this file. Decided in
     docs/adr/0005-feature-layer.md. -->

## What to build

geo 0.33.1 buffers a point with a round cap by its own loop, which calls std's `cos` and `sin`,
the platform's libm. So a point's buffer, a circle of 120 segments, need not have the same last
bits natively and on wasm32 (Q-2, G-9). ADR-0041 decides that `buffer` is geo's.

The maintainer chose on 2026-11-19 a successor of ADR-0041. The session proposes it first through
`propose-adr`: Forager computes a point's circle itself with `libm`, as GEOS's `createCircle`
does, from `(x + d, y)` clockwise; every other buffer stays geo's. Once the maintainer accepts
it, the session implements it, and the deviation stated in the semantics page goes. The change
deletes `docs/backlog/point-buffer-platform-trig.md`.

## Acceptance criteria

- [ ] Given the successor of ADR-0041, when the session implements, then the maintainer has
  accepted it.
- [ ] Given a point buffered by a distance, when the geometry module computes it, then its 120
  vertices are GEOS's, within the tolerance the module's tests hold GEOS's coordinates to.
- [ ] Given a map that buffers a point, when it renders natively and in wasmi (ADR-0034), then the
  two images are equal.
- [ ] Given the workspace's `clippy.toml`, when clippy runs, then the new code calls no banned
  function.

## Read first

- Touches: `crates/symbolize/src/geometry.rs` (`buffer`), `crates/symbolize/src/geometry/tests.rs`,
  `docs/semantics/style-symbol.md` (the deviation), `docs/adr/0041-geometry-through-geo.md`
- `docs/backlog/point-buffer-platform-trig.md`; ADR-0034 for the wasm comparison.
- The skill `propose-adr`, its rule on successors.

## Blocked by

- docs/plan/backlog-deviations/02-layer-transform-on-a-multipoint.md

## Mode

with the human: it needs a successor of ADR-0041, which the maintainer accepts before the
implementation starts.

## Where it stands

2026-11-19: phase 1, the ADR, is done. The successor of ADR-0041 is proposed as
`docs/adr/draft-geometry-through-geo-and-own-circle.md`, commit 5f0bcc39. It waits for the
maintainer. Nothing is implemented.

Found while drafting:

- geo 0.33.1 reaches its own circle from a point, each point of a multipoint, a line whose points
  are all equal, and each such line of a multiline. The draft covers all four and keeps geo's
  `unary_union` and `union` that merge the circles.
- No feature switch routes geo's `cos` and `sin` to `libm`. num-traits' `libm` is already on, but
  num-traits 0.2.19 takes std's methods whenever `std` is on. geo, geo-types' `std`, and
  serde-saphyr turn `std` on, and Cargo unifies features across one build.
- On `wasm32-unknown-unknown`, std's `cos` is compiler-builtins' copy of libm 0.2.16. Natively on
  glibc 2.41 it differs in the last bit at 5 `cos` and 7 `sin` of geo's 120 angles.
- JTS 1.20.0 and `master` snap a `cos` or `sin` below 5 × 10⁻¹⁶ to 0, and GEOS took this up in
  3.13.0. GEOS 3.12.1, the reference, does not. The draft ports JTS 1.19.0 for the circle.

Open for the maintainer: the questions the hand-off lists, among them JTS 1.19.0 or `master`, geo's
unions kept for a line of one place, and the circle's dropped-point step.

Next: once the maintainer accepts the draft, implement it in `buffer_with_style` of
`crates/symbolize/src/geometry.rs`, then the tests the draft's *Enforcement* names, remove the
deviation from `docs/semantics/style-symbol.md`, and delete
`docs/backlog/point-buffer-platform-trig.md`.
