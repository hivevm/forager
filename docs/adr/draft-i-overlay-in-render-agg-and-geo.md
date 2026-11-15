# ADR-draft-i-overlay-in-render-agg-and-geo: The hatch is cut from its polygon by i_overlay's boolean intersection, as MapServer's Clipper cuts it, and i_overlay reaches another crate only through geo

- **Status:** 🟡 proposed
- **Date:** 2026-11-15
- **Deciders:** Markus Brigl
- **Applies to:** the cut of a hatch in `crates/render-agg`, the `i_overlay` entry of its `Cargo.toml`, the i_overlay that geo brings into `crates/symbolize`, the bounds of a hatch in `crates/render`, and the test `only_render_agg_cuts_polygons_with_i_overlay` of `crates/arch-test`
- **Supersedes:** [ADR-0028](0028-hatch-cut-by-i-overlay.md)

## Context

[ADR-0028](0028-hatch-cut-by-i-overlay.md) cuts a hatch from its polygon with i_overlay's integer
intersection. Its *Context*, *Alternatives*, and *Sources* still hold. Its *Decision* ends:
"i_overlay must be a dependency of `render-agg` alone, with its default features off." Its *Out of
scope* leaves a `GEOMTRANSFORM` `buffer` free to choose i_overlay by an ADR of its own.

[ADR-draft-geometry-through-geo](draft-geometry-through-geo.md) makes geo 0.33 a dependency of
`symbolize`, as the maintainer chose on 2026-11-15. geo 0.33.1 depends on i_overlay 4.5.2 with
i_float 1.16, i_shape 1.18, i_key_sort 0.10, and i_tree 0.18. Cargo keeps them beside the 9.1.0
family of `render-agg`, since the two majors are not compatible. `symbolize` so depends on
i_overlay through geo, against the letter of ADR-0028's clause. That ADR takes this one as its
precondition.

The test `only_render_agg_cuts_polygons_with_i_overlay` of `crates/arch-test/tests/dependencies.rs`
holds ADR-0028's clause. It reads each workspace crate's direct dependencies, so geo's i_overlay
does not fail it.

This ADR restates every clause of ADR-0028 in its wording, but these:

- *The dependency*: i_overlay stays a direct dependency of `render-agg` alone. A new item lets
  geo bring its own i_overlay into `symbolize`;
- *Out of scope*: the `buffer` of a `GEOMTRANSFORM` is no longer an example of what is left open,
  since the geometry ADR decides it, and the unification of the two i_overlay majors is added;
- *Enforcement*: the arch-test gains an assertion on dependencies through other crates;
- *Applies to*: geo's i_overlay in `symbolize` and the arch-test are added.

## Decision

We will cut a hatch's stroke from its polygon with i_overlay's integer intersection, as
MapServer's Clipper cuts it. i_overlay reaches another crate only through geo.

- **The cut.** The polygon is the subject and the dash rectangles the clip, both under the
  even-odd rule. Each coordinate must be scaled by 100 and rounded half away from zero to an
  `i32`. The cut runs on i_overlay's fragment solver at absolute precision on one thread. The
  result is scaled back and filled under the non-zero rule through the gamma.
- **The bounds.** The renderer must cut only a hatch within the bounds that the display list's
  `Hatch::is_bounded` sets on its lines, its dashes, the polygon's vertices, and an estimate of
  their crossings. The engine must refuse a hatch beyond them with a diagnostic (Q-9).
- **The dependency.**
  - i_overlay must be a direct dependency of `render-agg` alone, with its default features off.
  - `symbolize` may depend on i_overlay through geo, in the version geo asks for.
  - No other crate of the workspace may depend on i_overlay, directly or through another crate.

**Out of scope:**

- any other polygon boolean operation, which may choose i_overlay by an ADR of its own;
- the `GAMMA` of the output format and of the layer, which is not carried yet;
- the unification of the two i_overlay majors, which waits for a geo release on i_overlay 9.

## Alternatives considered

- **Keep ADR-0028 and read its clause as direct dependencies only** — no new record, and the test
  already reads it so. A reader of "a dependency of `render-agg` alone" would take geo's i_overlay
  for a violation. A silent reading of an accepted ADR is what
  [`AGENTS.md` §3](../../AGENTS.md#3-adr-rules) rule 5 forbids.
- **Make i_overlay 9.1.0 a direct dependency of `symbolize` too** — what a port of JTS with
  i_overlay's union would have needed. The maintainer chose geo, which brings its own i_overlay.
- **Wait for a geo release on i_overlay 9** — one i_overlay family in the tree. No such release
  exists at geo 0.33.1, and the fidelity plan needs `simplify` now.
- **The alternatives of ADR-0028** — a cut of Forager's own, geo's `BooleanOps` for the hatch,
  coverage only, and a port of Clipper 4. They stand as ADR-0028 records them. geo's `BooleanOps`
  stays rejected for the hatch, which needs Clipper's rounding to a hundredth of a pixel.

## Sources / Prior art

- [ADR-0028](0028-hatch-cut-by-i-overlay.md), whose *Context*, *Sources*, and measurements this
  ADR keeps.
- geo 0.33.1, crates.io, its `Cargo.toml`: `i_overlay` 4.5, read 2026-11-15; the `Cargo.lock` a
  scratch copy of the workspace wrote with geo added to `symbolize`.
- `crates/arch-test/tests/dependencies.rs`, `dependents` and
  `only_render_agg_cuts_polygons_with_i_overlay`, read 2026-11-15.
- [ADR-draft-geometry-through-geo](draft-geometry-through-geo.md).

## Consequences

- Positive: the cut of a hatch stays as it is, and its two cases keep passing. The geometry ADR
  can be accepted without a conflict between two accepted ADRs.
- Negative / trade-offs: the tree carries two i_overlay families, five crates each, until geo moves
  to i_overlay 9. A defect fixed in one family needs its fix in the other.
- Follow-ups: when geo depends on i_overlay 9, a change that moves both to one family, with the
  hatch's tests and the geometry module's tests green.

## Enforcement

Tests cite this ADR with `Verifies:`:

- in `crates/arch-test/tests/dependencies.rs`, the test
  `only_render_agg_cuts_polygons_with_i_overlay` moves its `Verifies:` from ADR-0028 to this ADR.
  It keeps failing when a crate other than `render-agg` depends on i_overlay directly. It gains an
  assertion on dependencies through other crates. That assertion fails when a crate other than
  `symbolize` and `render-agg` depends on i_overlay by any chain;
- in `crates/render-agg` (`src/hatch.rs`, `src/lib.rs`), the polygon and the stroke intersected
  under the even-odd rule, overlapping rings and pieces left empty, coordinates rounded to a
  hundredth of a pixel half away from zero, and a hole left white under rotated dashes;
- in `crates/engine/tests/pixmaps.rs`, the clip buffer of a hatched class.

The conformance harness holds the two hatch cases of the category geometry and symbols.
