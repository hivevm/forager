// The tests of a symbol's marker size and anchor point work their values out from MapServer's
// msGetMarkerSize, src/mapsymbol.c, and msAdjustMarkerPos, src/maprendering.c, of the release
// pinned in scripts/fetch-msautotest.sh.
// Copyright (c) 1996-2005 Regents of the University of Minnesota.
// Copyright (c) 1996-2011 Regents of the University of Minnesota (src/maprendering.c).
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

//! The geometry of symbols at a point, each branch of the MapServer functions it follows checked
//! against values worked out by hand from their code.

use super::*;
use std::rc::Rc;

use forager_render::PremultipliedImage;

const RED: Rgba = Rgba::opaque(255, 0, 0);
const BLACK: Rgba = Rgba::opaque(0, 0, 0);

fn symbol(kind: SymbolKind, paths: Vec<Vec<[f64; 2]>>, extent: [f64; 2], filled: bool) -> Symbol {
    Symbol {
        name: None,
        kind,
        paths,
        extent: Some(extent),
        filled,
        anchor_point: None,
        character: None,
        font: None,
        image: None,
        transparent: None,
        span: None,
    }
}

/// A triangle 2 wide and 2 high, its apex at the bottom (y downwards).
fn triangle(filled: bool) -> Symbol {
    symbol(
        SymbolKind::Vector,
        vec![vec![[0.0, 0.0], [2.0, 0.0], [1.0, 2.0]]],
        [2.0, 2.0],
        filled,
    )
}

fn circle(filled: bool) -> Symbol {
    symbol(
        SymbolKind::Ellipse,
        vec![vec![[1.0, 1.0]]],
        [1.0, 1.0],
        filled,
    )
}

fn style(size: Option<f64>, angle: Option<f64>) -> Style {
    Style {
        color: Some(Bound::Value(RED)),
        size: size.map(Bound::Value),
        angle: angle.map(Bound::Value),
        ..Style::default()
    }
}

const AT: Point = Point { x: 50.0, y: 50.0 };

fn close(actual: &[Point], expected: &[(f64, f64)]) {
    assert_eq!(actual.len(), expected.len(), "{actual:?}");
    for (a, &(x, y)) in actual.iter().zip(expected) {
        assert!(
            (a.x - x).abs() < 1e-9 && (a.y - y).abs() < 1e-9,
            "{actual:?} is not {expected:?}"
        );
    }
}

fn ops(style: &Style, symbol: &Symbol) -> Vec<DrawOp> {
    let mut ops = Vec::new();
    symbol_ops(&mut ops, AT, style, symbol, 1.0);
    ops
}

#[test]
fn a_style_marks_by_its_symbol() {
    let symbols = [triangle(true), circle(true), {
        let mut pixmap = triangle(false);
        pixmap.kind = SymbolKind::Pixmap;
        pixmap
    }];
    let by = |symbol: Option<Bound<SymbolRef>>| Style {
        symbol,
        ..Style::default()
    };
    let index = |i| by(Some(Bound::Value(SymbolRef::Index(i))));
    assert_eq!(mark(&by(None), &symbols), Mark::Square);
    assert_eq!(mark(&index(0), &symbols), Mark::Square);
    assert_eq!(mark(&index(1), &symbols), Mark::Symbol(&symbols[0]));
    assert_eq!(mark(&index(2), &symbols), Mark::Symbol(&symbols[1]));
    assert_eq!(mark(&index(3), &symbols), Mark::Pixmap(3, &symbols[2]));
    assert_eq!(mark(&index(4), &symbols), Mark::Nothing, "past the last");
    let name = by(Some(Bound::Value(SymbolRef::Name("x.png".into()))));
    assert_eq!(mark(&name, &symbols), Mark::Square);
}

#[test]
fn a_filled_symbol_fills_with_the_colour_and_outlines_with_the_outline_colour() {
    // computeSymbolStyle, the branch of a filled symbol.
    let mut outlined = style(Some(10.0), None);
    outlined.outline_color = Some(Bound::Value(BLACK));
    outlined.width = Some(Bound::Value(3.0));
    let s = symbol_style(&outlined, &triangle(true), 1.0);
    assert_eq!(
        (s.fill, s.outline, s.outline_width),
        (Some(RED), Some(BLACK), 3.0)
    );
    let s = symbol_style(&style(Some(10.0), None), &triangle(true), 1.0);
    assert_eq!((s.fill, s.outline, s.outline_width), (Some(RED), None, 0.0));
}

