// The tests of the drawing of a line work their values out from MapServer's msDrawLineSymbol()
// and msOutlineRenderingPrepareStyle(), src/maprendering.c, of the release pinned in
// scripts/fetch-msautotest.sh.
// Copyright (c) 1996-2005 Regents of the University of Minnesota.
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

use std::rc::Rc;

use forager_render::{Dashes, DrawOp, InnerJoin, LineCap, LineJoin, Point, PremultipliedImage};
use forager_style_ir::{Bound, LineOffset, Rgba, Style, Symbol, SymbolKind, SymbolRef};

use super::*;
use crate::canvas::Canvas;
use crate::line::Pass;
use crate::line_marker::MarkerBudget;
use crate::style::fixed;
use crate::symbol::Pixmaps;
use crate::tile::Tiles;

const BLUE: Rgba = Rgba::opaque(30, 60, 200);
const BLACK: Rgba = Rgba::opaque(0, 0, 0);

fn p(x: f64, y: f64) -> Point {
    Point { x, y }
}

/// A canvas without symbols, of 100 × 100.
fn plain() -> Canvas<'static> {
    Canvas::new(&[], 100, 100, Factors::new(1.0, 1.0))
}

fn line() -> Vec<Vec<Point>> {
    vec![vec![p(0.0, 0.0), p(10.0, 0.0)]]
}

fn colored(color: Option<Rgba>, outline: Option<Rgba>) -> Style {
    Style {
        color: color.map(Bound::Value),
        outline_color: outline.map(Bound::Value),
        ..Style::default()
    }
}

/// The strokes of one pass of a style on [`line`]: color, width, dashes, and paths.
fn strokes(style: &Style, pass: Pass) -> Vec<(Rgba, f64, Dashes, Vec<Vec<Point>>)> {
    let mut ops = Vec::new();
    line_symbol(&mut ops, line(), style, pass, &mut plain());
    ops.into_iter()
        .map(|op| match op {
            DrawOp::Stroke {
                color,
                width,
                dashes,
                paths,
                ..
            } => (color, width, dashes, paths),
            DrawOp::Fill { .. }
            | DrawOp::Hatch(_)
            | DrawOp::Glyphs { .. }
            | DrawOp::Pixmap { .. }
            | DrawOp::ResampledPixmap { .. }
            | DrawOp::TiledFill { .. }
            | DrawOp::BrushedLine { .. } => {
                panic!("a line is not filled")
            }
        })
        .collect()
}

#[test]
fn the_bound_on_the_width_scales_the_pattern_the_initial_gap_and_the_offset() {
    // WIDTH 10 bounded by MAXWIDTH 5: msDrawLineSymbol's finalscalefactor is 0.5.
    let style = Style {
        width: Some(Bound::Value(10.0)),
        max_width: Some(5.0),
        pattern: vec![20.0, 8.0],
        initial_gap: Some(6.0),
        line_offset: Some(LineOffset {
            distance: Bound::Value(4.0),
            both_sides: false,
        }),
        ..colored(Some(BLUE), None)
    };
    let drawn = strokes(&style, Pass::Plain);
    let (_, width, dashes, paths) = &drawn[0];
    assert_eq!(*width, 5.0);
    assert_eq!(dashes.pattern, vec![10.0, 4.0]);
    assert_eq!(dashes.offset, 3.0);
    assert_eq!(paths[0], vec![p(0.0, 2.0), p(10.0, 2.0)]);
    // MINWIDTH raises the width and scales alike.
    let raised = Style {
        width: Some(Bound::Value(2.0)),
        min_width: Some(4.0),
        pattern: vec![3.0, 1.0],
        ..colored(Some(BLUE), None)
    };
    assert_eq!(strokes(&raised, Pass::Plain)[0].2.pattern, vec![6.0, 2.0]);
}

#[test]
fn an_outline_pass_of_symbols_grows_their_width_and_size_and_swaps_their_colors() {
    // msOutlineRenderingPrepareStyle at a scale factor of 1: the width and its bounds by
    // twice the outline width, the size by it, MapServer's -1 where none is given.
    let style = Style {
        width: Some(Bound::Value(2.0)),
        size: Some(Bound::Value(6.0)),
        outline_width: Some(Bound::Value(1.5)),
        ..colored(Some(BLUE), Some(BLACK))
    };
    let prepared = outline_style(&style, Factors::new(1.0, 1.0));
    assert_eq!(fixed(&prepared.width), Some(5.0));
    assert_eq!(
        (prepared.min_width, prepared.max_width),
        (Some(3.0), Some(35.0))
    );
    assert_eq!(fixed(&prepared.size), Some(7.5));
    assert_eq!(
        (fixed(&prepared.color), fixed(&prepared.outline_color)),
        (Some(BLACK), Some(BLUE))
    );
    let without_size = Style {
        size: None,
        ..style
    };
    assert_eq!(
        fixed(&outline_style(&without_size, Factors::new(1.0, 1.0)).size),
        Some(0.5)
    );
}

