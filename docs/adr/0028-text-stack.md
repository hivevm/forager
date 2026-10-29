# ADR-0028: Label glyphs are read with skrifa, laid out unshaped, and scaled as FreeType scales them unhinted

- **Status:** 🟡 proposed
- **Date:** 2026-10-29
- **Deciders:** Markus Brigl
- **Applies to:** `crates/label`, the text of `crates/engine` and `crates/render-agg`, the glyph operation of `crates/render`, the resource loader each host hands the engine, and every crate that lays out, measures, or draws text

## Context

G-10 holds every label to its reference position within 1 px, and G-13 holds the category text to
its reference image; both references come from MapServer at the pin, `rel-8-6-6`. Its text path
is FreeType, its own layout, and its AGG renderer. Read from its source at the pin:

- **Size.** `msComputeTextPath` in `src/maplabel.c` takes the label's size times the scale factor,
  held between `MINSIZE` and `MAXSIZE` times the resolution factor, and rounds it with `MS_NINT`,
  which is `lrint` on Linux and rounds half to even. A glyph is loaded at the pixel size
  `MS_NINT(glyph_size * 96 / 72)` (`FT_Set_Pixel_Sizes` in `src/fontcache.c`) with
  `FT_LOAD_NO_HINTING`, `FT_LOAD_NO_BITMAP`, and `FT_LOAD_IGNORE_GLOBAL_ADVANCE_WIDTH`. The line
  height is `ceil(glyph_size * 1.33)`.
- **Scaling.** Unhinted, FreeType's TrueType loader scales every point of a `glyf` outline with
  `FT_MulFix` by the size's scale, `FT_DivFix(ppem << 6, units_per_em)`, both rounding half away
  from zero; it shifts the outline by the scaled left phantom point, `FT_MulFix(xMin - lsb)`, and
  takes the advance as the difference of the two scaled phantom points. A composite glyph scales
  each component and each component's offset on its own. Fonts with CFF outlines and instances of
  variable fonts go through other arithmetic.
- **Layout.** `msLayoutTextSymbol` in `src/textlayout.c` gives a line made only of Latin-1
  characters no shaping: its "basic shaper" looks each character up in the font's character map
  and moves the pen by the glyph's unhinted advance, with no substitution, no positioning, and no
  kerning. HarfBuzz and FriBiDi run only for other scripts. Of the fonts a label's `FONT` lists,
  the first that has every glyph of the run is used; the last stands in with its `?` for a glyph
  none has.
- **Drawing.** `agg2RenderGlyphsPath` in `src/mapagg.cpp` turns each glyph's outline into an AGG
  path (`decompose_ft_outline`, each 26.6 coordinate divided by 64.0), rotates and moves it to the
  glyph's point, flattens its curves with `conv_curve`, and fills it non-zero with the scanline
  rasterizer that applies no gamma. An outline is drawn first, as `conv_contour` of the same
  path, its width `MS_NINT(OUTLINEWIDTH * glyph_size / SIZE)` plus 1 (`msDrawTextSymbol` in
  `src/maprendering.c`).

The specification assumes pure-Rust crates for this that build for `wasm32-unknown-unknown`
without C code and stay maintained, each verified by the ADR that picks it. skrifa reads fonts
with `read-fonts` and gives a glyph's outline and metrics in font units. agg-rust, the reference
renderer's rasterizer (ADR-0015), ports `conv_curve` and `conv_contour`. ADR-0009 already has a
core crate `label`, synchronous and without I/O, and makes `engine` the one core crate that awaits
the ports. The host hands the fonts over as it hands every other file the mapfile names (the
labels spec).

## Decision

We will read a label's glyphs with skrifa in the core crate `label`, lay out Latin text unshaped
as MapServer's basic shaper does, scale `glyf` outlines and advances from font units as FreeType's
TrueType loader scales them unhinted in 26.6 fixed point at MapServer's integer sizes, draw them
through the reference renderer as MapServer's AGG renderer draws glyph outlines, and load each
font file in the engine through the resource loader when a label first needs it.

- **Sizes.** The glyph size is the label's size held to its bounds and rounded half to even; the
  pixel size is `glyph_size * 96 / 72` rounded half to even; the line height is
  `ceil(glyph_size * 1.33)`.
- **Scaling.** The scale is `FT_DivFix(ppem << 6, units_per_em)`. Points, phantom points, and
  component offsets are `FT_MulFix` products, each rounded half away from zero, combined as
  FreeType's TrueType loader combines them unhinted; a coordinate is the 26.6 value divided by 64.
  skrifa reads the outlines and metrics unscaled; nothing is hinted.
- **Layout.** A line of Latin-1 characters maps each character through the font's character map
  and advances by the glyph's 26.6 advance; the font is the first of the label's list that has
  every glyph of the line, and the last list entry's `?` stands in for a glyph none has.