#[test]
fn a_symbol_not_filled_is_outlined_with_the_colour_or_else_the_outline_colour() {
    // computeSymbolStyle, the branch of a symbol not filled: the outline is the style's width,
    // 1 by default.
    let s = symbol_style(&style(Some(10.0), None), &triangle(false), 1.0);
    assert_eq!((s.fill, s.outline, s.outline_width), (None, Some(RED), 1.0));
    let only_outline = Style {
        outline_color: Some(Bound::Value(BLACK)),
        width: Some(Bound::Value(100.0)),
        ..Style::default()
    };
    let s = symbol_style(&only_outline, &triangle(false), 1.0);
    assert_eq!(
        (s.fill, s.outline, s.outline_width),
        (None, Some(BLACK), 32.0)
    );
    let neither = Style::default();
    assert!(ops(&neither, &triangle(false)).is_empty());
}

#[test]
fn the_size_is_the_symbols_height_and_bounded() {
    // msSymbolGetDefaultSize: the height, the width where there is no height, else 1.
    let wide = symbol(
        SymbolKind::Vector,
        vec![vec![[0.0, 0.0], [4.0, 0.0]]],
        [4.0, 0.0],
        false,
    );
    assert_eq!(default_size(&wide), 4.0);
    assert_eq!(default_size(&triangle(false)), 2.0);
    let point = symbol(
        SymbolKind::Vector,
        vec![vec![[0.0, 0.0]]],
        [0.0, 0.0],
        false,
    );
    assert_eq!(default_size(&point), 1.0);
    // Without SIZE, the symbol's own size: a scale of 1.
    assert_eq!(
        symbol_style(&style(None, None), &triangle(true), 1.0).scale,
        1.0
    );
    assert_eq!(
        symbol_style(&style(Some(10.0), None), &triangle(true), 1.0).scale,
        5.0
    );
    let mut bounded = style(Some(10.0), None);
    bounded.max_size = Some(4.0);
    assert_eq!(symbol_style(&bounded, &triangle(true), 1.0).scale, 2.0);
    bounded.min_size = Some(8.0);
    assert_eq!(
        symbol_style(&bounded, &triangle(true), 1.0).scale,
        2.0,
        "MAXSIZE applies last"
    );
    assert_eq!(
        symbol_style(&style(None, Some(90.0)), &triangle(true), 1.0).rotation,
        90.0 * DEG_TO_RAD
    );
}

#[test]
fn minsize_raises_the_size_and_minwidth_the_outline() {
    // computeSymbolStyle: MS_MAX with the lower bound before MS_MIN with the upper.
    let mut small = style(Some(2.0), None);
    small.min_size = Some(6.0);
    small.width = Some(Bound::Value(0.5));
    small.min_width = Some(2.0);
    let s = symbol_style(&small, &triangle(false), 1.0);
    assert_eq!((s.scale, s.outline_width), (3.0, 2.0));
}

#[test]
fn a_vector_symbol_is_centred_scaled_and_turned_counter_clockwise() {
    // agg2RenderVectorSymbol: translate by half the extent, scale by 10 / 2, turn by -90° in
    // AGG's y-down sense, move to the point. The apex, below the centre, turns to its right.
    let ops = ops(&style(Some(10.0), Some(90.0)), &triangle(true));
    let [
        DrawOp::Fill {
            rings,
            color,
            gamma: None,
        },
    ] = ops.as_slice()
    else {
        panic!("{ops:?}");
    };
    assert_eq!(*color, RED);
    close(&rings[0], &[(45.0, 55.0), (45.0, 45.0), (55.0, 50.0)]);
}

