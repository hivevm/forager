// The drawing of a TrueType symbol follows the TrueType branches of MapServer's
// msDrawMarkerSymbolInternal(), msImagePolylineMarkers(), and getTile(), and drawGlyphMarker(),
// src/maprendering.c, msGetMarkerSize() and msGetCharacterSize(), src/mapsymbol.c, and
// agg2RenderGlyphsPath(), src/mapagg.cpp, of the release pinned in scripts/fetch-msautotest.sh.
// Copyright (c) 1996-2005 Regents of the University of Minnesota.
// Copyright (c) 1996-2007 Regents of the University of Minnesota.
// Copyright (c) 1996-2011 Regents of the University of Minnesota.
// Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
// associated documentation files (the "Software"), to deal in the Software without restriction,
// including without limitation the rights to use, copy, modify, merge, publish, distribute,
// sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions: The above copyright notice and this
// permission notice shall be included in all copies of this Software or works derived from this
// Software. THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED,
// INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE
// AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

//! A TrueType symbol at a point, anchored, along a line, and tiled over a polygon: its glyph asked
//! of the port [`crate::glyph::Glyphs`], placed as drawGlyphMarker places it and drawn as
//! agg2RenderGlyphsPath draws it (ADR-draft-truetype-marker-glyph-through-a-port).

use std::rc::Rc;

use forager_render::{DrawOp, GlyphOutline, PathCommand, Point};
use forager_style_ir::c::lrint;
use forager_style_ir::{ANCHOR_CENTRE, Style, Symbol};

use crate::canvas::Canvas;
use crate::glyph::{Glyph, Glyphs, font_and_character};
use crate::line_marker::{Placement, taken};
use crate::style::fixed;
use crate::symbol::{self, Affine, SymbolStyle};
use crate::tile;

/// The size in points a TrueType marker's glyph is taken at: `MS_MAX(MS_NINT(scale), 1)`, held to
/// an `unsigned int`, as msDrawMarkerSymbolInternal hands it to msGetGlyphByIndex.
fn glyph_size(scale: f64) -> u32 {
    let rounded = lrint(scale).max(1);
    u32::try_from(rounded).unwrap_or(u32::MAX)
}

/// Where drawGlyphMarker puts the origin of a glyph whose control box, y down, is `bounds`, to
/// centre it on `at`, turned by `rotation` in radians: the centre of the box, turned where the
/// rotation is not 0. drawGlyphMarker computes the centre in the font's coordinates, y up, takes
/// it from `at` in x and adds it in y; its y is the negated centre of the box y down, exactly.
pub(crate) fn origin(
    at: Point,
    (min_x, min_y, max_x, max_y): (f64, f64, f64, f64),
    rotation: f64,
) -> Point {
    let ox = (max_x + min_x) / 2.0;
    let oy = -((max_y + min_y) / 2.0);
    if rotation != 0.0 {
        let (sina, cosa) = (libm::sin(rotation), libm::cos(rotation));
        let rox = ox * cosa - oy * sina;
        let roy = ox * sina + oy * cosa;
        Point {
            x: at.x - rox,
            y: at.y + roy,
        }
    } else {
        Point {
            x: at.x - ox,
            y: at.y + oy,
        }
    }
}

/// `path`, in pixels about the glyph's origin, y down, as the port gives it, already flipped as
/// decompose_ft_outline flips it, drawn from `origin` turned by `rotation`, as
/// agg2RenderGlyphsPath transforms it: turned by `-rotation` and moved to the origin.
fn placed(path: &[PathCommand], origin: Point, rotation: f64) -> Vec<PathCommand> {
    let matrix = Affine::rotation(-rotation).then(Affine::translation(origin.x, origin.y));
    let at = |p: &Point| matrix.apply(p.x, p.y);
    path.iter()
        .map(|command| match command {
            PathCommand::MoveTo(p) => PathCommand::MoveTo(at(p)),
            PathCommand::LineTo(p) => PathCommand::LineTo(at(p)),
            PathCommand::QuadTo(c, p) => PathCommand::QuadTo(at(c), at(p)),
            PathCommand::CubicTo(c0, c1, p) => PathCommand::CubicTo(at(c0), at(c1), at(p)),
            PathCommand::Close => PathCommand::Close,
        })
        .collect()
}

