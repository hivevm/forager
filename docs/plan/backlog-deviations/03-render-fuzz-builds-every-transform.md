# Ticket: The render fuzz target builds every geometry and label transform

<!-- Copy to docs/plan/<spec>/<NN>-<slug>.md, next to the spec it slices, if it has one. A
     vertical slice: a narrow but complete path through every layer the change touches, demoable
     or verifiable on its own, sized to one fresh agent session. Claim it by creating the branch
     ticket/<spec>/<NN>-<slug> BEFORE any work, so parallel sessions sharing the repository skip
     it. The branch that lands it deletes this file. Decided in
     docs/adr/0005-feature-layer.md. -->

## What to build

`fuzz/fuzz_targets/render_input.rs` names `GeomTransform::Expression` only, none of `Start`,
`End`, `Vertices`, `Bbox`, `Centroid`, `LabelPoint`, `LabelPoly`, and `LabelCenter`, and of
`LabelTransform` it names `Point` and `Center`, not `Poly`. No other fuzz target builds them.
The pixel box of `Bbox` and the label box of `LabelPoly` are code of their own no fuzzer reaches.

The session has the render target read every variant of both enums from bytes it does not read
yet, adds both enums to `ENUMS` of `scripts/check-fuzz-variants.sh`, and brings a seed that
reaches each. Every existing seed decodes to a different render then; the fuzz seed convention
has each seed run again and the render seed test decides that each still builds its path. The
change deletes `docs/backlog/geomtransform-variants-unfuzzed.md`.

Ticket 02 made a layer's transform draw on a multipoint, a path the render target does not reach,
since it builds no multipoint. The review of ticket 02 left its seed to this ticket, which rewrites
the decoding anyway: the target builds a multipoint of several parts under a layer's transform.

## Acceptance criteria

- [ ] Given the fuzz variant check, when it runs, then it lists `GeomTransform` and
  `LabelTransform` and passes.
- [ ] Given a variant of either enum removed from the render target by hand, when the check runs,
  then it fails naming the variant.
- [ ] Given the render seeds, when `fuzz/tests/render_seeds.rs` runs, then each seed builds the
  path it names, and a new seed names each variant of both enums.
- [ ] Given a seed that builds a multipoint of several parts under a layer's `GEOMTRANSFORM`, when
  the render seed test runs, then it builds that path.
- [ ] Given the render target, when it is fuzzed for the time the convention names, then it finds
  no panic, or each panic found is fixed or filed.

## Read first

- Touches: `fuzz/fuzz_targets/render_input.rs`, `scripts/check-fuzz-variants.sh` (`ENUMS`),
  `fuzz/tests/render_seeds.rs`, the render seed corpus
- `docs/backlog/geomtransform-variants-unfuzzed.md`; `crates/style-ir/src/model.rs`
  (`GeomTransform`, `LabelTransform`)
- `docs/CONVENTIONS.md`, the fuzz seed convention; Q-3 of `docs/SPECIFICATION.md`.
- Golden path: how the target builds the transform tree and `SymbolKind` today.

## Blocked by

- None (can start immediately)

## Mode

unattended: the variant check and the seed test decide the criteria, a wrong result costs one
revert, and it touches the fuzz target and a check only. Stop when a fuzz run finds a panic whose
fix leaves the ticket's scope; file it in the backlog and hand off.

## Where it stands (2026-11-19, stopped at the Mode's stop)

- Built: `fuzz/fuzz_targets/render_input.rs` reads a `Transforms` from the second half of the
  bytes, back to front: a class style's `GEOMTRANSFORM` from `start` to `labelcenter` on the
  polygon and line layers, drawn with the vector symbol; the label's style of `labelpoly`, of the
  hatch symbol or of none; and a multipoint appended to the features.
  `scripts/check-fuzz-variants.sh` lists `GeomTransform` and `LabelTransform` in `ENUMS` and
  passes (8 enums, 40 variants); with `GeomTransform::Bbox` and `LabelTransform::Poly` taken out
  of the target by hand, it fails naming both, and its self-test holds both cases.
- Seeds: `fuzz/tests/render_seeds.rs` names three new paths, a class style's transform per
  variant, the label's `labelpoly`, and a layer's tree drawn on a multipoint of several parts.
  Ten new seeds, `style-geomtransform-<variant>` for the eight variants, `label-labelpoly`, and
  `layer-geomtransform-multipoint`, each found by a fuzz run against a temporary panic at its
  path, not committed. Of the older seeds, `far-end-rounds-to-its-parameter` lost its path, its
  multipoint failing the layers' transform, and was re-derived the same way. The seed test passes.
- The first 300-second run of the render target from these seeds timed out, 12 s on one input,
  past Q-3's 10 s: two layers' nested `smoothsia` within the visit budget, on a polygon and the
  new multipoint. It is filed with its input in
  `docs/backlog/smoothsia-budget-outlasts-the-fuzz-timeout.md`. The last criterion is open until
  that is resolved and a run is clean.
- `docs/backlog/geomtransform-variants-unfuzzed.md` is deleted: what it observed no longer holds.