#[test]
fn a_vector_symbol_not_filled_is_an_open_outline() {
    let ops = ops(&style(Some(10.0), None), &triangle(false));
    let [
        DrawOp::Stroke {
            paths,
            closed,
            color,
            width,
            cap,
            join,
            ..
        },
    ] = ops.as_slice()
    else {
        panic!("{ops:?}");
    };
    assert_eq!((*closed, *color, *width), (false, RED, 1.0));
    // AGG's defaults, which agg2RenderVectorSymbol leaves as they are.
    assert_eq!((*cap, *join), (LineCap::Butt, LineJoin::Miter));
    close(&paths[0], &[(45.0, 45.0), (55.0, 45.0), (50.0, 55.0)]);
}

#[test]
fn a_vector_symbols_pen_ups_start_new_paths() {
    let cross = symbol(
        SymbolKind::Vector,
        vec![vec![[0.0, 1.0], [2.0, 1.0]], vec![[1.0, 0.0], [1.0, 2.0]]],
        [2.0, 2.0],
        false,
    );
    let ops = ops(&style(Some(2.0), None), &cross);
    let [DrawOp::Stroke { paths, .. }] = ops.as_slice() else {
        panic!("{ops:?}");
    };
    close(&paths[0], &[(49.0, 50.0), (51.0, 50.0)]);
    close(&paths[1], &[(50.0, 49.0), (50.0, 51.0)]);
}

#[test]
fn the_anchor_point_lies_on_the_point_turned_with_the_symbol() {
    // msAdjustMarkerPos: the offset of the anchor from the centre, at msGetMarkerSize, turned.
    let mut anchored_triangle = triangle(true);
    anchored_triangle.anchor_point = Some([0.0, 0.0]);
    let ops = ops(&style(Some(10.0), None), &anchored_triangle);
    let [DrawOp::Fill { rings, .. }] = ops.as_slice() else {
        panic!("{ops:?}");
    };
    close(&rings[0], &[(50.0, 50.0), (60.0, 50.0), (55.0, 60.0)]);
    let ops = super::tests::ops(&style(Some(10.0), Some(90.0)), &anchored_triangle);
    let [DrawOp::Fill { rings, .. }] = ops.as_slice() else {
        panic!("{ops:?}");
    };
    close(&rings[0], &[(50.0, 50.0), (50.0, 40.0), (60.0, 45.0)]);
}

#[test]
fn the_polar_offset_then_the_offset_move_the_symbol_before_its_anchor() {
    // msAdjustMarkerPos: POLAROFFSET 10 90 moves up the image by 10 (counter-clockwise from
    // rightwards), OFFSET 2 3 by 2 right and 3 down; the anchor point then lies on that point.
    let mut moved = style(Some(10.0), None);
    moved.polar_offset = Some([Bound::Value(10.0), Bound::Value(90.0)]);
    moved.offset = Some([Bound::Value(2.0), Bound::Value(3.0)]);
    let mut anchored_triangle = triangle(true);
    anchored_triangle.anchor_point = Some([0.0, 0.0]);
    let ops = ops(&moved, &anchored_triangle);
    let [DrawOp::Fill { rings, .. }] = ops.as_slice() else {
        panic!("{ops:?}");
    };
    close(&rings[0], &[(52.0, 43.0), (62.0, 43.0), (57.0, 53.0)]);
    // A polar offset of distance 0 at an angle other than 0 moves nothing.
    let mut turned_only = style(None, None);
    turned_only.polar_offset = Some([Bound::Value(0.0), Bound::Value(30.0)]);
    assert_eq!(shifted(AT, &turned_only, 1.0), AT);
    // A line offset moves a symbol by the OFFSET it was written as: its y is −99.
    let mut line_offset = style(None, None);
    line_offset.line_offset = Some(forager_style_ir::LineOffset {
        distance: Bound::Value(4.0),
        both_sides: false,
    });
    assert_eq!(shifted(AT, &line_offset, 1.0), Point { x: 54.0, y: -49.0 });
}