/// The glyph of the TrueType symbol `symbol`, at `index` from 1, at `size` points, asked of the
/// canvas's port, as msGetFontFace, msUTF8ToUniChar, msGetGlyphIndex, and msGetGlyphByIndex find
/// it. A symbol whose font or character is refused has none (see [`font_and_character`]), nor
/// one the port has none for. The canvas records why. One whose font lacks the character is
/// glyph 0, and the canvas records that too.
pub(crate) fn glyph_of(
    symbol: &Symbol,
    index: usize,
    size: u32,
    canvas: &mut Canvas,
) -> Option<Rc<Glyph>> {
    let asked = ask(canvas.glyphs, symbol, size);
    record(canvas, index, asked)
}

/// What [`glyph_of`] finds, before the canvas records it: the glyph and, where the font lacks the
/// character, what is drawn instead; or why there is none.
type Asked = Result<(Rc<Glyph>, Option<String>), String>;

/// The glyph of `symbol` at `size` points asked of `glyphs` (see [`glyph_of`]), nothing recorded.
fn ask(glyphs: Option<&dyn Glyphs>, symbol: &Symbol, size: u32) -> Asked {
    let (font, character) = font_and_character(symbol)?;
    let glyph = match glyphs {
        Some(glyphs) => glyphs.glyph(font, character, size)?,
        None => return Err("no fonts are kept where it is drawn".to_owned()),
    };
    let missing = glyph.missing.then(|| {
        format!(
            "the font `{font}` has no glyph for the character U+{:04X}; glyph 0 is drawn, or the \
             font's `?` where glyph 0 cannot be",
            u32::from(character)
        )
    });
    Ok((glyph, missing))
}

/// The glyph `asked` finds for the symbol at `index` from 1, the canvas recording why there is
/// none, or that its font lacks the character.
fn record(canvas: &mut Canvas, index: usize, asked: Asked) -> Option<Rc<Glyph>> {
    match asked {
        Ok((glyph, missing)) => {
            if let Some(missing) = missing {
                canvas.missing_glyphs.entry(index).or_insert(missing);
            }
            Some(glyph)
        }
        Err(why) => {
            canvas.refused_symbols.entry(index).or_insert(why);
            None
        }
    }
}

/// The glyph of the TrueType symbol `symbol`, at `index` from 1, at the size computeSymbolStyle's
/// `s` asks it at (see [`glyph_size`]); see [`glyph_of`].
pub(crate) fn styled_glyph(
    symbol: &Symbol,
    index: usize,
    s: &SymbolStyle,
    canvas: &mut Canvas,
) -> Option<Rc<Glyph>> {
    glyph_of(symbol, index, glyph_size(s.scale), canvas)
}

/// The glyph's control box, y down: `(min_x, min_y, max_x, max_y)`.
fn bounds(glyph: &Glyph) -> (f64, f64, f64, f64) {
    (glyph.min_x, glyph.min_y, glyph.max_x, glyph.max_y)
}

/// The width and height of `glyph`, of its control box, as msGetCharacterSize gives them to
/// msGetMarkerSize and msImagePolylineMarkers reads them of the glyph's metrics.
pub(crate) fn glyph_extent(glyph: &Glyph) -> (f64, f64) {
    (glyph.max_x - glyph.min_x, glyph.max_y - glyph.min_y)
}

/// `glyph` at the pixel `at` in computeSymbolStyle's `s`, as drawGlyphMarker draws it: placed by
/// [`origin`]; one glyph item, its outline the `OUTLINECOLOR` at the outline width cut to an
/// integer plus 1, its fill the `COLOR`.
pub(crate) fn glyph_op(glyph: &Glyph, at: Point, s: &SymbolStyle) -> DrawOp {
    let origin = origin(at, bounds(glyph), s.rotation);
    // drawGlyphMarker's `int olwidth`: the outline width cut toward zero.
    let outline = s.outline.map(|color| GlyphOutline {
        color,
        width: s.outline_width.trunc() + 1.0,
    });
    DrawOp::Glyphs {
        path: placed(&glyph.path, origin, s.rotation),
        color: s.fill,
        outline,
    }
}