#[test]
fn an_outline_pass_grows_the_style_s_own_width_bounds() {
    // msOutlineRenderingPrepareStyle grows the style's MINWIDTH and MAXWIDTH where it has
    // them, and MapServer's defaults, 0 and 32, where it has none.
    let style = Style {
        min_width: Some(4.0),
        max_width: Some(10.0),
        outline_width: Some(Bound::Value(1.5)),
        ..colored(Some(BLUE), Some(BLACK))
    };
    let prepared = outline_style(&style, Factors::new(1.0, 1.0));
    assert_eq!(
        (prepared.min_width, prepared.max_width),
        (Some(7.0), Some(13.0))
    );
    let defaults = outline_style(
        &Style {
            min_width: None,
            max_width: None,
            ..style
        },
        Factors::new(1.0, 1.0),
    );
    assert_eq!(
        (defaults.min_width, defaults.max_width),
        (Some(3.0), Some(35.0))
    );
}

#[test]
fn a_symbol_index_past_the_last_draws_no_line() {
    // msDrawLineSymbol returns at once for a symbol it does not have.
    let style = Style {
        symbol: Some(Bound::Value(SymbolRef::Index(9))),
        ..colored(Some(BLUE), None)
    };
    assert!(strokes(&style, Pass::Plain).is_empty());
}

#[test]
fn a_line_without_a_color_is_drawn_in_its_outline_color() {
    let drawn = strokes(&colored(None, Some(BLACK)), Pass::Plain);
    assert_eq!(drawn[0].0, BLACK);
    assert!(strokes(&colored(None, None), Pass::Plain).is_empty());
}

#[test]
fn an_outline_pass_widens_the_line_and_swaps_its_colors() {
    let style = Style {
        width: Some(Bound::Value(6.0)),
        outline_width: Some(Bound::Value(2.0)),
        ..colored(Some(BLUE), Some(BLACK))
    };
    let drawn = strokes(&style, Pass::Outline);
    assert_eq!((drawn[0].0, drawn[0].1), (BLACK, 10.0));
    // The first style's outline falls back to its colour, as the swapped style does.
    let uncolored_outline = Style {
        outline_width: Some(Bound::Value(2.0)),
        ..colored(Some(BLUE), None)
    };
    assert_eq!(strokes(&uncolored_outline, Pass::Outline)[0].0, BLUE);
    // A later style draws its outline only with an OUTLINECOLOR, itself only with a COLOR.
    assert!(strokes(&uncolored_outline, Pass::OutlineIfOutlined).is_empty());
    assert!(strokes(&colored(None, Some(BLACK)), Pass::PlainIfColored).is_empty());
}

#[test]
fn a_line_no_wider_than_a_pixel_has_butt_caps_and_bevel_joins_and_inner_joins() {
    // agg2RenderLine applies the style's cap and join only to a line wider than a pixel, and
    // bevels the inner joins of any other; a wider line keeps AGG's default inner miter.
    let style = |width| Style {
        width: Some(Bound::Value(width)),
        line_cap: Some(LineCap::Square),
        line_join: Some(LineJoin::Miter),
        ..colored(Some(BLUE), None)
    };
    let ends = |width| {
        let mut ops = Vec::new();
        line_symbol(&mut ops, line(), &style(width), Pass::Plain, &mut plain());
        match &ops[0] {
            DrawOp::Stroke {
                cap,
                join,
                inner_join,
                ..
            } => (*cap, *join, *inner_join),
            DrawOp::Fill { .. }
            | DrawOp::Hatch(_)
            | DrawOp::Glyphs { .. }
            | DrawOp::Pixmap { .. }
            | DrawOp::ResampledPixmap { .. }
            | DrawOp::TiledFill { .. }
            | DrawOp::BrushedLine { .. } => {
                panic!("a line is not filled")
            }
        }
    };
    assert_eq!(
        ends(1.0),
        (LineCap::Butt, LineJoin::Bevel, InnerJoin::Bevel)
    );
    assert_eq!(
        ends(1.5),
        (LineCap::Square, LineJoin::Miter, InnerJoin::Miter)
    );
}