#[test]
fn the_marker_size_cuts_the_size_to_whole_pixels() {
    // msGetMarkerSize: `size` is an int; with SIZE, the width follows the height; without, the
    // symbol's own extent.
    let wide = symbol(
        SymbolKind::Vector,
        vec![vec![[0.0, 0.0], [4.0, 2.0]]],
        [4.0, 2.0],
        true,
    );
    assert_eq!(
        marker_size(&style(Some(10.7), None), &wide, 1.0),
        (20.0, 10.0)
    );
    assert_eq!(marker_size(&style(None, None), &wide, 1.0), (4.0, 2.0));
    // The bounds cut again: MINSIZE 4.5 raises 3 to 4.
    let mut raised = style(Some(3.0), None);
    raised.min_size = Some(4.5);
    assert_eq!(marker_size(&raised, &wide, 1.0), (8.0, 4.0));
    // The size is scaled before it is cut: 10.7 × 1.5 is 16; without SIZE, the extent stands.
    assert_eq!(
        marker_size(&style(Some(10.7), None), &wide, 1.5),
        (32.0, 16.0)
    );
    assert_eq!(marker_size(&style(None, None), &wide, 1.5), (4.0, 2.0));
}

#[test]
fn the_marker_size_is_bounded_by_maxsize_or_its_default() {
    // msGetMarkerSize: MS_MIN with the style's MAXSIZE, MS_MAXSYMBOLSIZE (500) without one, cut
    // to whole pixels.
    let wide = symbol(
        SymbolKind::Vector,
        vec![vec![[0.0, 0.0], [4.0, 2.0]]],
        [4.0, 2.0],
        true,
    );
    let mut capped = style(Some(10.0), None);
    capped.max_size = Some(6.5);
    assert_eq!(marker_size(&capped, &wide, 1.0), (12.0, 6.0));
    assert_eq!(
        marker_size(&style(Some(800.0), None), &wide, 1.0),
        (1000.0, 500.0)
    );
    // The default MINSIZE, 0, leaves a size of 0 where it is.
    assert_eq!(marker_size(&style(Some(0.4), None), &wide, 1.0), (0.0, 0.0));
}

#[test]
fn a_label_moves_away_from_a_marker_of_the_size_ms_get_marker_size_gives() {
    let wide = symbol(
        SymbolKind::Vector,
        vec![vec![[0.0, 0.0], [4.0, 2.0]]],
        [4.0, 2.0],
        true,
    );
    let symbols = [wide];
    let mut drawn = style(Some(10.0), None);
    drawn.symbol = Some(Bound::Value(SymbolRef::Index(1)));
    assert_eq!(
        marker_size_of(&drawn, &symbols, &Pixmaps::new(), 1.0),
        Ok((20.0, 10.0))
    );
    // MapServer's default symbol, a point.
    assert_eq!(
        marker_size_of(&style(Some(10.0), None), &symbols, &Pixmaps::new(), 1.0),
        Ok((1.0, 1.0))
    );
    drawn.symbol = Some(Bound::Value(SymbolRef::Index(2)));
    assert!(
        marker_size_of(&drawn, &symbols, &Pixmaps::new(), 1.0).is_err(),
        "no such symbol"
    );
}

#[test]
fn a_pixmap_s_marker_is_sized_by_its_image_and_one_without_an_image_is_named() {
    // msGetMarkerSize, src/mapsymbol.c:776-781, for an image 40 wide and 20 high at SIZE 10:
    // 10 high and 10 / 20 × 40 = 20 wide.
    let mut pin = symbol(SymbolKind::Pixmap, vec![], [0.0, 0.0], false);
    pin.image = Some("pin.png".to_owned());
    let mut drawn = style(Some(10.0), None);
    drawn.symbol = Some(Bound::Value(SymbolRef::Index(1)));
    let image = Rc::new(PremultipliedImage::new(40, 20, vec![9; 40 * 20 * 4]).unwrap());
    let held = Pixmaps::from([("pin.png".to_owned(), Ok(image))]);
    let symbols = [pin.clone()];
    assert_eq!(
        marker_size_of(&drawn, &symbols, &held, 1.0),
        Ok((20.0, 10.0))
    );
    // msGetMarkerSize fails where the image does not load (736-738); the error names the
    // symbol, by its name where it has one, and why.
    let refused = Pixmaps::from([("pin.png".to_owned(), Err("refused".to_owned()))]);
    let error = marker_size_of(&drawn, &symbols, &refused, 1.0).unwrap_err();
    assert!(
        error.starts_with("symbol 1,") && error.ends_with("refused"),
        "{error}"
    );
    let error = marker_size_of(&drawn, &symbols, &Pixmaps::new(), 1.0).unwrap_err();
    assert!(error.contains("pin.png is not loaded"), "{error}");
    pin.name = Some("pin".to_owned());
    pin.image = None;
    let error = marker_size_of(&drawn, &[pin], &held, 1.0).unwrap_err();
    assert!(
        error.starts_with("symbol `pin`,") && error.ends_with("names no image"),
        "{error}"
    );
}