/// The TrueType symbol `symbol`, at `index` from 1, at the pixel `at` in `style` at the canvas's
/// factors, as msDrawMarkerSymbol draws it: computeSymbolStyle's style, nothing without a colour
/// or at a scale of 0; moved by the style's offsets, and by its `ANCHORPOINT` of the size
/// msGetMarkerSize gives it, its character's bounds at [`symbol::marker_points`]
/// (msAdjustMarkerPos); its glyph (see [`styled_glyph`]) drawn by [`glyph_op`]. Where either
/// glyph is not found, nothing is drawn.
pub(crate) fn glyph_ops<T>(
    ops: &mut Vec<DrawOp>,
    at: Point,
    style: &Style<T>,
    index: usize,
    symbol: &Symbol,
    canvas: &mut Canvas,
) {
    let s = symbol::symbol_style(style, symbol, canvas.factors);
    if (s.fill.is_none() && s.outline.is_none()) || s.scale == 0.0 {
        return;
    }
    let factor = canvas.factors.scale;
    let mut at = symbol::shifted(at, style, factor);
    if symbol.anchor() != ANCHOR_CENTRE {
        let points = symbol::marker_points(style, fixed(&style.size), factor);
        let Some(sized) = glyph_of(symbol, index, points, canvas) else {
            return;
        };
        at = symbol::anchored(at, symbol, s.rotation, || glyph_extent(&sized));
    }
    if let Some(glyph) = styled_glyph(symbol, index, &s, canvas) {
        ops.push(glyph_op(&glyph, at, &s));
    }
}

/// The glyphs of the TrueType symbol `symbol`, at `index` from 1, of `style` along `paths`, in
/// pixels already offset, as msDrawLineSymbol hands them to msImagePolylineMarkers: the glyph
/// at computeSymbolStyle's size, its control box the marker's size, not held to 1 as another
/// symbol's is (`src/maprendering.c:402-413`), each marker drawn by [`glyph_op`]. `initial_gap`
/// is the `INITIALGAP` scaled as the width is. Each glyph costs one point of the canvas's budget.
pub(crate) fn line_glyphs<T>(
    paths: &[Vec<Point>],
    style: &Style<T>,
    index: usize,
    symbol: &Symbol,
    initial_gap: f64,
    canvas: &mut Canvas,
) -> Vec<DrawOp> {
    if canvas.markers.spent() {
        return Vec::new();
    }
    let s = symbol::symbol_style(style, symbol, canvas.factors);
    if (s.fill.is_none() && s.outline.is_none()) || s.scale == 0.0 {
        return Vec::new();
    }
    let Some(glyph) = styled_glyph(symbol, index, &s, canvas) else {
        return Vec::new();
    };
    let placement = Placement::of(
        style,
        (s.gap, s.rotation),
        glyph_extent(&glyph),
        symbol.anchor(),
        initial_gap,
        canvas.image(),
    );
    let mut ops = Vec::new();
    let mut each = s;
    for marker in taken(paths, &placement, 1, &mut canvas.markers) {
        each.rotation = marker.rotation;
        ops.push(glyph_op(&glyph, marker.at, &each));
    }
    ops
}