/// A filled circle, symbol 1.
fn circle() -> Symbol {
    Symbol {
        name: None,
        kind: SymbolKind::Ellipse,
        paths: vec![vec![[1.0, 1.0]]],
        extent: Some([1.0, 1.0]),
        filled: true,
        anchor_point: None,
        character: None,
        font: None,
        image: None,
        transparent: None,
        span: None,
    }
}

/// The operations of `style` on [`line`] at `factor`.
fn strokes_at(style: &Style, factor: f64) -> Vec<DrawOp> {
    let mut ops = Vec::new();
    let mut canvas = Canvas::new(&[], 100, 100, Factors::new(factor, factor));
    line_symbol(&mut ops, line(), style, Pass::Plain, &mut canvas);
    ops
}

fn stroke(op: &DrawOp) -> (f64, &Dashes, &[Vec<Point>]) {
    match op {
        DrawOp::Stroke {
            width,
            dashes,
            paths,
            ..
        } => (*width, dashes, paths),
        other => panic!("{other:?}"),
    }
}

#[test]
fn at_a_factor_a_line_is_scaled_and_bounded_by_the_resolution_factor() {
    // msDrawLineSymbol, maprendering.c:622-653, at a scale and a resolution factor of 2:
    // the width times the scale factor, MAXWIDTH and MINWIDTH times the resolution factor,
    // and the offset, the pattern, and the initial gap by the final width over the WIDTH.
    let bounded = Style {
        width: Some(Bound::Value(10.0)),
        max_width: Some(5.0),
        pattern: vec![20.0, 8.0],
        initial_gap: Some(6.0),
        line_offset: Some(LineOffset {
            distance: Bound::Value(4.0),
            both_sides: false,
        }),
        ..colored(Some(BLUE), None)
    };
    let ops = strokes_at(&bounded, 2.0);
    let (width, dashes, paths) = stroke(&ops[0]);
    assert_eq!(width, 10.0, "MAXWIDTH 5 is 10");
    assert_eq!(dashes.pattern, vec![20.0, 8.0]);
    assert_eq!(dashes.offset, 6.0);
    assert_eq!(paths[0], vec![p(0.0, 4.0), p(10.0, 4.0)]);
    let plain = Style {
        width: Some(Bound::Value(3.0)),
        pattern: vec![3.0, 1.0],
        ..colored(Some(BLUE), None)
    };
    let ops = strokes_at(&plain, 2.0);
    assert_eq!(stroke(&ops[0]).0, 6.0);
    assert_eq!(stroke(&ops[0]).1.pattern, vec![6.0, 2.0]);
    let wide = Style {
        width: Some(Bound::Value(40.0)),
        ..colored(Some(BLUE), None)
    };
    assert_eq!(
        stroke(&strokes_at(&wide, 2.0)[0]).0,
        64.0,
        "the default MAXWIDTH 32 is 64"
    );
    let raised = Style {
        width: Some(Bound::Value(1.0)),
        min_width: Some(4.0),
        ..colored(Some(BLUE), None)
    };
    assert_eq!(stroke(&strokes_at(&raised, 2.0)[0]).0, 8.0);
}

#[test]
fn at_a_factor_symbols_along_a_line_are_those_of_the_scaled_style() {
    // msDrawLineSymbol hands computeSymbolStyle the scale factor, and the initial gap is
    // scaled by the final width over the WIDTH: at 2, SIZE 6, GAP 20, and INITIALGAP 5 draw
    // what SIZE 12, GAP 40, and INITIALGAP 10 draw at 1.
    let symbols = [circle()];
    let gapped = |size: f64, gap: f64, initial_gap: f64| Style {
        symbol: Some(Bound::Value(SymbolRef::Index(1))),
        size: Some(Bound::Value(size)),
        gap: Some(gap),
        initial_gap: Some(initial_gap),
        ..colored(Some(BLUE), None)
    };
    let along = |style: &Style, factor: f64| {
        let mut ops = Vec::new();
        let mut canvas = Canvas::new(&symbols, 101, 101, Factors::new(factor, factor));
        let long = vec![vec![p(0.0, 50.0), p(100.0, 50.0)]];
        line_symbol(&mut ops, long, style, Pass::Plain, &mut canvas);
        ops
    };
    let scaled = along(&gapped(6.0, 20.0, 5.0), 2.0);
    assert_eq!(scaled.len(), 3);
    assert_eq!(scaled, along(&gapped(12.0, 40.0, 10.0), 1.0));
}