#[test]
fn an_ellipse_is_aggs_polygon_of_its_scaled_extent() {
    // agg2RenderEllipseSymbol: a circle 1 across at size 12 has a radius of 6, which AGG's
    // calc_num_steps draws with uround(2π / (2 acos(6 / 6.125))) = 16 vertices, from the angle 0.
    let ops = ops(&style(Some(12.0), None), &circle(true));
    let [
        DrawOp::Fill {
            rings,
            color,
            gamma: None,
        },
    ] = ops.as_slice()
    else {
        panic!("{ops:?}");
    };
    assert_eq!(*color, RED);
    assert_eq!(rings[0].len(), 16);
    close(&rings[0][..1], &[(56.0, 50.0)]);
    close(&rings[0][4..5], &[(50.0, 56.0)]);
}

#[test]
fn an_ellipse_turns_about_its_centre_and_its_outline_is_closed() {
    let wide = symbol(
        SymbolKind::Ellipse,
        vec![vec![[2.0, 1.0]]],
        [2.0, 1.0],
        false,
    );
    let ops = ops(&style(Some(10.0), Some(90.0)), &wide);
    let [
        DrawOp::Stroke {
            paths,
            closed,
            width,
            ..
        },
    ] = ops.as_slice()
    else {
        panic!("{ops:?}");
    };
    assert_eq!((*closed, *width), (true, 1.0));
    // The end of the long axis, 10 to the right, turns up.
    close(&paths[0][..1], &[(50.0, 40.0)]);
}

#[test]
fn an_outline_of_width_zero_is_not_drawn() {
    let mut thin = style(Some(10.0), None);
    thin.width = Some(Bound::Value(0.0));
    assert!(ops(&thin, &circle(false)).is_empty());
}

#[test]
fn a_hostile_ellipse_is_bounded_and_a_hostile_size_draws_without_a_panic() {
    // Verifies: Q-9
    let huge = symbol(
        SymbolKind::Ellipse,
        vec![vec![[1e300, 1.0]]],
        [1e300, 1.0],
        true,
    );
    let ops = ops(&style(Some(10.0), None), &huge);
    let [DrawOp::Fill { rings, .. }] = ops.as_slice() else {
        panic!("{ops:?}");
    };
    assert_eq!(rings[0].len(), MAX_ELLIPSE_STEPS as usize);
    for size in [f64::NAN, f64::INFINITY, -1.0, 0.0] {
        let _ = super::tests::ops(&style(Some(size), Some(f64::NAN)), &circle(true));
        let _ = super::tests::ops(&style(Some(size), None), &triangle(false));
    }
}

#[test]
fn at_a_factor_the_size_and_outline_are_scaled_and_their_bounds_by_the_resolution_factor() {
    // computeSymbolStyle, maprendering.c:62-74, at a scale and a resolution factor of 2: the
    // size times the scale factor, bounded by MINSIZE and MAXSIZE times the resolution factor;
    // the outline width alike; the GAP scaled as the size is.
    let triangle = triangle(false);
    assert_eq!(
        symbol_style(&style(Some(10.0), None), &triangle, 2.0).scale,
        10.0
    );
    assert_eq!(
        symbol_style(&style(None, None), &triangle, 2.0).scale,
        2.0,
        "without SIZE, the symbol's own size scaled"
    );
    let mut capped = style(Some(10.0), None);
    capped.max_size = Some(4.0);
    assert_eq!(
        symbol_style(&capped, &triangle, 2.0).scale,
        4.0,
        "MAXSIZE 4 is 8"
    );
    let mut raised = style(Some(2.0), None);
    raised.min_size = Some(6.0);
    assert_eq!(
        symbol_style(&raised, &triangle, 2.0).scale,
        6.0,
        "MINSIZE 6 is 12"
    );
    let mut wide = style(Some(10.0), None);
    wide.width = Some(Bound::Value(20.0));
    assert_eq!(
        symbol_style(&wide, &triangle, 2.0).outline_width,
        40.0,
        "the default MAXWIDTH 32 is 64"
    );
    wide.max_width = Some(15.0);
    assert_eq!(symbol_style(&wide, &triangle, 2.0).outline_width, 30.0);
    let mut thin = style(Some(10.0), None);
    thin.width = Some(Bound::Value(0.5));
    thin.min_width = Some(2.0);
    assert_eq!(symbol_style(&thin, &triangle, 2.0).outline_width, 4.0);
    let mut gapped = style(Some(10.0), None);
    gapped.gap = Some(5.0);
    assert_eq!(symbol_style(&gapped, &triangle, 2.0).gap, 10.0);
}

