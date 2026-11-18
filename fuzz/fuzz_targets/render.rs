//! The renderer on structured input: arbitrary points, lines, and polygons, an arbitrary
//! vector or ellipse symbol at the points, and a hatch over the polygons at the symbol's size,
//! the line width, the symbol's angle, and the first further style's pattern, with any `f64` —
//! NaN, infinities, and the extremes included — in an arbitrary view, drawn by the engine with
//! agg-rust behind its clipping (Q-3, ADR-0025). JSON cannot carry such numbers, so the GeoJSON
//! target never reaches these paths. Further styles of a line's dashes, caps, joins, offsets,
//! outlines, width bounds, and the symbol placed along it by a gap, or brushed along it with its
//! tile without one, are read from the same bytes back to front, so that they leave how an input
//! decodes to its view, its symbol, and its shapes as it was. The points carry a label of
//! arbitrary text, size, bounds, and resolution, in a test font of the label crate, drawn through
//! the text path (ADR-0031). The second half of the bytes is read once more for the features'
//! properties, a filter, a class expression, and a computed size and colour, as arbitrary trees
//! (ADR-0016), which every layer evaluates. A pixmap symbol, a small PNG every image path loads
//! as, is drawn at the points, along the lines at a gap or brushed along them without one, and
//! tiled over the polygons, at an arbitrary size, angle, and gap, so that its resampling, the
//! tiled fill, and the brushed line are fuzzed too (ADR-0029, ADR-0026). The bytes back to front
//! are read once more for the layers' limit on their features, which layers are drawn after the
//! label cache, the cache's edge buffer, and the pixmap as the label's labelpoint and labelcenter
//! marker. The last third of the bytes is read once more for a layer's `GEOMTRANSFORM`, a
//! transform tree of up to eight functions of arbitrary numbers, `[map_cellsize]` among them,
//! which the polygon and line layers apply to their features (ADR-0048); its steps include
//! `simplify` and `buffer` (ADR-0041) and `simplifypt` (ADR-0047). The first third of the bytes,
//! back to front, is read once more for what the fidelity spec added: a class style's
//! `GEOMTRANSFORM` tree of the same steps on the polygon and line layers (ADR-0048), the map's
//! `ANGLE` with or without a projection of the map and its layers (ADR-0045), a circle layer,
//! `ANTIALIAS FALSE` on the first style, and a TrueType symbol at the points, along the lines,
//! and over the polygons, its glyph drawn through the glyph port (ADR-0044) from the test font or
//! from a font of the fuzz bytes. What the target builds of its bytes is `render_input.rs`. A
//! field added to a view changes what every older seed decodes to; `fuzz/tests/render_seeds.rs`
//! holds each seed to the path it was made for.
// Verifies: Q-3, ADR-0025, ADR-0016, ADR-0046, ADR-0041, ADR-0044, ADR-0045, ADR-0047, ADR-0048
#![no_main]

mod expr_input;
mod render_input;

use forager_render_agg::AggRenderer;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Some(decoded) = render_input::decode(data) {
        let _ = decoded.render(AggRenderer::new());
    }
});