/// A style with `MINWIDTH` and `MAXWIDTH` whose `WIDTH` is bound to a feature's property that
/// reads as no number, `nan`, which atof() reads as NaN.
fn bounded_nan(min_width: f64, max_width: f64, outline: Option<Rgba>) -> Style {
    Style {
        width: Some(Bound::Value(forager_style_ir::c::atof(b"nan"))),
        min_width: Some(min_width),
        max_width: Some(max_width),
        ..colored(Some(BLUE), outline)
    }
}

#[test]
fn a_line_bounds_its_width_as_ms_draw_line_symbol_does() {
    // msDrawLineSymbol, maprendering.c:622-624: MS_MIN with MAXWIDTH times the resolution
    // factor, then MS_MAX with MINWIDTH times it. A NaN width becomes MAXWIDTH, and a MINWIDTH
    // above the MAXWIDTH wins.
    let style = bounded_nan(2.0, 5.0, None);
    assert_eq!(
        stroke(&strokes_at(&style, 2.0)[0]).0,
        10.0,
        "MAXWIDTH 5 is 10"
    );
    let crossed = Style {
        width: Some(Bound::Value(4.0)),
        ..bounded_nan(6.0, 3.0, None)
    };
    assert_eq!(
        stroke(&strokes_at(&crossed, 1.0)[0]).0,
        6.0,
        "MINWIDTH last"
    );
}

/// A pixmap symbol of `path`.
fn pixmap_symbol(path: Option<&str>) -> Symbol {
    Symbol {
        name: Some("pin".into()),
        kind: SymbolKind::Pixmap,
        paths: Vec::new(),
        extent: None,
        filled: false,
        anchor_point: None,
        character: None,
        font: None,
        image: path.map(str::to_owned),
        transparent: None,
        span: None,
    }
}

/// The top-left pixels of the 4 × 3 image of symbol 1 that [`line_symbol`] blends along
/// (0, 50), (30, 50) in `style`'s `pass` on `canvas`, a panic on any other operation.
fn pixmaps_along(style: &Style, pass: Pass, canvas: &mut Canvas) -> Vec<(i32, i32)> {
    let mut ops = Vec::new();
    let line = vec![vec![p(0.0, 50.0), p(30.0, 50.0)]];
    line_symbol(&mut ops, line, style, pass, canvas);
    ops.iter()
        .map(|op| match op {
            DrawOp::Pixmap { left, top, .. } => (*left, *top),
            other => panic!("{other:?}"),
        })
        .collect()
}

/// The 4 × 3 image `pin.png`, of symbol 1 of [`pixmap_symbol`].
fn pin_pixmaps() -> Pixmaps {
    let image = Rc::new(PremultipliedImage::new(4, 3, vec![9; 48]).unwrap());
    [("pin.png".to_owned(), Ok(image))].into()
}

/// A style of symbol 1 with `gap`, an `OUTLINECOLOR`, and no `COLOR`.
fn gapped_pin(gap: f64) -> Style {
    Style {
        symbol: Some(Bound::Value(SymbolRef::Index(1))),
        gap: Some(gap),
        outline_color: Some(Bound::Value(BLACK)),
        ..Style::default()
    }
}

#[test]
fn a_pixmap_s_gap_along_a_line_is_scaled_as_its_size_is() {
    // computeSymbolStyle, src/maprendering.c:62-66: s->gap = src->gap * target_size /
    // style_size. SIZE 6 bounded by MAXSIZE 3 at a factor of 1, and SIZE 1.5 at a scale and
    // resolution factor of 2, are each a target of 3, a scale of 3 / 3 = 1 for the 4 × 3
    // image: GAP 20 × 3 / 6 and GAP 5 × 3 / 1.5 are a spacing of 10. Without an INITIALGAP the
    // markers stand at 10 / 2 = 5, 15, 25 (420, 443-445, 512), each blended at MS_NINT(x - 2),
    // MS_NINT(50 - 1.5) = 48 (src/mapagg.cpp:699-700): the gap unscaled would put them at
    // 10, 30, and at 2.5, 7.5, ....
    let pixmaps = pin_pixmaps();
    let symbols = [pixmap_symbol(Some("pin.png"))];
    let along = vec![(3, 48), (13, 48), (23, 48)];
    let bounded = Style {
        size: Some(Bound::Value(6.0)),
        max_size: Some(3.0),
        ..gapped_pin(20.0)
    };
    let mut canvas = Canvas::new(&symbols, 101, 101, Factors::new(1.0, 1.0)).with_pixmaps(&pixmaps);
    assert_eq!(pixmaps_along(&bounded, Pass::Plain, &mut canvas), along);
    let scaled = Style {
        size: Some(Bound::Value(1.5)),
        ..gapped_pin(5.0)
    };
    let mut canvas = Canvas::new(&symbols, 101, 101, Factors::new(2.0, 2.0)).with_pixmaps(&pixmaps);
    assert_eq!(pixmaps_along(&scaled, Pass::Plain, &mut canvas), along);
}