#[test]
fn at_a_factor_the_offsets_and_the_anchor_move_the_marker_scaled() {
    // msAdjustMarkerPos, maprendering.c:1013-1046, at a scale factor of 2: the polar offset
    // and the OFFSET times the factor, the anchor point by the size msGetMarkerSize gives at it.
    let mut moved = style(None, None);
    moved.polar_offset = Some([Bound::Value(10.0), Bound::Value(90.0)]);
    moved.offset = Some([Bound::Value(2.0), Bound::Value(3.0)]);
    close(&[shifted(AT, &moved, 2.0)], &[(54.0, 36.0)]);
    let mut anchored_triangle = triangle(true);
    anchored_triangle.anchor_point = Some([0.0, 0.0]);
    let sized = style(Some(10.0), None);
    assert_eq!(
        anchored(AT, &anchored_triangle, 0.0, || {
            marker_size(&sized, &anchored_triangle, 1.0)
        }),
        Point { x: 55.0, y: 55.0 }
    );
    assert_eq!(
        anchored(AT, &anchored_triangle, 0.0, || {
            marker_size(&sized, &anchored_triangle, 2.0)
        }),
        Point { x: 60.0, y: 60.0 },
        "a marker 20 across"
    );
}

#[test]
fn a_symbol_bounds_its_size_and_outline_as_compute_symbol_style_does() {
    // computeSymbolStyle, maprendering.c:62-64 and 69-71, at a factor of 2: MS_MAX with the
    // lower bound times the resolution factor, then MS_MIN with the upper. A SIZE and a WIDTH bound
    // to a property that reads as `nan`, which atof() reads as NaN, become MINSIZE and MINWIDTH;
    // a MAXSIZE or MAXWIDTH below the lower bound wins.
    let nan = forager_style_ir::c::atof(b"nan");
    let bounded = |size: f64, width: f64, min: f64, max: f64| Style {
        size: Some(Bound::Value(size)),
        width: Some(Bound::Value(width)),
        min_size: Some(min),
        max_size: Some(max),
        min_width: Some(min),
        max_width: Some(max),
        ..style(None, None)
    };
    let triangle = triangle(false);
    let s = symbol_style(&bounded(nan, nan, 2.0, 5.0), &triangle, 2.0);
    assert_eq!(
        (s.scale, s.outline_width),
        (2.0, 4.0),
        "MINSIZE 2 is a size of 4"
    );
    let s = symbol_style(&bounded(4.0, 4.0, 6.0, 3.0), &triangle, 2.0);
    assert_eq!((s.scale, s.outline_width), (3.0, 6.0), "the maximum last");
}

