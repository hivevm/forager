# ADR-0031: Label glyphs are drawn from skrifa's unhinted FreeType outlines, laid out unshaped as MapServer lays out its Latin lines

- **Status:** 🟢 accepted
- **Date:** 2026-10-29
- **Deciders:** Markus Brigl
- **Applies to:** `crates/label`, the text of `crates/engine` and `crates/render-agg`, the glyph operation of `crates/render`, the resource loader each host hands the engine, and every crate that lays out, measures, or draws text

## Context

G-10 holds every label to its reference position within 1 px, and G-13 holds the category text to
its reference image; both references come from MapServer at the pin, `rel-8-6-6`, built with
FreeType, HarfBuzz, and FriBiDi ([ADR-0033](0033-noise-measurement.md)). Read from its source at
the pin:

- **Size.** `msComputeTextPath` in `src/maplabel.c` takes the label's size times the scale factor,
  held between `MINSIZE` and `MAXSIZE` times the resolution factor, and rounds it with `MS_NINT`,
  which is `lrint` and rounds half to even. `src/fontcache.c` sets the pixel size
  `MS_NINT(glyph_size * 96 / 72)` and loads each glyph with `FT_LOAD_NO_HINTING`,
  `FT_LOAD_NO_BITMAP`, and `FT_LOAD_IGNORE_GLOBAL_ADVANCE_WIDTH`, from face 0 of the file, at a
  variable font's default instance. A font without a Unicode character map is read through its
  Symbol or its Apple Roman map. The line height is `ceil(glyph_size * 1.33)`.
- **Scaling.** Unhinted, FreeType's TrueType loader scales a `glyf` outline in 26.6 fixed point:
  the scale is `FT_DivFix(ppem << 6, units_per_em)`, every point, phantom point, and component
  offset an `FT_MulFix` product, the outline shifted by the left phantom point, and the advance
  the difference of the two phantom points.
- **Layout.** `msLayoutTextSymbol` in `src/textlayout.c` gives a line whose code points all lie
  below U+02B0 its "basic shaper": each character is looked up in the font's character map and
  the pen moves by the glyph's unhinted advance, with no substitution, positioning, or kerning.
  Any other line goes through FriBiDi and is split by script; its runs of Latin or Common script
  take the basic shaper too, and HarfBuzz shapes the rest.
- **Fonts of a label.** `get_face_for_run` tries the entries of the label's `FONT` list prefixed
  `en:` first for such a line, then the list in order, each name with its `xx:` prefix stripped;
  an entry the font set lacks or that does not open is skipped. The first entry that has every
  glyph of the run is used; failing that, the last entry is, and its `?`, or glyph 0 where it has
  none, stands in for each glyph it lacks.
- **Drawing.** `agg2RenderGlyphsPath` in `src/mapagg.cpp` turns each glyph's outline into an AGG
  path (`decompose_ft_outline`, each 26.6 coordinate divided by 64.0), rotates and moves it to the
  glyph's point, flattens its curves with `conv_curve`, and fills it non-zero with the scanline
  rasterizer that applies no gamma. An outline is drawn first, as `conv_contour` of the same path,
  `ow + 1` wide, where `ow = MS_NINT(OUTLINEWIDTH * (glyph_size / SIZE))` (`msDrawTextSymbol` in
  `src/maprendering.c`).

skrifa, which reads fonts with `read-fonts`, draws an unhinted `glyf` outline in its FreeType path
style with FreeType's arithmetic: `font-types` implements `FT_MulFix` and `FT_DivFix` bit for bit,
and its TrueType scaler shifts by the phantom point, takes the advance from the phantom points, and
scales composites as FreeType's loader does. It differs in one place: a component whose offset is
scaled by its transform (`SCALED_COMPONENT_OFFSET`) with rotation or skew takes an approximate
`hypot` where FreeType calls `FT_Hypot`. agg-rust, the reference renderer's rasterizer
([ADR-0025](0025-agg-rust-rasterizer.md)), ports `conv_curve` and `conv_contour`.
[ADR-0008](0008-ports-and-adapters-crates.md) has a core crate `label`, synchronous and without
I/O, and makes `engine` the one core crate that awaits the ports. Q-9 bounds a font file at
32 MiB.

## Decision

We will draw a label's glyphs from skrifa's unhinted outlines in its FreeType path style, in the
core crate `label`, at MapServer's integer sizes, lay out a line whose code points all lie below
U+02B0 unshaped as MapServer's basic shaper does, draw them through the reference renderer as
MapServer's AGG renderer draws glyph outlines, and load each font file in the engine through the
resource loader when a label first needs it.

- **Sizes.** The glyph size is the label's size times the scale factor, held between `MINSIZE` and
  `MAXSIZE` times the resolution factor, rounded half to even. The pixel size is
  `glyph_size * 96 / 72` rounded half to even. The line height is `ceil(glyph_size * 1.33)`.
- **Glyphs.** skrifa draws each glyph unhinted at the pixel size, at the default instance of face
  0, in its FreeType path style, and gives its advance. Nothing may be hinted. A coordinate is the
  26.6 value divided by 64.
- **Layout.** A line whose code points all lie below U+02B0 maps each character through the font's
  Unicode character map and advances by the glyph's advance. Any other line must not be drawn.
  Its label gets a diagnostic naming the line, until the spec of complex scripts decides it.
- **Fonts of a label.** The entries of the label's `FONT` list are tried as `get_face_for_run`
  tries them for such a line:
  - those prefixed `en:` first, then all in order, prefixes stripped;
  - an entry the font set lacks or that does not load is skipped;
  - the first entry with every glyph is used;
  - failing that, the last entry is used, with its `?`, or glyph 0, for each glyph it lacks.