#[test]
fn a_pixmap_along_a_line_costs_one_point_of_the_layer_s_budget() {
    // Verifies: Q-9
    // Three markers every 10 pixels along (0, 50), (30, 50): a budget of 2 points draws none
    // of the line's, nor anything after it; one of 3 draws all three.
    let pixmaps = pin_pixmaps();
    let symbols = [pixmap_symbol(Some("pin.png"))];
    let style = gapped_pin(10.0);
    let mut canvas = Canvas::new(&symbols, 101, 101, Factors::new(1.0, 1.0)).with_pixmaps(&pixmaps);
    canvas.markers = MarkerBudget::of(2);
    assert!(pixmaps_along(&style, Pass::Plain, &mut canvas).is_empty());
    assert!(canvas.markers_spent());
    let mut canvas = Canvas::new(&symbols, 101, 101, Factors::new(1.0, 1.0)).with_pixmaps(&pixmaps);
    canvas.markers = MarkerBudget::of(3);
    assert_eq!(pixmaps_along(&style, Pass::Plain, &mut canvas).len(), 3);
    assert!(!canvas.markers_spent());
}

#[test]
fn a_later_cached_pixmap_brush_without_a_colour_brushes_the_line() {
    // Verifies: ADR-0046
    // The cache's later pass draws a pixmap without a COLOR (src/mapdraw.c:1487-1494), and
    // msDrawLineSymbol brushes one with a GAP of 0 along the line with its tile
    // (src/maprendering.c:687-707), not a line in the OUTLINECOLOR.
    let pixmaps = pin_pixmaps();
    let symbols = [pixmap_symbol(Some("pin.png"))];
    let mut tiles = Tiles::default();
    let mut canvas = Canvas::new(&symbols, 101, 101, Factors::new(1.0, 1.0))
        .with_pixmaps(&pixmaps)
        .with_tiles(&mut tiles);
    let mut ops = Vec::new();
    let line = vec![vec![p(0.0, 50.0), p(30.0, 50.0)]];
    let brush = gapped_pin(0.0);
    line_symbol(
        &mut ops,
        line,
        &brush,
        Pass::PlainIfColoredOrPixmap,
        &mut canvas,
    );
    assert!(matches!(&ops[..], [DrawOp::BrushedLine { .. }]), "{ops:?}");
}

#[test]
fn a_pixmap_at_a_scale_of_0_draws_nothing_along_a_line_and_spends_no_budget() {
    // At a scale factor of 0 the target size is 0, the scale 0, and the gap 10 × 0 / 3 = 0
    // (src/maprendering.c:62-66), on which msImagePolylineMarkers' loop never ends (443-513).
    // Forager draws nothing, as at a point, and leaves the layer's budget whole.
    let pixmaps = pin_pixmaps();
    let symbols = [pixmap_symbol(Some("pin.png"))];
    let mut canvas = Canvas::new(&symbols, 101, 101, Factors::new(0.0, 0.0)).with_pixmaps(&pixmaps);
    assert!(pixmaps_along(&gapped_pin(10.0), Pass::Plain, &mut canvas).is_empty());
    assert!(!canvas.markers_spent());
}

/// A vector symbol of the extent 3 × 2.
fn arrow() -> Symbol {
    Symbol {
        name: Some("arrow".into()),
        kind: SymbolKind::Vector,
        paths: vec![vec![[0.0, 1.0], [3.0, 1.0]]],
        extent: Some([3.0, 2.0]),
        ..circle()
    }
}

