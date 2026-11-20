# REN-i-overlay-shared-with-geo: The hatch is cut from its polygon by i_overlay's boolean intersection, as MapServer's Clipper cuts it, and geo is the only other package that depends on i_overlay

- **Status:** 🟢 accepted
- **Date:** 2026-11-15
- **Deciders:** Markus Brigl
- **Applies to:** the cut of a hatch in `crates/render-agg`, the `i_overlay` entry of its `Cargo.toml`, the i_overlay that geo brings into `crates/symbolize`, the bounds of a hatch in `crates/render`, and the test `only_render_agg_cuts_polygons_with_i_overlay` of `crates/arch-test`
- **Supersedes:** [REN-hatch-cut-by-i-overlay](REN-hatch-cut-by-i-overlay.md)

## Context

[REN-hatch-cut-by-i-overlay](REN-hatch-cut-by-i-overlay.md) cuts a hatch from its polygon with
i_overlay's integer intersection. Its *Context*, *Alternatives*, and *Sources* still hold. Its
*Decision* ends: "i_overlay must be a dependency of `render-agg` alone, with its default features
off." Its *Out of scope* leaves a `GEOMTRANSFORM` `buffer` free to choose i_overlay by an ADR of its
own.

[GEO-simplify-buffer-through-geo](GEO-simplify-buffer-through-geo.md) makes geo 0.33 a dependency of
`symbolize`, as the maintainer chose on 2026-11-15. geo 0.33.1 depends on i_overlay 4.5.2 with
i_float 1.16, i_shape 1.18, i_key_sort 0.10, and i_tree 0.18. Cargo keeps them beside the 9.1.0
family of `render-agg`, since the two majors are not compatible. `symbolize` so depends on i_overlay
through geo, against the letter of REN-hatch-cut-by-i-overlay's clause. That ADR takes this one as
its precondition.

The test `only_render_agg_cuts_polygons_with_i_overlay` of `crates/arch-test/tests/dependencies.rs`
holds REN-hatch-cut-by-i-overlay's clause. It reads each workspace crate's direct dependencies, so
geo's i_overlay does not fail it.

This ADR restates every clause of REN-hatch-cut-by-i-overlay in its wording, but these:

- the title and the *Decision*'s first sentence: they add that geo is the only other package that
  depends on i_overlay;
- *The dependency*: i_overlay stays a direct dependency of `render-agg` alone among the workspace's
  crates. New items let geo depend on it, and `symbolize` alone depend on geo;
- *Out of scope*: the `buffer` of a `GEOMTRANSFORM` is no longer an example of what is left open,
  since the geometry ADR decides it, and the unification of the two i_overlay majors is added;
- *Consequences*: REN-hatch-cut-by-i-overlay's are restated, but its follow-up that a
  `GEOMTRANSFORM` `buffer` may build on i_overlay under an ADR of its own is dropped, since the
  geometry ADR decides it. A Positive item, the geometry ADR accepted without a conflict, and the
  two i_overlay families are added;
- *Enforcement*: the arch-test's assertion on the workspace's crates gives way to two on the
  resolved graph, which take in the packages outside the workspace;
- *Applies to*: geo's i_overlay in `symbolize` and the arch-test are added.

## Decision

We will cut a hatch's stroke from its polygon with i_overlay's integer intersection, as
MapServer's Clipper cuts it. geo is the only other package that may depend on i_overlay.

- **The cut.** The polygon is the subject and the dash rectangles the clip, both under the
  even-odd rule. Each coordinate must be scaled by 100 and rounded half away from zero to an
  `i32`. The cut runs on i_overlay's fragment solver at absolute precision on one thread. The
  result is scaled back and filled under the non-zero rule through the gamma.
- **The bounds.** The renderer must cut only a hatch within the bounds that the display list's
  `Hatch::is_bounded` sets on its lines, its dashes, the polygon's vertices, and an estimate of
  their crossings. The engine must refuse a hatch beyond them with a diagnostic (Q-9).
- **The dependency.**
  - i_overlay must be a direct dependency of `render-agg` alone among the workspace's crates, with
    its default features off.
  - geo may depend directly on i_overlay, in the version it asks for.
  - No other package of the resolved graph may depend directly on i_overlay.
  - `symbolize` must be the only crate of the workspace that depends directly on geo.

  A crate that depends on `render-agg` or `symbolize`, such as `engine`, so reaches i_overlay
  through them, as REN-hatch-cut-by-i-overlay already allowed for `render-agg`.

