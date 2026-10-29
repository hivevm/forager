# ADR-0028: Labels are shaped with harfrust and drawn from skrifa's outlines, scaled as FreeType scales them unhinted

- **Status:** 🟡 proposed
- **Date:** 2026-10-29
- **Deciders:** Markus Brigl
- **Applies to:** `crates/label`, the text of `crates/engine` and `crates/render-agg`, the glyph operation of `crates/render`, and every crate that shapes, measures, or draws text

## Context

G-10 holds every label to its reference position within 1 px, and G-13 holds the category text to
its reference image; both references come from MapServer at the pin, `rel-8-6-6`. Its text path
is FreeType, HarfBuzz, and its AGG renderer. Read from its source at the pin:

- **Size.** A glyph is loaded at the integer pixel size `MS_NINT(size * 96 / 72)`
  (`FT_Set_Pixel_Sizes` in `src/fontcache.c`), with `FT_LOAD_NO_HINTING`, `FT_LOAD_NO_BITMAP`, and
  `FT_LOAD_IGNORE_GLOBAL_ADVANCE_WIDTH`: unhinted outlines and metrics, scaled by FreeType in 26.6
  fixed point. A glyph the font lacks is drawn as its `?`.
- **Shaping.** `hb_shape` runs with the features `kern` and `vert` turned off (`hbfeatures` in
  `src/textlayout.c`); a glyph's advance comes from a callback that returns FreeType's unhinted
  advance, and a position's offsets are divided by 64 as integers. Right-to-left runs go through
  FriBiDi first.
- **Drawing.** `agg2RenderGlyphsPath` in `src/mapagg.cpp` turns each glyph's outline into an AGG
  path (`decompose_ft_outline`, each 26.6 coordinate divided by 64.0), flattens its curves with
  `conv_curve`, and fills it non-zero; an outline is `conv_contour` of the same path, `OUTLINEWIDTH`
  plus 1 wide, drawn first.

The specification assumes pure-Rust crates for this that build for `wasm32-unknown-unknown`
without C code and stay maintained, each verified by the ADR that picks it; it names rustybuzz.
rustybuzz has had no release since 0.20.1 in November 2024; the HarfBuzz project now maintains
harfrust, a port of HarfBuzz's shaper onto `read-fonts`, the parser skrifa reads with. agg-rust,
the reference renderer's rasterizer (ADR-0015), ports `conv_curve` and `conv_contour`. ADR-0009
already has a core crate `label`, synchronous and without I/O, and makes `engine` the one core
crate that awaits the ports. The host hands the fonts over as it hands every other file the
mapfile names (the labels spec).

## Decision

We will shape a label's text with harfrust and read its glyphs with skrifa, in the core crate
`label`, scale glyph outlines and advances from font units as FreeType scales them unhinted in
26.6 fixed point at MapServer's integer pixel size, draw them through the reference renderer as
MapServer's AGG renderer draws glyph outlines, and load each font file in the engine through the
resource loader when a label first needs it.

- **Scaling.** The pixel size is `round(size * 96 / 72)`. The scale is FreeType's
  `FT_DivFix(ppem * 64, units_per_em)`; every outline point and every advance is
  `FT_MulFix(value, scale)`, rounded as FreeType rounds, in 26.6, and becomes a coordinate by
  dividing by 64. skrifa reads the outlines unscaled; no hinting is applied.
- **Shaping.** harfrust shapes each run with `kern` and `vert` off; the advance of each glyph is
  replaced by its 26.6 advance, and its offsets are scaled to 26.6 and divided by 64 as integers.
  A glyph the font lacks is the font's `?`. Bidirectional text is not shaped yet.
- **Drawing.** The display list carries a glyph run as contours of line, quadratic, and cubic
  segments in pixels, with its colour and its outline colour and width; `render-agg` flattens the
  curves with `conv_curve`, fills them non-zero, and draws the outline first as `conv_contour` of
  width `OUTLINEWIDTH + 1`.
- **Fonts.** The engine takes the resource loader, loads a font by the path the font set gives
  its name, once per render, when a label first needs it, and hands its bytes to `label`. A font
  that does not load, or past the size bound, leaves its labels undrawn with a diagnostic naming
  the font and its path.
- **Crates.** harfrust and skrifa are used by `label` alone, at the versions `Cargo.lock` holds,
  under ADR-0012.

**Out of scope:** the size bound on a font file, a resource limit the maintainer sets with the
fonts reaching the engine; placement and the label cache; bidirectional and complex scripts and
the crate for them; bitmap fonts; a font bundled with Forager.

## Alternatives considered

- **rustybuzz** — the crate the specification names, but without a release for nearly two years;
  it parses with ttf-parser, a second font parser beside skrifa.
- **swash** — shaping and scaling in one crate, but its shaper is its own, not HarfBuzz's, so its
  glyph choice and positions need not be MapServer's.
- **skrifa's own scaling, in `f32`** — simpler, but its coordinates differ from FreeType's 26.6 in
  the last bits, and the differences add up along a line, against G-10's 1 px.
- **Hinting** — MapServer loads glyphs unhinted; hinting would move every stem away from its
  references.
- **Kerning on** — HarfBuzz's default, which MapServer turns off.
- **Fonts loaded by each host ahead of the render** — every host would read every font of the font
  set, used or not, and enforce the containment of Q-8 and the bound of Q-9 itself.
- **A font port of its own** — a second port for what the resource loader already does: bytes by a
  path inside the base context.

## Sources / Prior art

- MapServer at `rel-8-6-6`: `src/fontcache.c` (`msGetGlyphByIndex`, `msGetGlyphOutline`),
  `src/textlayout.c` (`hbfeatures`, `_ms_get_glyph_h_advance_func`, `get_hb_font`, the shaping
  loop), `src/mapagg.cpp` (`agg2RenderGlyphsPath`, `decompose_ft_outline`); read 2026-10-29.
- crates.io, checked 2026-10-29: harfrust 0.15.0 (MIT, updated 2026-10-22), skrifa 0.49.0 (MIT OR
  Apache-2.0, updated 2026-10-21), rustybuzz 0.20.1 (MIT, last updated 2024-11-12), swash 0.2.10.
- Both build for `wasm32-unknown-unknown` with the pinned toolchain, with no `-sys` crate among
  their dependencies (`cargo check`, 2026-10-29).
- agg-rust 1.2.0: `conv_curve`, `conv_contour`, `path_storage`.
- FreeType's `FT_MulFix` and `FT_DivFix`, `src/base/ftcalc.c`.

## Consequences

- Positive: a glyph's points and advances are MapServer's to the 1/64 px, so a label's position
  can match its reference within G-10's pixel.
- Positive: one font parser, read-fonts, under both crates, maintained by the HarfBuzz and
  Fontations projects.
- Negative: the 26.6 arithmetic is Forager's own, a port of FreeType's, and must be kept right by
  tests rather than by a library.
- Negative: the specification's Assumption names rustybuzz; it changes to harfrust with this
  decision.
- Negative: the engine's render takes the resource loader, a change for every host.
- Follow-ups: the crate for bidirectional text, with the spec of complex scripts; the size bound
  on a font file.

## Enforcement

The dependency test of ADR-0009 fails when a crate other than `label` depends on harfrust or
skrifa. Tests in `label` cite this ADR with `Verifies:`: the 26.6 scale and products against
values worked from FreeType's definitions, a glyph's advance and outline points of a test font at
a size against those values, and a shaped run with `kern` off against one with it on.

**Not mechanically decidable:** that the whole path draws as MapServer draws, short of the
category text of G-13 and the positions of G-10, which the conformance harness holds once their
tickets land.