/// A TrueType symbol, its glyph the box of [`Boxes`].
fn truetype() -> Symbol {
    Symbol {
        name: Some("ttf".into()),
        kind: SymbolKind::Truetype,
        paths: Vec::new(),
        extent: None,
        character: Some("A".to_owned()),
        font: Some("sans".to_owned()),
        ..circle()
    }
}

/// A port whose every glyph is a box from (1, -18) to (11, 2) y down.
struct Boxes;

impl crate::glyph::Glyphs for Boxes {
    fn glyph(&self, _: &str, _: char, _: u32) -> Result<Rc<crate::glyph::Glyph>, String> {
        Ok(Rc::new(crate::glyph::Glyph {
            path: vec![forager_render::PathCommand::MoveTo(p(0.0, 0.0))],
            min_x: 1.0,
            min_y: -18.0,
            max_x: 11.0,
            max_y: 2.0,
            missing: false,
        }))
    }
}

/// The symbols of the brush tests: 1 an arrow, 2 an ellipse, 3 a pixmap of the 4 × 3 `pin.png`,
/// 4 a TrueType symbol, 5 a hatch, 6 an SVG symbol, 7 a simple symbol.
fn brushes() -> Vec<Symbol> {
    let kind = |kind| Symbol { kind, ..circle() };
    vec![
        arrow(),
        circle(),
        pixmap_symbol(Some("pin.png")),
        truetype(),
        kind(SymbolKind::Hatch),
        kind(SymbolKind::Svg),
        kind(SymbolKind::Simple),
    ]
}

/// A style of symbol `index`, without a `GAP`, at `SIZE` `size`, in blue.
fn brush(index: u32, size: f64) -> Style {
    Style {
        symbol: Some(Bound::Value(SymbolRef::Index(index))),
        size: Some(Bound::Value(size)),
        ..colored(Some(BLUE), None)
    }
}

/// The operations of `style` on (0, 50), (30, 50) on a canvas of the brush tests' symbols with
/// `tiles`, and what it left undrawn.
fn brushed(style: &Style, tiles: &mut Tiles) -> (Vec<DrawOp>, crate::canvas::Refusals) {
    let symbols = brushes();
    let pixmaps = pin_pixmaps();
    let mut canvas = Canvas::new(&symbols, 101, 101, Factors::new(1.0, 1.0))
        .with_pixmaps(&pixmaps)
        .with_tiles(tiles)
        .with_glyphs(&Boxes);
    canvas.class(std::slice::from_ref(style));
    let mut ops = Vec::new();
    let line = vec![vec![p(0.0, 50.0), p(30.0, 50.0)]];
    line_symbol(&mut ops, line, style, Pass::Plain, &mut canvas);
    (ops, canvas.take_refusals())
}

