# Spec: Deviations taken from the backlog

<!-- Copy to docs/plan/<spec>/spec.md. The destination of a piece of work larger than one agent
     session, written from the user's side; its tickets lie next to it in the same directory. It
     is disposable: the design revision deletes it once its tickets have merged, and whatever has
     to last moves into docs/SPECIFICATION.md, an ADR, docs/ARCHITECTURE.md, docs/GLOSSARY.md, or
     docs/CONVENTIONS.md first. No file paths or code; they go stale. How it is written is the
     plan-feature skill under .agents/skills/. Decided in docs/adr/0005-feature-layer.md. -->

## Problem

Five entries of the backlog name places where Forager does not do what it should, and the
maintainer chose them on 2026-11-19 to be fixed together:

- a self-test of the check runner fails at random under a heavy load, so a green run depends on
  the machine's load;
- a layer with a `GEOMTRANSFORM` draws nothing on a multipoint and warns, where MapServer draws
  the transformed shape;
- the render fuzz target never builds eight of the style's geometry transforms nor the label
  transform `labelpoly`, so the pixel box of `bbox` and the label box of `labelpoly` are code no
  fuzzer reaches;
- a brushed line leaves the piece at a sharp turn undrawn, though the rasterizer would draw it
  without an overflow, because the bound that guards the overflow is loose;
- a point's buffer takes `cos` and `sin` from the platform, so it need not have the same bits
  natively and on wasm32.

Ticket 03's first fuzz run found a sixth: two layers that share a nested `smoothsia` tree run past
the fuzz run's 10 s within the visit budget. The maintainer chose on 2026-11-19 to fix it in this
plan as ticket 06.

## Solution

The check runner's self-test passes under load and fails on a broken runner. A layer's transform
draws on a multipoint as MapServer draws it. The render fuzz target builds every geometry
transform and every label transform, and the fuzz variant check holds both enums. A brushed line
draws every piece the rasterizer can draw without an overflow, within a tighter bound that still
rejects each piece that would overflow. A point's buffer has the same bits on every target. The
render fuzz target runs its convention's time without a timeout.

## Existing behaviour

- The check runner's self-test pins the runner's stop on TERM, its cap of jobs, and its settle.
  The TERM cases must keep failing on a runner that leaves a check behind.
- A layer's transform refuses a multipoint and warns. Its tests pin the refusal; the semantics
  page states it. MapServer's GEOS transforms take the first part of a point shape, as the port
  already does for a point. The msautotest cases of the scope show whether any moves.
- The render fuzz target decodes its bytes into a map. Every seed of it names the path it was made
  for, and a test decides that each still builds it. A change to the decoding moves every seed.
- The brush's bound rejects every path that overflows agg-rust's distance interpolator. Its tests
  and the adversarial probes of the revision of 2026-11-16 pin that no rejected path is drawn and
  that no drawn path panics.
- A point's buffer is geo's, a circle of 120 segments within 10⁻¹² of GEOS's.
  ADR-0041 (superseded by ADR-draft-geometry-through-geo-and-own-circle) decided that `buffer` is
  geo's. The unit tests of the geometry module hold GEOS's coordinates.

## Goals served

- Q-2, G-9: the same bits natively and on wasm32 (point buffer).
- Q-3: no panic on any input, held by the fuzz targets (fuzz variants, brush bound).
- G-6, Q-4: drawn as MapServer draws (multipoint, brush).
- The quality bar of [`AGENTS.md` §5](../../../AGENTS.md#5-quality-bar--definition-of-done): a
  flaky test is a defect (check runner).

## User stories

1. As a maintainer, I want the check runner's self-test to pass on a loaded machine, so that a red
   run means a defect.
2. As a map author, I want a layer's `GEOMTRANSFORM` drawn on my multipoints, so that my map looks
   as it does in MapServer.
3. As a maintainer, I want every geometry and label transform fuzzed, so that a panic in their own
   code is found before a user meets it.
4. As a map author, I want a road with sharp turns drawn whole by its brush, so that no piece of it
   is missing.
5. As a host of the browser binding, I want a buffered point to look the same as on the server,
   so that tiles from both match.

## Implementation decisions

- Each ticket fixes one backlog entry and deletes it in the same change.
- The check runner's flake is fixed at the cause a reproduction under load shows. No retry, longer
  sleep, or looser bound passes for a fix.
- A multipoint reaches a layer's transform as a point shape of its parts. Each transform then
  takes what MapServer's takes from it.
- The render fuzz target reads the transforms from bytes it does not read yet. Both enums join the
  fuzz variant check's list. The seeds move as the fuzz seed convention says.
- The brush's bound becomes the bound by the box the pixels lie in, which the review of the
  revision of 2026-11-16 measured. The adversarial probe runs again on it.
- A point's buffer becomes a circle Forager computes with `libm`, as GEOS's `createCircle` does,
  under a successor of ADR-0041 (superseded by ADR-draft-geometry-through-geo-and-own-circle) the
  maintainer accepts first.

## Testing decisions

- The check runner: its self-test, run repeatedly on a loaded machine and against a runner broken
  by hand.
- The multipoint: the layer transform's unit tests, one per transform on a shape of several
  parts, and the msautotest run of the scope.
- The fuzz variants: the fuzz variant check and the render seed test.
- The brush: the brush's unit tests and the adversarial probe of the revision of 2026-11-16.
- The point buffer: the geometry module's unit tests against GEOS's coordinates, and the wasm
  renders compared in wasmi (ADR-0034).

## Out of scope

- The other nine entries of the backlog of 2026-11-19, and the entries the tickets file.
- A multipoint in a style's geometry transform; only a layer's transform changes.
- A buffer of a line or a polygon; only a point's circle leaves geo.
- A record of GEOS's coordinates made by the noise run.

## ADR impact

- ADR-0041 bound the point buffer; ADR-draft-geometry-through-geo-and-own-circle binds it now.
  Ticket 05 proposes its successor through `propose-adr`, and the maintainer accepts it before the
  implementation.
- ADR-0043 and ADR-0048 bind the layer's transform; the multipoint changes no clause of them.
- ADR-0046 and ADR-0049 bind the brush and its guard; the tighter bound keeps their clauses.
- ADR-0034 binds the wasm comparison the point buffer is tested by.
- The check runner and the fuzz target are bound by no ADR beyond the conventions.
