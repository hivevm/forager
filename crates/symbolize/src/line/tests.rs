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

use forager_render::{Dashes, DrawOp, LineCap, LineJoin, Point, PremultipliedImage};
use forager_style_ir::{Bound, LineOffset, Rgba, Style, Symbol, SymbolKind, SymbolRef};

use super::*;
use crate::canvas::Canvas;
use crate::line::Pass;
use crate::line_marker::MarkerBudget;
use crate::style::fixed;
use crate::symbol::Pixmaps;

const BLUE: Rgba = Rgba::opaque(30, 60, 200);
const BLACK: Rgba = Rgba::opaque(0, 0, 0);

fn p(x: f64, y: f64) -> Point {
    Point { x, y }
}

/// A canvas without symbols, of 100 × 100.
fn plain() -> Canvas<'static> {
    Canvas::new(&[], 100, 100, 1.0)
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
            | DrawOp::TiledFill { .. } => {
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
    let prepared = outline_style(&style, 1.0, 1.0);
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
        fixed(&outline_style(&without_size, 1.0, 1.0).size),
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
    let prepared = outline_style(&style, 1.0, 1.0);
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
        1.0,
        1.0,
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
fn a_line_no_wider_than_a_pixel_has_butt_caps_and_bevel_joins() {
    // agg2RenderLine applies the style's cap and join only to a line wider than a pixel.
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
            DrawOp::Stroke { cap, join, .. } => (*cap, *join),
            DrawOp::Fill { .. }
            | DrawOp::Hatch(_)
            | DrawOp::Glyphs { .. }
            | DrawOp::Pixmap { .. }
            | DrawOp::ResampledPixmap { .. }
            | DrawOp::TiledFill { .. } => {
                panic!("a line is not filled")
            }
        }
    };
    assert_eq!(ends(1.0), (LineCap::Butt, LineJoin::Bevel));
    assert_eq!(ends(1.5), (LineCap::Square, LineJoin::Miter));
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
    let mut canvas = Canvas::new(&[], 100, 100, factor);
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
        let mut canvas = Canvas::new(&symbols, 101, 101, factor);
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
    let mut canvas = Canvas::new(&symbols, 101, 101, 1.0).with_pixmaps(&pixmaps);
    assert_eq!(pixmaps_along(&bounded, Pass::Plain, &mut canvas), along);
    let scaled = Style {
        size: Some(Bound::Value(1.5)),
        ..gapped_pin(5.0)
    };
    let mut canvas = Canvas::new(&symbols, 101, 101, 2.0).with_pixmaps(&pixmaps);
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
    let mut canvas = Canvas::new(&symbols, 101, 101, 1.0).with_pixmaps(&pixmaps);
    canvas.markers = MarkerBudget::of(2);
    assert!(pixmaps_along(&style, Pass::Plain, &mut canvas).is_empty());
    assert!(canvas.markers_spent());
    let mut canvas = Canvas::new(&symbols, 101, 101, 1.0).with_pixmaps(&pixmaps);
    canvas.markers = MarkerBudget::of(3);
    assert_eq!(pixmaps_along(&style, Pass::Plain, &mut canvas).len(), 3);
    assert!(!canvas.markers_spent());
}

#[test]
fn a_later_cached_pixmap_brush_without_a_colour_draws_nothing() {
    // The cache's later pass draws a pixmap without a COLOR (src/mapdraw.c:1487-1494), and
    // msDrawLineSymbol tiles one with a GAP of 0 along the line (src/maprendering.c:687-707),
    // which Forager does not: the pass skips it, and strokes no line in the OUTLINECOLOR.
    let pixmaps = pin_pixmaps();
    let symbols = [pixmap_symbol(Some("pin.png"))];
    let mut canvas = Canvas::new(&symbols, 101, 101, 1.0).with_pixmaps(&pixmaps);
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
    assert!(ops.is_empty(), "{ops:?}");
}

#[test]
fn a_pixmap_at_a_scale_of_0_draws_nothing_along_a_line_and_spends_no_budget() {
    // At a scale factor of 0 the target size is 0, the scale 0, and the gap 10 × 0 / 3 = 0
    // (src/maprendering.c:62-66), on which msImagePolylineMarkers' loop never ends (443-513).
    // Forager draws nothing, as at a point, and leaves the layer's budget whole.
    let pixmaps = pin_pixmaps();
    let symbols = [pixmap_symbol(Some("pin.png"))];
    let mut canvas = Canvas::new(&symbols, 101, 101, 0.0).with_pixmaps(&pixmaps);
    assert!(pixmaps_along(&gapped_pin(10.0), Pass::Plain, &mut canvas).is_empty());
    assert!(!canvas.markers_spent());
}