/// The width and height of the tile of the one brushed line among `ops`.
fn brush_tile(ops: &[DrawOp]) -> (u32, u32) {
    match ops {
        [DrawOp::BrushedLine { tile, .. }] => (tile.width(), tile.height()),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_vector_ellipse_pixmap_or_truetype_symbol_without_a_gap_brushes_the_line() {
    // Verifies: ADR-0046
    for index in 1..=4 {
        let style = Style {
            offset: Some([Bound::Value(0.0), Bound::Value(2.0)]),
            ..brush(index, 6.0)
        };
        let (ops, refused) = brushed(&style, &mut Tiles::default());
        assert_eq!(
            refused,
            crate::canvas::Refusals::default(),
            "symbol {index}"
        );
        match &ops[..] {
            [DrawOp::BrushedLine { paths, reach, .. }] => {
                // Moved by the offset, and clipped by the renderer at the class's clip_buf: the
                // SIZE 6 plus 1.
                assert_eq!(
                    paths,
                    &[vec![p(0.0, 52.0), p(30.0, 52.0)]],
                    "symbol {index}"
                );
                assert_eq!(*reach, 7.0, "symbol {index}");
            }
            other => panic!("symbol {index}: {other:?}"),
        }
    }
    // A hatch, an SVG symbol, a simple symbol, and symbol 0 keep their line in the COLOR.
    for index in [5, 6, 7, 0] {
        let (ops, _) = brushed(&brush(index, 6.0), &mut Tiles::default());
        assert!(
            matches!(&ops[..], [DrawOp::Stroke { .. }]),
            "{index}: {ops:?}"
        );
    }
    // A GAP places the symbol instead.
    let gapped = Style {
        gap: Some(-20.0),
        ..brush(1, 6.0)
    };
    let (ops, _) = brushed(&gapped, &mut Tiles::default());
    assert!(
        !ops.iter()
            .any(|op| matches!(op, DrawOp::BrushedLine { .. }))
    );
}

#[test]
fn a_brush_tile_s_sides_are_ms_draw_line_symbol_s() {
    // Verifies: ADR-0046
    let size = |index, size| brush_tile(&brushed(&brush(index, size), &mut Tiles::default()).0);
    // The arrow's 3 × 2 at SIZE 10, a scale of 10 / 2: MS_NINT(15) by MS_NINT(10).
    assert_eq!(size(1, 10.0), (15, 10));
    // At SIZE 5, a scale of 2.5: MS_NINT(7.5) and MS_NINT(5), half to even 8 by 5.
    assert_eq!(size(1, 5.0), (8, 5));
    // The pixmap's 4 × 3 at SIZE 6, a scale of 2: 8 by 6; the TrueType symbol's 1 × 1 at SIZE
    // 9.5, MS_NINT(9.5) a side, 10.
    assert_eq!(size(3, 6.0), (8, 6));
    assert_eq!(size(4, 9.5), (10, 10));
    // At a scale of 1 the extent truncated: 3.7 × 2 at SIZE 2 is 3 by 2.
    let symbols_at_1 = Symbol {
        extent: Some([3.7, 2.0]),
        ..arrow()
    };
    let symbols = [symbols_at_1];
    let mut tiles = Tiles::default();
    let mut canvas = Canvas::new(&symbols, 101, 101, Factors::new(1.0, 1.0)).with_tiles(&mut tiles);
    let mut ops = Vec::new();
    line_symbol(&mut ops, line(), &brush(1, 2.0), Pass::Plain, &mut canvas);
    assert_eq!(brush_tile(&ops), (3, 2));
}

#[test]
fn a_polygon_s_cached_seamless_tile_serves_a_line_of_the_same_key() {
    // Verifies: ADR-0046
    // searchTileCache compares no seamless mode: the arrow's seamless fill tile at SIZE 10,
    // 15 × 10, serves the line's brush of the same symbol, size, scale, rotation, and colours.
    let symbols = brushes();
    let mut tiles = Tiles::default();
    let mut canvas = Canvas::new(&symbols, 101, 101, Factors::new(1.0, 1.0)).with_tiles(&mut tiles);
    let view = unit_view();
    let style = brush(1, 10.0);
    let mut ops = Vec::new();
    let ring = vec![vec![p(10.0, 10.0), p(90.0, 10.0), p(90.0, 90.0)]];
    assert!(crate::shade::shade(
        &mut ops,
        ring,
        &style,
        &view,
        &mut canvas
    ));
    line_symbol(&mut ops, line(), &style, Pass::Plain, &mut canvas);
    match &ops[..] {
        [
            DrawOp::TiledFill { tile: fill, .. },
            DrawOp::BrushedLine { tile: line, .. },
        ] => {
            assert!(Rc::ptr_eq(fill, line));
            // Nine arrows, of getTile's seamless mode.
            assert_eq!(line.marks().len(), 9);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_brush_tile_past_4096_pixels_a_side_draws_nothing_and_says_why() {
    // Verifies: ADR-0046, Q-9
    // MAXSIZE lets the SIZE past MapServer's default bound of 500.
    let huge = |index, size| Style {
        max_size: Some(1e6),
        ..brush(index, size)
    };
    for index in [1, 2, 4] {
        let (ops, refused) = brushed(&huge(index, 8194.0), &mut Tiles::default());
        assert!(ops.is_empty(), "{index}: {ops:?}");
        let why = &refused.symbols[&(index as usize)].1;
        assert!(
            why.contains("past the limit of 4096 × 4096"),
            "{index}: {why}"
        );
    }
    // The pixmap's 4 × 3 at SIZE 6000: 8000 × 6000.
    let (ops, refused) = brushed(&huge(3, 6000.0), &mut Tiles::default());
    assert!(ops.is_empty(), "{ops:?}");
    assert!(refused.pixmaps[&3].1.contains("8000 × 6000"), "{refused:?}");
}

#[test]
fn a_brush_tile_whose_copy_would_pass_256_mib_draws_nothing_and_says_why() {
    // Verifies: ADR-0046, Q-9
    // The arrow at SIZE 200: a tile of 300 × 200, 240,000 bytes, and the brush's copy of
    // 304 × 204 × 4 = 248,064 more. With 300,000 bytes left the polygon's fill tile fits, and
    // the line's brush of the same tile does not, built new or found held.
    let left = 300_000;
    for held in [false, true] {
        let mut tiles = Tiles::default();
        tiles.count_images(crate::tile::MAX_DECODED_IMAGE_BYTES - left);
        if held {
            let symbols = brushes();
            let mut canvas =
                Canvas::new(&symbols, 101, 101, Factors::new(1.0, 1.0)).with_tiles(&mut tiles);
            let mut ops = Vec::new();
            let view = unit_view();
            let ring = vec![vec![p(10.0, 10.0), p(90.0, 10.0), p(90.0, 90.0)]];
            let fill = Style {
                gap: Some(0.0),
                ..brush(1, 200.0)
            };
            crate::shade::shade(&mut ops, ring, &fill, &view, &mut canvas);
            assert!(matches!(&ops[..], [DrawOp::TiledFill { .. }]), "{ops:?}");
        }
        let (ops, refused) = brushed(&brush(1, 200.0), &mut tiles);
        assert!(ops.is_empty(), "{ops:?}");
        let why = &refused.symbols[&1].1;
        assert!(why.contains("300 × 200 pixels and the copy"), "{why}");
    }
    // A pixmap's says so of the pixmap: the 4 × 3 at SIZE 300, 400 × 300, 480,000 bytes, with
    // 500,000 left and its copy of 404 × 304 × 4.
    let mut tiles = Tiles::default();
    tiles.count_images(crate::tile::MAX_DECODED_IMAGE_BYTES - 500_000);
    let (ops, refused) = brushed(&brush(3, 300.0), &mut tiles);
    assert!(ops.is_empty(), "{ops:?}");
    assert!(
        refused.pixmaps[&3].1.contains("and the copy"),
        "{refused:?}"
    );
}

#[test]
fn a_brush_whose_image_or_glyph_is_not_found_draws_nothing_and_says_why() {
    // Verifies: ADR-0046
    let symbols = [pixmap_symbol(Some("gone.png")), truetype()];
    let mut tiles = Tiles::default();
    let mut canvas = Canvas::new(&symbols, 101, 101, Factors::new(1.0, 1.0)).with_tiles(&mut tiles);
    let mut ops = Vec::new();
    line_symbol(&mut ops, line(), &brush(1, 6.0), Pass::Plain, &mut canvas);
    line_symbol(&mut ops, line(), &brush(2, 6.0), Pass::Plain, &mut canvas);
    assert!(ops.is_empty(), "{ops:?}");
    let refused = canvas.take_refusals();
    assert!(refused.pixmaps[&1].1.contains("gone.png"), "{refused:?}");
    assert!(refused.symbols[&2].1.contains("no fonts"), "{refused:?}");
}

/// The view of 101 × 101 pixels over 0..100.
fn unit_view() -> crate::extent::View {
    crate::extent::View::for_image(
        forager_style_ir::Extent {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 100.0,
            max_y: 100.0,
        },
        101,
        101,
    )
    .unwrap()
}

#[test]
fn a_polygon_s_vector_symbol_without_a_color_brushes_its_outline() {
    // Verifies: ADR-0046
    // msDrawShadeSymbol hands a polygon of a symbol other than a pixmap, without a COLOR and with
    // an OUTLINECOLOR, to msDrawLineSymbol in the same style, which brushes it with the tile
    // without a GAP; the ring is a line that ends on its first point.
    let symbols = brushes();
    let mut tiles = Tiles::default();
    let mut canvas = Canvas::new(&symbols, 101, 101, Factors::new(1.0, 1.0)).with_tiles(&mut tiles);
    let style = Style {
        symbol: Some(Bound::Value(SymbolRef::Index(1))),
        size: Some(Bound::Value(10.0)),
        ..colored(None, Some(BLACK))
    };
    let mut ops = Vec::new();
    let ring = vec![vec![p(10.0, 10.0), p(90.0, 10.0), p(90.0, 90.0)]];
    assert!(crate::shade::shade(
        &mut ops,
        ring,
        &style,
        &unit_view(),
        &mut canvas
    ));
    match &ops[..] {
        [DrawOp::BrushedLine { paths, tile, .. }] => {
            assert_eq!(paths[0].len(), 4);
            assert_eq!((tile.width(), tile.height()), (15, 10));
        }
        other => panic!("{other:?}"),
    }
}