/// The TrueType symbol `symbol`, at `index` from 1, of `style` over `rings`, already moved by the
/// offset, as msDrawShadeSymbol tiles it: computeSymbolStyle's style at the canvas's factors, the
/// tile of its size, scale, and gap, never seamless (see [`tile::glyph_tile`]), and the polygon
/// filled with it. The glyph is asked only for a tile the cache does not hold, and drawn at the
/// tile's centre by [`glyph_op`]; a font that lacks the character is noted with the tile, so that
/// each layer that draws it records that. Where the glyph is not found or the tile is refused
/// (Q-9), nothing is drawn, and the canvas records why.
pub(crate) fn tiled_glyph<T>(
    ops: &mut Vec<DrawOp>,
    rings: Vec<Vec<Point>>,
    style: &Style<T>,
    index: usize,
    symbol: &Symbol,
    canvas: &mut Canvas,
) {
    let s = symbol::symbol_style(style, symbol, canvas.factors);
    let glyphs = canvas.glyphs;
    let mut refused = None;
    let mark = |at| match ask(glyphs, symbol, glyph_size(s.scale)) {
        Ok((glyph, missing)) => Some((glyph_op(&glyph, at, &s), missing)),
        Err(why) => {
            refused = Some(why);
            None
        }
    };
    let tile = match canvas.tiles.as_deref_mut() {
        Some(tiles) => tile::glyph_tile(tiles, index, &s, mark),
        None => Err("no tiles are kept where it is drawn".to_owned()),
    };
    match tile.map_err(Some).and_then(|tile| tile.ok_or(refused)) {
        Ok((tile, missing)) => {
            if let Some(missing) = missing {
                canvas.missing_glyphs.entry(index).or_insert(missing);
            }
            ops.push(DrawOp::TiledFill { rings, tile });
        }
        Err(Some(why)) => {
            canvas.refused_symbols.entry(index).or_insert(why);
        }
        Err(None) => {}
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use forager_style_ir::{Bound, SymbolKind, SymbolRef};

    use super::*;
    use crate::glyph::Glyphs;
    use crate::symbol::{Factors, marker_size_with};
    use crate::tile::Tiles;

    /// A port whose every glyph is a box from (1, -18) to (11, 2) y down, its path one move to
    /// the origin, glyph 0 where the second field says the font lacks the character, and which
    /// records the sizes it is asked at.
    #[derive(Default)]
    struct Boxes(RefCell<Vec<u32>>, bool);

    impl Glyphs for Boxes {
        fn glyph(&self, _: &str, _: char, size: u32) -> Result<Rc<Glyph>, String> {
            self.0.borrow_mut().push(size);
            Ok(Rc::new(Glyph {
                path: vec![PathCommand::MoveTo(Point { x: 0.0, y: 0.0 })],
                min_x: 1.0,
                min_y: -18.0,
                max_x: 11.0,
                max_y: 2.0,
                missing: self.1,
            }))
        }
    }

    fn truetype(anchor_point: Option<[f64; 2]>) -> Symbol {
        Symbol {
            name: Some("ttf".to_owned()),
            kind: SymbolKind::Truetype,
            paths: Vec::new(),
            extent: None,
            filled: false,
            anchor_point,
            character: Some("A".to_owned()),
            font: Some("sans".to_owned()),
            image: None,
            transparent: None,
            span: None,
        }
    }

    fn sized(size: f64) -> Style {
        Style {
            symbol: Some(Bound::Value(SymbolRef::Index(1))),
            size: Some(Bound::Value(size)),
            color: Some(Bound::Value(forager_style_ir::Rgba::opaque(0, 0, 0))),
            ..Style::default()
        }
    }

    #[test]
    fn a_truetype_marker_s_size_is_its_character_s_bounds_at_ms_get_marker_size_s_size() {
        // msGetMarkerSize cuts SIZE 10.7 at a factor of 1.5, 16.05, to 16 points and hands it to
        // msGetCharacterSize, whose bounds are 10 wide and 20 high.
        let symbols = [truetype(None)];
        let style = sized(10.7);
        let boxes = Boxes::default();
        let size = marker_size_with(
            &style,
            Some(10.7),
            &style.symbol,
            &symbols,
            None,
            Some(&boxes),
            1.5,
        );
        assert_eq!(size, Ok((10.0, 20.0)));
        assert_eq!(*boxes.0.borrow(), [16]);
        // Without the port, as a label's own marker is sized, it is not computed yet.
        let unknown =
            marker_size_with(&style, Some(10.7), &style.symbol, &symbols, None, None, 1.5);
        assert_eq!(
            unknown,
            Err("a truetype symbol, whose size is not computed yet".to_owned())
        );
    }

    #[test]
    fn a_truetype_marker_is_moved_by_its_anchor_point_as_ms_adjust_marker_pos_moves_it() {
        // ANCHORPOINT 0 0 moves the point by half the marker's size, (5, 10), to (105, 60); the
        // glyph is centred there by its box's centre, (6, -8) y down: its origin is (99, 68).
        let symbols = [truetype(Some([0.0, 0.0]))];
        let boxes = Boxes::default();
        let mut canvas =
            Canvas::new(&symbols, 200, 200, Factors::new(1.0, 1.0)).with_glyphs(&boxes);
        let mut ops = Vec::new();
        let at = Point { x: 100.0, y: 50.0 };
        glyph_ops(&mut ops, at, &sized(10.0), 1, &symbols[0], &mut canvas);
        let [DrawOp::Glyphs { path, .. }] = &ops[..] else {
            panic!("{ops:?}");
        };
        assert_eq!(path[0], PathCommand::MoveTo(Point { x: 99.0, y: 68.0 }));
        // The marker's size is asked at msGetMarkerSize's 10 points, the glyph at the style's 10.
        assert_eq!(*boxes.0.borrow(), [10, 10]);
    }

    #[test]
    fn a_glyph_s_size_is_its_scale_rounded_half_to_even_and_at_least_1() {
        assert_eq!(glyph_size(15.0), 15);
        assert_eq!(glyph_size(15.5), 16);
        assert_eq!(glyph_size(16.5), 16);
        assert_eq!(glyph_size(0.2), 1);
        // Past a long, lrint gives LONG_MIN, which MS_MAX lifts to 1, as in C; MAXSIZE keeps a
        // style's scale far below it.
        assert_eq!(glyph_size(1e30), 1);
    }

    #[test]
    fn a_glyph_is_centred_on_its_point_by_its_control_box_as_draw_glyph_marker_centres_it() {
        // Verifies: ADR-draft-truetype-marker-glyph-through-a-port
        // A box from (1, -2) to (11, 18) in the font's coordinates, y up, is (1, -18) to (11, 2)
        // y down. drawGlyphMarker's centre is (6, 8); unturned, the origin is (100 - 6, 50 + 8).
        let bounds = (1.0, -18.0, 11.0, 2.0);
        let at = Point { x: 100.0, y: 50.0 };
        assert_eq!(origin(at, bounds, 0.0), Point { x: 94.0, y: 58.0 });
        // Turned by 45 degrees: rox = 6 cos - 8 sin = -√2 and roy = 6 sin + 8 cos = 7√2, so the
        // origin is (100 + √2, 50 + 7√2).
        let turned = origin(at, bounds, std::f64::consts::FRAC_PI_4);
        let root2 = std::f64::consts::SQRT_2;
        assert!((turned.x - (100.0 + root2)).abs() < 1e-12, "{turned:?}");
        assert!(
            (turned.y - (50.0 + 7.0 * root2)).abs() < 1e-12,
            "{turned:?}"
        );
        // Turned by 90 degrees: rox = -8, roy = 6, the origin (108, 56).
        let turned = origin(at, bounds, std::f64::consts::FRAC_PI_2);
        assert!((turned.x - 108.0).abs() < 1e-12 && (turned.y - 56.0).abs() < 1e-12);
    }

    #[test]
    fn a_glyph_s_path_is_turned_and_moved_to_its_origin() {
        // The path y down, as the port gives it: (10, 20) up in the font is (10, -20).
        let path = [
            PathCommand::MoveTo(Point { x: 0.0, y: 0.0 }),
            PathCommand::LineTo(Point { x: 10.0, y: -20.0 }),
            PathCommand::Close,
        ];
        let origin = Point { x: 94.0, y: 58.0 };
        let unturned = placed(&path, origin, 0.0);
        assert_eq!(
            unturned,
            [
                PathCommand::MoveTo(origin),
                PathCommand::LineTo(Point { x: 104.0, y: 38.0 }),
                PathCommand::Close,
            ]
        );
        // Turned by 90 degrees counter-clockwise on the map: (10, -20) goes to (-20, -10) about
        // the origin, as AGG's rotation by -90 degrees takes it.
        let turned = placed(&path, origin, std::f64::consts::FRAC_PI_2);
        let PathCommand::LineTo(p) = turned[1] else {
            panic!("{turned:?}");
        };
        assert!(
            (p.x - 74.0).abs() < 1e-12 && (p.y - 48.0).abs() < 1e-12,
            "{p:?}"
        );
    }

    #[test]
    fn a_held_glyph_tile_asks_no_glyph_and_each_layer_records_its_missing_character() {
        // Two layers fill with one TrueType symbol whose font lacks the character: the first
        // builds the tile and asks the port; the second finds it held, asks nothing, and records
        // the missing character as the first did.
        let symbols = [truetype(None)];
        let boxes = Boxes(RefCell::default(), true);
        let mut tiles = Tiles::default();
        let square = vec![vec![
            Point { x: 0.0, y: 0.0 },
            Point { x: 50.0, y: 0.0 },
            Point { x: 50.0, y: 50.0 },
        ]];
        for layer in 0..2 {
            let mut canvas = Canvas::new(&symbols, 100, 100, Factors::new(1.0, 1.0))
                .with_glyphs(&boxes)
                .with_tiles(&mut tiles);
            let mut ops = Vec::new();
            tiled_glyph(
                &mut ops,
                square.clone(),
                &sized(8.0),
                1,
                &symbols[0],
                &mut canvas,
            );
            assert!(matches!(ops[..], [DrawOp::TiledFill { .. }]), "{ops:?}");
            assert_eq!(canvas.take_refusals().glyphs.len(), 1, "layer {layer}");
        }
        assert_eq!(*boxes.0.borrow(), [8], "the port is asked once");
    }
}