#[test]
fn each_reader_of_a_styles_symbol_tells_the_default_the_missing_the_unresolved_and_the_found() {
    // msDrawMarkerSymbol, msSymbolGetDefaultSize, and msGetMarkerSize each read the symbol by
    // its index from 1: index 0 is MapServer's default symbol, an index past the set is none, a
    // name the frontend did not resolve, or a binding, is not an index yet.
    let mut hatch = triangle(false);
    hatch.kind = SymbolKind::Hatch;
    let symbols = [triangle(true), hatch];
    let by = |symbol: Option<Bound<SymbolRef>>| Style {
        symbol,
        ..style(Some(10.0), None)
    };
    let index = |i| by(Some(Bound::Value(SymbolRef::Index(i))));
    let none = by(None);
    let zero = index(0);
    let past = index(3);
    let name = by(Some(Bound::Value(SymbolRef::Name("x.png".into()))));
    let binding = by(Some(Bound::Binding {
        binding: "sym".into(),
    }));
    let vector = index(1);
    let hatched = index(2);

    for (style, case) in [
        (&none, "none"),
        (&zero, "0"),
        (&name, "a name"),
        (&binding, "a binding"),
    ] {
        assert_eq!(mark(style, &symbols), Mark::Square, "{case}");
        assert!(!is_hatch(style, &symbols), "{case}");
        assert_eq!(default_size_of(style, &symbols), 1.0, "{case}");
    }
    assert_eq!(mark(&past, &symbols), Mark::Nothing);
    assert_eq!(mark(&vector, &symbols), Mark::Symbol(&symbols[0]));
    assert_eq!(mark(&hatched, &symbols), Mark::Square);

    assert!(!is_hatch(&past, &symbols));
    assert!(!is_hatch(&vector, &symbols));
    assert!(is_hatch(&hatched, &symbols));

    assert_eq!(default_size_of(&past, &symbols), 1.0);
    assert_eq!(default_size_of(&vector, &symbols), 2.0);
    assert_eq!(default_size_of(&hatched, &symbols), 1.0);

    assert_eq!(
        marker_size_of(&none, &symbols, &Pixmaps::new(), 1.0),
        Ok((1.0, 1.0))
    );
    assert_eq!(
        marker_size_of(&zero, &symbols, &Pixmaps::new(), 1.0),
        Ok((1.0, 1.0))
    );
    assert_eq!(
        marker_size_of(&past, &symbols, &Pixmaps::new(), 1.0),
        Err("symbol 3, which the symbol set does not hold".to_owned())
    );
    for unresolved in [&name, &binding] {
        assert_eq!(
            marker_size_of(unresolved, &symbols, &Pixmaps::new(), 1.0),
            Err("a symbol by a name that is not resolved".to_owned())
        );
    }
    assert_eq!(
        marker_size_of(&vector, &symbols, &Pixmaps::new(), 1.0),
        Ok((10.0, 10.0))
    );
    assert_eq!(
        marker_size_of(&hatched, &symbols, &Pixmaps::new(), 1.0),
        Ok((10.0, 10.0))
    );
}

#[test]
fn a_pixmap_s_marker_size_is_its_image_at_size_1_and_follows_its_height_otherwise() {
    // msGetMarkerSize, src/mapsymbol.c:720-799, for an image 40 wide and 20 high. `size` is an
    // int (722): SIZE 1 is 1, the image's own width and height (772-775); SIZE 10.9 is 10, the
    // height 10 and the width 10 / 20 × 40 = 20 (776-781); without a size, msSymbolGetDefaultSize
    // gives the image's height, 20, times the scale factor 1.5, 30, the width 30 / 20 × 40 = 60
    // (750-751).
    assert_eq!(
        pixmap_marker_size(&style(None, None), Some(1.0), 40, 20, 1.0),
        (40.0, 20.0)
    );
    assert_eq!(
        pixmap_marker_size(&style(None, None), Some(10.9), 40, 20, 1.0),
        (20.0, 10.0)
    );
    assert_eq!(
        pixmap_marker_size(&style(None, None), None, 40, 20, 1.5),
        (60.0, 30.0)
    );
    // MINSIZE and MAXSIZE hold the size unscaled after the cut (754-755): SIZE 3 at a factor of
    // 2 is 6, raised by MINSIZE 7.5 to 7, the width 7 / 20 × 40 = 14; SIZE 30 capped by MAXSIZE
    // 12.5 to 12, the width 24.
    let mut raised = style(None, None);
    raised.min_size = Some(7.5);
    assert_eq!(
        pixmap_marker_size(&raised, Some(3.0), 40, 20, 2.0),
        (14.0, 7.0)
    );
    let mut capped = style(None, None);
    capped.max_size = Some(12.5);
    assert_eq!(
        pixmap_marker_size(&capped, Some(30.0), 40, 20, 1.0),
        (24.0, 12.0)
    );
}
