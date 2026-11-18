# The render fuzz target builds no style GEOMTRANSFORM but an expression, and no labelpoly

**Observed:** ticket revision-2026-11-16/02 (2026-11-18). The check
`scripts/check-fuzz-variants.sh` could take `GeomTransform` and `LabelTransform` as it takes the
transform tree and `SymbolKind`, but it would fail on the tree as it stands:
`fuzz/fuzz_targets/render_input.rs`, what the render target builds, names
`GeomTransform::Expression` only, none of `Start`, `End`, `Vertices`, `Bbox`, `Centroid`,
`LabelPoint`, `LabelPoly`, and `LabelCenter`, and of `LabelTransform` it names `Point` and
`Center`, not `Poly`. No other fuzz target builds a style IR with them: the `mapfile` target loads
a mapfile without drawing it, and the `geojson` target draws a fixed map. Grep found them in no
target; whether the browser or another path reaches them with arbitrary numbers was not probed.

**Where:** `fuzz/fuzz_targets/render_input.rs`, `crates/style-ir/src/model.rs` (`GeomTransform`,
`LabelTransform`), the `ENUMS` list of `scripts/check-fuzz-variants.sh`; Q-3 of
`docs/SPECIFICATION.md`, the fuzz seed convention of `docs/CONVENTIONS.md`.

**What a change weighs:**

- **Build them in the render target**, from bytes the target does not read yet, with a seed that
  reaches each, and add both enums to `ENUMS`: the check then holds them as it holds the
  transform tree. Every existing seed decodes to a different render, as each change to how the
  target decodes its bytes does, and the convention has each seed run again.
- **Leave them to the convention:** no work. The point and line transforms draw through the
  marker and line paths the target already fuzzes, but the pixel bounds of `Bbox` and the label
  box of `LabelPoly` are their own code.