**Out of scope:**

- any other polygon boolean operation, which may choose i_overlay by an ADR of its own;
- the `GAMMA` of the output format and of the layer, which is not carried yet;
- the unification of the two i_overlay majors, which waits for a geo release on i_overlay 9.

## Alternatives considered

- **Keep REN-hatch-cut-by-i-overlay and read its clause as direct dependencies only** — no new
  record, and the test already reads it so. A reader of "a dependency of `render-agg` alone" would
  take geo's i_overlay for a violation. A silent reading of an accepted ADR is what
  [`AGENTS.md` §3](../../AGENTS.md#3-adr-rules) rule 5 forbids.
- **Make i_overlay 9.1.0 a direct dependency of `symbolize` too** — what a port of JTS with
  i_overlay's union would have needed. The maintainer chose geo, which brings its own i_overlay.
- **Wait for a geo release on i_overlay 9** — one i_overlay family in the tree. No such release
  exists at geo 0.33.1, and the fidelity plan needs `simplify` now.
- **The alternatives of REN-hatch-cut-by-i-overlay** — a cut of Forager's own, geo's `BooleanOps`
  for the hatch, coverage only, and a port of Clipper 4. They stand as REN-hatch-cut-by-i-overlay
  records them. geo's `BooleanOps` stays rejected for the hatch, which needs Clipper's rounding to a
  hundredth of a pixel.

## Sources / Prior art

- [REN-hatch-cut-by-i-overlay](REN-hatch-cut-by-i-overlay.md), whose *Context*, *Sources*, and
  measurements this ADR keeps.
- geo 0.33.1, crates.io, its `Cargo.toml`: `i_overlay` 4.5, read 2026-11-15; the `Cargo.lock` a
  scratch copy of the workspace wrote with geo added to `symbolize`.
- `crates/arch-test/tests/dependencies.rs`, `dependents` and
  `only_render_agg_cuts_polygons_with_i_overlay`, read 2026-11-15.
- [GEO-simplify-buffer-through-geo](GEO-simplify-buffer-through-geo.md).

## Consequences

- Positive: the cut is the intersection MapServer computes, holes stay white, overlapping rings
  and stroked lines cancel exactly as under Clipper, and `renderers/polygon-hatch.map` and
  `renderers/polygon-hatch-dashed.map` pass; Forager keeps no clipper of its own.
- Positive: the geometry ADR can be accepted without a conflict between two accepted ADRs.
- Negative / trade-offs: a dependency of five crates of one project; a hatch is not drawn whose
  polygon has more than 16 384 vertices or crosses itself too often, or whose stroke crosses the
  polygon's edges too often by the estimate, where MapServer draws it however long it takes;
  i_overlay snaps within a hundredth of a pixel where Clipper does not.
- Negative / trade-offs: the tree carries two i_overlay families, five crates each, until geo moves
  to i_overlay 9. A defect fixed in one family needs its fix in the other.
- Follow-ups:
  - the bounds of Q-9 follow the first measurement of the expected scale;
  - when geo depends on i_overlay 9, a change that moves both to one family, with the hatch's
    tests and the geometry module's tests green.

## Enforcement

Tests cite this ADR with `Verifies:`:

- in `crates/arch-test/tests/dependencies.rs`, the test
  `only_render_agg_cuts_polygons_with_i_overlay` moves its `Verifies:` from
  REN-hatch-cut-by-i-overlay to this ADR. Its assertion on the workspace's crates gives way to two
  assertions on the resolved graph, which imply it. One fails when a package other than `render-agg`
  and geo depends directly on i_overlay. The other fails when a workspace crate other than
  `symbolize` depends directly on geo;
- in `crates/render-agg` (`src/hatch.rs`, `src/lib.rs`), the polygon and the stroke intersected
  under the even-odd rule, overlapping rings and pieces left empty, coordinates rounded to a
  hundredth of a pixel half away from zero, and a hole left white under rotated dashes;
- in `crates/engine/tests/pixmaps.rs`, the clip buffer of a hatched class.

The conformance harness holds the two hatch cases of the category geometry and symbols.