- **Drawing.** The display list carries a label's glyphs as contours of line, quadratic, and cubic
  segments in pixels, with its colour and its outline colour and width. `render-agg` flattens the
  curves with `conv_curve` and draws the outline first, as `conv_contour` of width
  `round_half_even(OUTLINEWIDTH * (glyph_size / SIZE)) + 1`, computed in `f64` in that order. It
  then fills the glyphs non-zero, without gamma.
- **Fonts.** The engine takes the resource loader. It loads a font by the path the font set gives
  its name, once per render, when a label first needs it, and hands its bytes to `label`. A label
  none of whose fonts can be used must not be drawn, with a diagnostic naming the font, its path,
  and the label. A font cannot be used when the last entry does not load, has no `glyf` outlines,
  or has no Unicode character map.
- **Crates.** skrifa must be used by `label` alone, at the version `Cargo.lock` holds, under
  [ADR-0011](0011-crate-supply-chain.md).

**Out of scope:** shaping, bidirectional text, and the scripts MapServer runs through FriBiDi and
HarfBuzz, with their crates; fonts with CFF outlines; fonts read through a Symbol or Apple Roman
character map; the size bound on a font file, which Q-9 sets; placement and the label cache; bitmap
fonts; a font bundled with Forager.

## Alternatives considered

- **Shaping the Latin lines with harfrust or rustybuzz** — would apply the font's substitutions and
  positioning, ligatures among them, which MapServer never applies to such a line.
- **FreeType's TrueType arithmetic ported by hand onto skrifa's unscaled outlines** — what skrifa's
  FreeType path style already does, kept a second time.
- **swash** — reading and scaling in one crate, but its scaler is its own, and its shaper too.
- **skrifa's `f32` path style** — rounds otherwise than FreeType, so its coordinates leave
  MapServer's in the last bits, and the differences add up along a line.
- **Hinting** — MapServer loads glyphs unhinted; hinting would move every stem away from its
  references.
- **Fonts loaded by each host ahead of the render** — every host would read every font of the font
  set, used or not, and enforce the containment of Q-8 and the bound of Q-9 itself.
- **A font port of its own** — a second port for what the resource loader already does: bytes by a
  path inside the base context.

## Sources / Prior art

- MapServer at `rel-8-6-6`: `src/maplabel.c` (`msComputeTextPath`), `src/fontcache.c`
  (`msGetFontFace`, `msGetGlyphIndex`, `msGetGlyphByIndex`, `msGetGlyphOutline`),
  `src/textlayout.c` (`unicode_is_latin1`, `msLayoutTextSymbol` and its basic shaper,
  `get_face_for_run`, `check_single_font`), `src/mapagg.cpp` (`agg2RenderGlyphsPath`,
  `decompose_ft_outline`), `src/maprendering.c` (`msDrawTextSymbol`), `src/mapserver.h`
  (`MS_NINT`); read 2026-10-29.
- FreeType 2.13.3: `src/base/ftcalc.c` (`FT_MulFix`, `FT_DivFix`), `src/truetype/ttgload.c` (the
  unhinted scaling of points, phantom points, and composite offsets, and `FT_Hypot` for scaled
  component offsets).
- skrifa 0.49.0 (`src/outline/glyf/mod.rs`, its FreeType path style) and font-types 0.12.7
  (`src/fixed.rs`), read 2026-10-29; skrifa is MIT OR Apache-2.0, updated 2026-10-21 on
  crates.io, and builds for `wasm32-unknown-unknown` with the pinned toolchain, with no `-sys`
  crate among its dependencies.
- crates.io, checked 2026-10-29: harfrust 0.15.0, rustybuzz 0.20.1, swash 0.2.10.
- agg-rust 1.2.0: `conv_curve`, `conv_contour`, `path_storage`.

## Consequences

- Positive: a glyph's points and advances are MapServer's to the 1/64 px, so a label's position
  can match its reference within G-10's pixel, with FreeType's arithmetic kept by skrifa, not by
  Forager.
- Positive: no shaper is needed for the lines below U+02B0; one font parser comes with skrifa.
- Negative: a composite glyph with a transformed, scaled offset may stand 1/64 px off; such
  components are rare in text fonts, and the harness would show it.
- Negative: a line with a code point at or above U+02B0, a typographic apostrophe or a dash among
  them, draws no label until the spec of complex scripts.
- Negative: a font with CFF outlines draws no label until a later decision covers it.
- Negative: the engine's render takes the resource loader, which every host hands it.
- Follow-ups: the shaper, the bidirectional crate, and the lines at or above U+02B0, with the spec
  of complex scripts; CFF fonts; symbol fonts.

## Enforcement

Tests cite this ADR with `Verifies:`:

- in `crates/arch-test/tests/dependencies.rs`, a test fails when a crate other than `label`
  depends on skrifa;
- in `crates/label/tests/text.rs`, a test font's simple glyph whose `xMin` differs from its left
  side bearing, and a composite glyph, drawn at a size, against points and advances worked from
  FreeType's definitions; the sizes rounded half to even; the gate at U+02B0; the font list's
  `en:` prefix, its skipped entries, and the last entry's `?`; the outline width in its order;
- in `crates/engine/tests/fonts.rs`, a font loaded once per render, and a label whose last font
  fails left undrawn with the diagnostic;
- reference images in `crates/render-agg/src/lib.rs` and `crates/cli/tests/render.rs`: the
  outline drawn before the fill, at its width.

**Not mechanically decidable:** that the whole path draws as MapServer draws, beyond the cases of
the category text of G-13 and the label positions of G-10 that the conformance harness holds.