- **Drawing.** The display list carries a label's glyphs as contours of line, quadratic, and cubic
  segments in pixels, with its colour and its outline colour and width; `render-agg` flattens the
  curves with `conv_curve`, draws the outline first as `conv_contour` of width
  `round(OUTLINEWIDTH * glyph_size / SIZE) + 1`, rounded half to even, and fills the glyphs
  non-zero, without gamma.
- **Fonts.** The engine takes the resource loader, loads a font by the path the font set gives
  its name once per render when a label first needs it, and hands its bytes to `label`. A font
  that does not load, is past the size bound, or has no `glyf` outlines leaves its labels undrawn
  with a diagnostic naming the font, its path, and the label.
- **Crates.** skrifa is used by `label` alone, at the version `Cargo.lock` holds, under ADR-0012.

**Out of scope:** shaping, bidirectional text, and the scripts MapServer shapes with HarfBuzz and
FriBiDi, with the crates for them; fonts with CFF outlines and instances of variable fonts; the
size bound on a font file, a resource limit the maintainer sets with the fonts reaching the
engine; placement and the label cache; bitmap fonts; a font bundled with Forager.

## Alternatives considered

- **Shaping Latin text with harfrust or rustybuzz** — would apply the font's substitutions and
  positioning, ligatures among them, which MapServer never applies to such text.
- **swash** — reading and scaling in one crate, but its scaler is its own, and its shaper too.
- **skrifa's own scaling, in `f32`** — simpler, but its coordinates differ from FreeType's 26.6 in
  the last bits, and the differences add up along a line, against G-10's 1 px.
- **Hinting** — MapServer loads glyphs unhinted; hinting would move every stem away from its
  references.
- **Fonts loaded by each host ahead of the render** — every host would read every font of the font
  set, used or not, and enforce the containment of Q-8 and the bound of Q-9 itself.
- **A font port of its own** — a second port for what the resource loader already does: bytes by a
  path inside the base context.

## Sources / Prior art

- MapServer at `rel-8-6-6`: `src/maplabel.c` (`msComputeTextPath`), `src/fontcache.c`
  (`msGetGlyphByIndex`, `msGetGlyphOutline`), `src/textlayout.c` (`msLayoutTextSymbol`, its basic
  shaper, `check_single_font`), `src/mapagg.cpp` (`agg2RenderGlyphsPath`, `decompose_ft_outline`),
  `src/maprendering.c` (`msDrawTextSymbol`), `src/mapserver.h` (`MS_NINT`); read 2026-10-29.
- FreeType 2.13.3: `src/base/ftcalc.c` (`FT_MulFix`, `FT_DivFix`), `src/truetype/ttgload.c` (the
  unhinted scaling of points, phantom points, and composite offsets).
- crates.io, checked 2026-10-29: skrifa 0.49.0 (MIT OR Apache-2.0, updated 2026-10-21); harfrust
  0.15.0 and rustybuzz 0.20.1 for the shaping left out; swash 0.2.10.
- skrifa builds for `wasm32-unknown-unknown` with the pinned toolchain, with no `-sys` crate among
  its dependencies (`cargo check`, 2026-10-29).
- agg-rust 1.2.0: `conv_curve`, `conv_contour`, `path_storage`.

## Consequences

- Positive: a glyph's points and advances are MapServer's to the 1/64 px, so a label's position
  can match its reference within G-10's pixel.
- Positive: no shaper is needed for the scripts this spec covers; one font parser, read-fonts,
  comes with skrifa.
- Negative: the 26.6 arithmetic is Forager's own, a port of FreeType's TrueType loader, kept right
  by tests rather than by a library.
- Negative: a font with CFF outlines draws no label until a later decision covers it.
- Negative: the engine's render takes the resource loader, a change for every host.
- Follow-ups: the shaper and the bidirectional crate, with the spec of complex scripts; CFF fonts;
  the size bound on a font file.

## Enforcement

A test in `crates/arch-test` cites this ADR with `Verifies:` and fails when a crate other than
`label` depends on skrifa. Tests in `label` cite it: the 26.6 scale and products against values
worked from FreeType's definitions; the advance and outline points of a test font's simple glyph
whose `xMin` differs from its left side bearing, and of a composite glyph; the sizes, rounded half
to even; the font list's fallback and its `?`. A test in `engine` cites it: a font loaded once per
render, and one that fails leaving its labels undrawn with the diagnostic. Reference images in
`render-agg` cite it: the outline drawn before the fill, at its width.

**Not mechanically decidable:** that the whole path draws as MapServer draws, short of the
category text of G-13 and the positions of G-10, which the conformance harness holds once their
tickets land.
