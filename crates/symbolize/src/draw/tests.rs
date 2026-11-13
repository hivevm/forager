// The tests of the drawing of a feature in a style work their values out from MapServer's
// msDrawShape(), src/mapdraw.c, msDrawLineSymbol(), msDrawShadeSymbol(), and
// msDrawMarkerSymbol(), src/maprendering.c, of the release pinned in scripts/fetch-msautotest.sh.
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
use forager_style_ir::{
    Bound, Coord, Extent, GeomTransform, Geometry, RenderMode, Rgba, Style, Symbol, SymbolKind,
    SymbolRef,
};

use super::*;
use crate::canvas::Canvas;
use crate::extent::View;
use crate::line::Pass;
use crate::line_marker::MarkerBudget;
use crate::symbol::{self, Factors, Pixmaps};
use crate::tile::Tiles;

const BLUE: Rgba = Rgba::opaque(30, 60, 200);
const BLACK: Rgba = Rgba::opaque(0, 0, 0);

fn p(x: f64, y: f64) -> Point {
    Point { x, y }
}

fn colored(color: Option<Rgba>, outline: Option<Rgba>) -> Style {
    Style {
        color: color.map(Bound::Value),
        outline_color: outline.map(Bound::Value),
        ..Style::default()
    }
}

/// The points of drawing operations.
fn points(ops: &[DrawOp]) -> usize {
    ops.iter()
        .map(|op| match op {
            DrawOp::Stroke { paths, .. } => paths.iter().map(Vec::len).sum(),
            DrawOp::Fill { rings, .. } => rings.iter().map(Vec::len).sum(),
            DrawOp::Hatch(_)
            | DrawOp::Glyphs { .. }
            | DrawOp::Pixmap { .. }
            | DrawOp::ResampledPixmap { .. }
            | DrawOp::TiledFill { .. } => 0,
        })
        .sum()
}

#[test]
fn the_symbols_along_the_lines_of_a_layer_share_one_budget() {
    // Verifies: Q-9
    // A filled triangle of 3 points every 0.01 pixels from 0.005 along lines 100 pixels long:
    // 10 000 markers, 30 000 points, each line far under the limit. A feature of 20 such lines
    // takes 600 000 points and one of 10 another 300 000; a third of 20 would pass the
    // million and draws nothing, nor does any feature after it.
    let view = View::for_image(
        Extent {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 100.0,
            max_y: 100.0,
        },
        101,
        101,
    )
    .unwrap();
    let triangle = Symbol {
        name: None,
        kind: SymbolKind::Vector,
        paths: vec![vec![[0.0, 0.0], [2.0, 0.0], [1.0, 2.0]]],
        extent: Some([2.0, 2.0]),
        filled: true,
        anchor_point: None,
        character: None,
        font: None,
        image: None,
        transparent: None,
        span: None,
    };
    let symbols = [triangle];
    let style = Style {
        symbol: Some(Bound::Value(SymbolRef::Index(1))),
        gap: Some(-0.01),
        ..colored(Some(BLUE), None)
    };
    let lines = |count: i32| {
        Geometry::MultiLineString(
            (0..count)
                .map(|i| {
                    let y = f64::from(i) * 5.0;
                    vec![Coord { x: 0.0, y }, Coord { x: 100.0, y }]
                })
                .collect(),
        )
    };
    let mut canvas = Canvas::new(&symbols, 101, 101, 1.0);
    let mut draw = |geometry: &Geometry| {
        let ops = draw_ops(
            RenderMode::Line,
            geometry,
            &style,
            Pass::Plain,
            &view,
            &mut canvas,
        );
        (points(&ops), canvas.markers_spent())
    };
    assert_eq!(draw(&lines(20)), (600_000, false));
    assert_eq!(draw(&lines(10)), (300_000, false));
    assert_eq!(draw(&lines(20)), (0, true));
    assert_eq!(
        draw(&lines(1)),
        (0, true),
        "nothing once the budget is spent"
    );
}

/// A view of 0..100 on 101 × 101 pixels: a map unit is a pixel, y downwards from 100.
fn unit_view() -> View {
    View::for_image(
        Extent {
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

fn coords(points: &[(f64, f64)]) -> Vec<Coord> {
    points.iter().map(|&(x, y)| Coord { x, y }).collect()
}

/// An L from pixel (10, 20) right to (90, 20), then down to (90, 90).
fn ell() -> Geometry {
    Geometry::LineString(coords(&[(10.0, 80.0), (90.0, 80.0), (90.0, 10.0)]))
}

fn transformed_style(transform: GeomTransform, symbol: Option<u32>) -> Style {
    Style {
        symbol: symbol.map(|i| Bound::Value(SymbolRef::Index(i))),
        size: Some(Bound::Value(6.0)),
        geom_transform: Some(transform),
        ..colored(Some(BLUE), None)
    }
}

fn transformed_ops(geometry: &Geometry, style: &Style, canvas: &mut Canvas) -> Vec<DrawOp> {
    draw_ops(
        RenderMode::Line,
        geometry,
        style,
        Pass::Plain,
        &unit_view(),
        canvas,
    )
}

#[test]
fn a_transformed_style_without_a_symbol_or_with_symbol_0_draws_nothing() {
    // msDrawMarkerSymbolInternal returns at a symbol of 0 or below, where a point layer draws
    // its stand-in square.
    let symbols = [circle()];
    for symbol in [None, Some(0)] {
        let style = transformed_style(GeomTransform::Start, symbol);
        let mut canvas = Canvas::new(&symbols, 101, 101, 1.0);
        assert!(transformed_ops(&ell(), &style, &mut canvas).is_empty());
    }
    let style = transformed_style(GeomTransform::Start, Some(1));
    let mut canvas = Canvas::new(&symbols, 101, 101, 1.0);
    assert!(!transformed_ops(&ell(), &style, &mut canvas).is_empty());
}

/// The ring msDrawTransformedShape builds for `bbox`, from the lower left of its bounds in pixels
/// up, around, and back (`src/mapgeomtransform.c:170-187`).
fn ring(min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> Vec<Point> {
    vec![
        p(min_x, min_y),
        p(min_x, max_y),
        p(max_x, max_y),
        p(max_x, min_y),
        p(min_x, min_y),
    ]
}

/// A line from pixel (-50, -20) to (150, 60) on the image of 101 × 101 pixels.
fn past_the_image() -> Geometry {
    Geometry::LineString(coords(&[(-50.0, 120.0), (150.0, 40.0)]))
}

#[test]
fn bbox_fills_and_outlines_the_bounds_held_by_the_padding_with_symbol_0() {
    // The padding (int)(MAX(5, -1) + 3) = 8 holds x = -50 at -8 and 150 at 101 + 8 = 109, y = -20
    // at -8, and keeps y = 60. msDrawShadeSymbol fills the ring of symbol 0 in the COLOR and
    // strokes it in the OUTLINECOLOR, at the colour's alpha, 5 pixels wide.
    let style = Style {
        width: Some(Bound::Value(5.0)),
        geom_transform: Some(GeomTransform::Bbox),
        ..colored(Some(BLUE), Some(BLACK))
    };
    let mut canvas = Canvas::new(&[], 101, 101, 1.0);
    let ops = transformed_ops(&past_the_image(), &style, &mut canvas);
    let expected = ring(-8.0, -8.0, 109.0, 60.0);
    let [
        DrawOp::Fill { rings, color, .. },
        DrawOp::Stroke {
            paths,
            color: outline,
            width,
            ..
        },
    ] = ops.as_slice()
    else {
        panic!("{ops:?}");
    };
    assert_eq!((rings, *color), (&vec![expected.clone()], BLUE));
    assert_eq!((paths, *outline, *width), (&vec![expected], BLACK, 5.0));
}

#[test]
fn bbox_strokes_the_ring_of_an_outline_colour_without_a_colour() {
    let style = Style {
        width: Some(Bound::Value(5.0)),
        geom_transform: Some(GeomTransform::Bbox),
        ..colored(None, Some(BLACK))
    };
    let mut canvas = Canvas::new(&[], 101, 101, 1.0);
    let ops = transformed_ops(&past_the_image(), &style, &mut canvas);
    let [DrawOp::Stroke { paths, color, .. }] = ops.as_slice() else {
        panic!("{ops:?}");
    };
    assert_eq!(
        (paths, *color),
        (&vec![ring(-8.0, -8.0, 109.0, 60.0)], BLACK)
    );
    // Neither colour: msDrawShadeSymbol draws nothing.
    let style = Style {
        outline_color: None,
        ..style
    };
    assert!(transformed_ops(&past_the_image(), &style, &mut canvas).is_empty());
}

#[test]
fn bbox_pads_by_the_larger_of_width_and_size_cut_to_an_int() {
    // SIZE 10.7 over the default WIDTH 1: (int)13.7 = 13; WIDTH 2.9 without a SIZE, which
    // initStyle sets to -1: (int)5.9 = 5. A shape within the image keeps its bounds.
    let mut canvas = Canvas::new(&[], 101, 101, 1.0);
    for (width, size, pad) in [(None, Some(10.7), 13.0), (Some(2.9), None, 5.0)] {
        let style = Style {
            width: width.map(Bound::Value),
            size: size.map(Bound::Value),
            geom_transform: Some(GeomTransform::Bbox),
            ..colored(Some(BLUE), None)
        };
        let ops = transformed_ops(&past_the_image(), &style, &mut canvas);
        let [DrawOp::Fill { rings, .. }] = ops.as_slice() else {
            panic!("{ops:?}");
        };
        assert_eq!(rings, &vec![ring(-pad, -pad, 101.0 + pad, 60.0)], "{pad}");
        let ops = transformed_ops(&ell(), &style, &mut canvas);
        let [DrawOp::Fill { rings, .. }] = ops.as_slice() else {
            panic!("{ops:?}");
        };
        assert_eq!(rings, &vec![ring(10.0, 20.0, 90.0, 90.0)], "{pad}");
    }
}

#[test]
fn bbox_pads_as_the_pin_s_int_does_past_an_int() {
    // A WIDTH past an `int`: cvttsd2si gives INT_MIN, whose negation wraps to INT_MIN again, so
    // the lower bounds stay and the upper ones are held at 101 + INT_MIN, as the x86-64 build at
    // the pin computes them; no panic.
    let style = Style {
        width: Some(Bound::Value(1e12)),
        geom_transform: Some(GeomTransform::Bbox),
        ..colored(Some(BLUE), None)
    };
    let mut canvas = Canvas::new(&[], 101, 101, 1.0);
    let ops = transformed_ops(&ell(), &style, &mut canvas);
    let [DrawOp::Fill { rings, .. }] = ops.as_slice() else {
        panic!("{ops:?}");
    };
    let high = 101.0 + f64::from(i32::MIN);
    assert_eq!(rings, &vec![ring(10.0, 20.0, high, high)]);
}

#[test]
fn bbox_in_an_outline_pass_pads_by_the_style_the_outline_prepares() {
    // msOutlineRenderingPrepareStyle runs before msDrawTransformedShape: WIDTH 5 and the default
    // SIZE -1 grow by OUTLINEWIDTH 2 to 9 and 1, so the padding is (int)(9 + 3) = 12, and the
    // colours swap: the ring is filled in the OUTLINECOLOR and stroked in the COLOR.
    let style = Style {
        width: Some(Bound::Value(5.0)),
        outline_width: Some(Bound::Value(2.0)),
        geom_transform: Some(GeomTransform::Bbox),
        ..colored(Some(BLUE), Some(BLACK))
    };
    let mut canvas = Canvas::new(&[], 101, 101, 1.0);
    let ops = draw_ops(
        RenderMode::Line,
        &past_the_image(),
        &style,
        Pass::Outline,
        &unit_view(),
        &mut canvas,
    );
    let [
        DrawOp::Fill { rings, color, .. },
        DrawOp::Stroke {
            color: outline,
            width,
            ..
        },
    ] = ops.as_slice()
    else {
        panic!("{ops:?}");
    };
    assert_eq!(rings, &vec![ring(-12.0, -12.0, 113.0, 60.0)]);
    assert_eq!((*color, *outline, *width), (BLACK, BLUE, 9.0));
}

#[test]
fn bbox_on_a_polygon_layer_hatches_and_counts_a_hatch_refused() {
    // Verifies: Q-9
    // The square from pixel (10, 10) to (90, 90) of a polygon layer: its ring hatched with
    // SIZE 10; with MAXSIZE 0 the spacing is 0, a hatch that would not end, refused and counted.
    let symbols = [hatch_symbol()];
    let square = Geometry::Polygon(vec![coords(&[
        (10.0, 10.0),
        (90.0, 10.0),
        (90.0, 90.0),
        (10.0, 90.0),
        (10.0, 10.0),
    ])]);
    let style = Style {
        symbol: Some(Bound::Value(SymbolRef::Index(1))),
        size: Some(Bound::Value(10.0)),
        geom_transform: Some(GeomTransform::Bbox),
        ..colored(Some(BLUE), None)
    };
    let polygon_ops = |style: &Style, canvas: &mut Canvas| {
        draw_ops(
            RenderMode::Polygon,
            &square,
            style,
            Pass::Plain,
            &unit_view(),
            canvas,
        )
    };
    let mut canvas = Canvas::new(&symbols, 101, 101, 1.0);
    let ops = polygon_ops(&style, &mut canvas);
    assert!(matches!(ops.as_slice(), [DrawOp::Hatch(_)]), "{ops:?}");
    assert_eq!(canvas.hatching.refused, 0);
    let refused = Style {
        min_size: Some(0.0),
        max_size: Some(0.0),
        ..style
    };
    assert!(polygon_ops(&refused, &mut canvas).is_empty());
    assert_eq!(canvas.hatching.refused, 1);
}

#[test]
fn without_angle_auto_a_transformed_symbol_keeps_its_angle() {
    // The arrow at the corner of the L: at its own ANGLE 30 without AUTO, turned by -45°, as
    // calcMidAngle gives it, with.
    let arrow = Symbol {
        kind: SymbolKind::Vector,
        paths: vec![vec![[0.0, 2.0], [2.0, 0.0], [4.0, 2.0]]],
        extent: Some([4.0, 2.0]),
        filled: false,
        ..circle()
    };
    let symbols = [arrow.clone()];
    let style = Style {
        angle: Some(Bound::Value(30.0)),
        ..transformed_style(GeomTransform::Vertices, Some(1))
    };
    let at_corner = |style: &Style| {
        let mut ops = Vec::new();
        symbol::symbol_ops(
            &mut ops,
            p(90.0, 20.0),
            style,
            &arrow,
            Factors::new(1.0, 1.0),
        );
        ops
    };
    let mut canvas = Canvas::new(&symbols, 101, 101, 1.0);
    assert_eq!(
        transformed_ops(&ell(), &style, &mut canvas),
        at_corner(&style)
    );
    let auto = Style {
        auto_angle: true,
        ..style.clone()
    };
    let turned = Style {
        angle: Some(Bound::Value(-45.0)),
        ..style
    };
    let drawn = transformed_ops(&ell(), &auto, &mut canvas);
    let expected = at_corner(&turned);
    let (DrawOp::Stroke { paths: a, .. }, DrawOp::Stroke { paths: b, .. }) =
        (&drawn[0], &expected[0])
    else {
        panic!("{drawn:?}");
    };
    for (a, b) in a[0].iter().zip(&b[0]) {
        assert!(
            (a.x - b.x).abs() < 1e-6 && (a.y - b.y).abs() < 1e-6,
            "{a:?} {b:?}"
        );
    }
}

#[test]
fn the_symbols_a_transform_places_share_the_layer_s_budget() {
    // Verifies: Q-9
    // Four inner vertices take four circles; a budget one circle short of them draws none
    // of the shape's, nor anything after it.
    let symbols = [circle()];
    let style = transformed_style(GeomTransform::Vertices, Some(1));
    let zigzag = Geometry::LineString(coords(&[
        (10.0, 50.0),
        (20.0, 60.0),
        (30.0, 50.0),
        (40.0, 60.0),
        (50.0, 50.0),
        (60.0, 60.0),
    ]));
    let mut canvas = Canvas::new(&symbols, 101, 101, 1.0);
    let drawn = transformed_ops(&zigzag, &style, &mut canvas);
    assert_eq!(drawn.len(), 4);
    let cost = points(&drawn);
    let mut canvas = Canvas::new(&symbols, 101, 101, 1.0);
    canvas.markers = MarkerBudget::of(cost - 1);
    assert!(transformed_ops(&zigzag, &style, &mut canvas).is_empty());
    assert!(canvas.markers_spent());
    assert!(transformed_ops(&ell(), &style, &mut canvas).is_empty());
    let mut canvas = Canvas::new(&symbols, 101, 101, 1.0);
    canvas.markers = MarkerBudget::of(cost);
    assert_eq!(transformed_ops(&zigzag, &style, &mut canvas).len(), 4);
    assert!(!canvas.markers_spent());
}

#[test]
fn the_markers_of_a_point_layer_share_the_layer_s_budget() {
    // Verifies: Q-9
    // Three circles at three points, priced as a transform prices them; a budget one circle
    // short of them draws none of the shape's, nor anything after it.
    let symbols = [circle()];
    let style = Style {
        geom_transform: None,
        ..transformed_style(GeomTransform::Vertices, Some(1))
    };
    let three = Geometry::MultiPoint(coords(&[(10.0, 50.0), (50.0, 50.0), (90.0, 50.0)]));
    let point_ops = |canvas: &mut Canvas| {
        draw_ops(
            RenderMode::Point,
            &three,
            &style,
            Pass::Plain,
            &unit_view(),
            canvas,
        )
    };
    let mut canvas = Canvas::new(&symbols, 101, 101, 1.0);
    let drawn = point_ops(&mut canvas);
    assert_eq!(drawn.len(), 3);
    let cost = points(&drawn);
    let mut canvas = Canvas::new(&symbols, 101, 101, 1.0);
    canvas.markers = MarkerBudget::of(cost - 1);
    assert!(point_ops(&mut canvas).is_empty());
    assert!(canvas.markers_spent());
    let mut canvas = Canvas::new(&symbols, 101, 101, 1.0);
    canvas.markers = MarkerBudget::of(cost);
    assert_eq!(point_ops(&mut canvas).len(), 3);
    assert!(!canvas.markers_spent());
}

#[test]
fn the_stand_in_squares_of_symbols_not_drawn_yet_pay_the_budget_and_symbol_0_s_does_not() {
    // Verifies: Q-9
    // An SVG, a TrueType, a hatch, or a bound symbol is drawn at a point as Forager's square,
    // which spends the layer's budget as the symbol would, by the maintainer's decision of
    // 2026-11-08; the square of no symbol or symbol 0, which MapServer draws nothing for,
    // spends none.
    let three = Geometry::MultiPoint(coords(&[(10.0, 50.0), (50.0, 50.0), (90.0, 50.0)]));
    let of_kind = |kind| Symbol {
        kind,
        ..hatch_symbol()
    };
    let symbols = [
        of_kind(SymbolKind::Svg),
        of_kind(SymbolKind::Truetype),
        of_kind(SymbolKind::Hatch),
    ];
    let at_points = |symbol: Option<Bound<SymbolRef>>| Style {
        symbol,
        geom_transform: None,
        ..transformed_style(GeomTransform::Vertices, None)
    };
    let point_ops = |style: &Style, budget: usize| {
        let mut canvas = Canvas::new(&symbols, 101, 101, 1.0);
        canvas.markers = MarkerBudget::of(budget);
        let ops = draw_ops(
            RenderMode::Point,
            &three,
            style,
            Pass::Plain,
            &unit_view(),
            &mut canvas,
        );
        (ops, canvas.markers_spent())
    };
    let bound = Bound::Binding {
        binding: "sym".to_owned(),
    };
    let index = |i| Some(Bound::Value(SymbolRef::Index(i)));
    for symbol in [index(1), index(2), index(3), Some(bound)] {
        let style = at_points(symbol.clone());
        let (drawn, _) = point_ops(&style, usize::MAX);
        assert_eq!(drawn.len(), 3, "{symbol:?}");
        // Three squares, each a ring of four points.
        let cost = points(&drawn);
        assert_eq!(cost, 12, "{symbol:?}");
        assert_eq!(
            point_ops(&style, cost - 1),
            (Vec::new(), true),
            "{symbol:?}"
        );
    }
    for symbol in [None, index(0)] {
        let (drawn, spent) = point_ops(&at_points(symbol.clone()), 0);
        assert_eq!((drawn.len(), spent), (3, false), "{symbol:?}");
    }
}

#[test]
fn a_centroid_of_a_collection_is_that_of_all_its_parts() {
    // Two lines of a collection, (10, 50)–(30, 50) and (70, 50)–(90, 50): one centroid, at
    // (50, 50), not one for each.
    let symbols = [circle()];
    let style = transformed_style(GeomTransform::Centroid, Some(1));
    let collection = Geometry::Collection(vec![
        Geometry::LineString(coords(&[(10.0, 50.0), (30.0, 50.0)])),
        Geometry::LineString(coords(&[(70.0, 50.0), (90.0, 50.0)])),
    ]);
    let mut canvas = Canvas::new(&symbols, 101, 101, 1.0);
    let drawn = transformed_ops(&collection, &style, &mut canvas);
    let mut expected = Vec::new();
    symbol::symbol_ops(
        &mut expected,
        p(50.0, 50.0),
        &style,
        &symbols[0],
        Factors::new(1.0, 1.0),
    );
    assert_eq!(drawn, expected);
}

fn hatch_symbol() -> Symbol {
    Symbol {
        name: None,
        kind: forager_style_ir::SymbolKind::Hatch,
        paths: Vec::new(),
        extent: None,
        filled: false,
        anchor_point: None,
        character: None,
        font: None,
        image: None,
        transparent: None,
        span: None,
    }
}

/// The hatch ops of `geometry` in a hatch style of SIZE 10 and WIDTH 1 on a view of the map
/// from 0 to 100 at a pixel a unit, y downwards from 100.
fn hatches(geometry: &Geometry) -> Vec<forager_render::Hatch> {
    let view = View::for_image(
        forager_style_ir::Extent {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 100.0,
            max_y: 100.0,
        },
        101,
        101,
    )
    .unwrap();
    let style = Style {
        symbol: Some(Bound::Value(forager_style_ir::SymbolRef::Index(1))),
        size: Some(Bound::Value(10.0)),
        ..colored(Some(BLUE), None)
    };
    let symbols = [hatch_symbol()];
    let mut canvas = Canvas::new(&symbols, 101, 101, 1.0);
    canvas.class(std::slice::from_ref(&style));
    let ops = draw_ops(
        RenderMode::Polygon,
        geometry,
        &style,
        Pass::Plain,
        &view,
        &mut canvas,
    );
    ops.into_iter()
        .map(|op| match op {
            DrawOp::Hatch(hatch) => hatch,
            other => panic!("{other:?}"),
        })
        .collect()
}

fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Coord> {
    [(x0, y0), (x1, y0), (x1, y1), (x0, y1), (x0, y0)]
        .map(|(x, y)| Coord { x, y })
        .to_vec()
}

#[test]
fn a_hatch_s_box_is_that_of_the_polygon_clipped_beyond_the_extent() {
    // The class's clip buffer is max(1, max(10, 1)) + 1 + 2 = 13 pixels: the polygon from
    // x = -1000 is clipped at -13, and the box of the hatch, grown by 0.7072, starts there.
    let reaching = Geometry::Polygon(vec![square(-1000.0, 10.0, 50.0, 60.0)]);
    let hatch = &hatches(&reaching)[0];
    assert!(
        (hatch.lines[0][0].x - (-13.7072)).abs() < 1e-9,
        "{:?}",
        hatch.lines[0]
    );
    // Inside the extent the polygon is not clipped.
    let inside = Geometry::Polygon(vec![square(5.0, 10.0, 50.0, 60.0)]);
    let hatch = &hatches(&inside)[0];
    assert!(
        (hatch.lines[0][0].x - 4.2928).abs() < 1e-9,
        "{:?}",
        hatch.lines[0]
    );
}

#[test]
fn a_multipolygon_is_hatched_as_one_shape() {
    // msDrawShadeSymbol hatches the whole shape: one box over both squares.
    let two = Geometry::MultiPolygon(vec![
        vec![square(5.0, 10.0, 20.0, 20.0)],
        vec![square(60.0, 70.0, 90.0, 90.0)],
    ]);
    let drawn = hatches(&two);
    assert_eq!(drawn.len(), 1);
    assert_eq!(drawn[0].gamma, Some(0.75));
    assert_eq!(drawn[0].rings.len(), 2);
    let first = drawn[0].lines[0];
    assert!((first[0].x - 4.2928).abs() < 1e-9 && (first[1].x - 91.2928).abs() < 1e-9);
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
fn at_a_factor_a_point_draws_its_symbol_scaled_and_its_stand_in_square_as_written() {
    // pointLayerDrawShape and msDrawTransformedShape hand msDrawMarkerSymbol the style's
    // scale factor; the square of SYMBOL 0 is Forager's stand-in, drawn at 1. Symbol 1 is a
    // vector square of side 2: at the factor 2, computeSymbolStyle's scale is SIZE 6 × 2 over
    // msSymbolGetDefaultSize's sizey 2, 6 (maprendering.c:62-65, mapsymbol.c:87), and its
    // outline the default WIDTH 1 × 2 (maprendering.c:69); msAdjustMarkerPos moves the point
    // by OFFSET (2, 1) × 2 (maprendering.c:1022-1023); agg2RenderVectorSymbol moves the
    // square's centre (1, 1) to the origin, scales it by 6, and moves it to the point
    // (mapagg.cpp:615-623).
    let square = Symbol {
        kind: SymbolKind::Vector,
        paths: vec![vec![
            [0.0, 0.0],
            [2.0, 0.0],
            [2.0, 2.0],
            [0.0, 2.0],
            [0.0, 0.0],
        ]],
        extent: Some([2.0, 2.0]),
        ..circle()
    };
    let symbols = [square];
    let ring = |x0: f64, y0: f64, x1: f64, y1: f64| {
        vec![vec![p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1), p(x0, y0)]]
    };
    let marked = |symbol: Option<u32>, mode: RenderMode, transform| Style {
        symbol: symbol.map(|i| Bound::Value(SymbolRef::Index(i))),
        size: Some(Bound::Value(6.0)),
        offset: Some([Bound::Value(2.0), Bound::Value(1.0)]),
        geom_transform: transform,
        ..colored(Some(BLUE), (mode == RenderMode::Point).then_some(BLACK))
    };
    let drawn = |style: &Style, mode: RenderMode, geometry: &Geometry, factor: f64| {
        let mut canvas = Canvas::new(&symbols, 101, 101, factor);
        draw_ops(
            mode,
            geometry,
            style,
            Pass::Plain,
            &unit_view(),
            &mut canvas,
        )
    };
    let point = Geometry::Point(Coord { x: 50.0, y: 50.0 });
    let style = marked(Some(1), RenderMode::Point, None);
    // The point (50, 50) moved to (54, 52): the square from (48, 46) to (60, 58), filled,
    // then outlined open, as agg2RenderVectorSymbol strokes its path.
    let expected = vec![
        DrawOp::Fill {
            rings: ring(48.0, 46.0, 60.0, 58.0),
            color: BLUE,
            gamma: None,
        },
        DrawOp::Stroke {
            paths: ring(48.0, 46.0, 60.0, 58.0),
            closed: false,
            color: BLACK,
            width: 2.0,
            cap: LineCap::Butt,
            join: LineJoin::Miter,
            dashes: Dashes::default(),
        },
    ];
    assert_eq!(drawn(&style, RenderMode::Point, &point, 2.0), expected);
    assert_ne!(drawn(&style, RenderMode::Point, &point, 1.0), expected);
    let square = marked(None, RenderMode::Point, None);
    assert_eq!(
        drawn(&square, RenderMode::Point, &point, 2.0),
        drawn(&square, RenderMode::Point, &point, 1.0)
    );
    // GEOMTRANSFORM start at the L's first vertex, the pixel (10, 20), moved to (14, 22)
    // (mapgeomtransform.c:132-142): the square from (8, 16) to (20, 28), filled alone.
    let start = marked(Some(1), RenderMode::Line, Some(GeomTransform::Start));
    assert_eq!(
        drawn(&start, RenderMode::Line, &ell(), 2.0),
        [DrawOp::Fill {
            rings: ring(8.0, 16.0, 20.0, 28.0),
            color: BLUE,
            gamma: None,
        }]
    );
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
fn a_stand_in_square_bounds_its_outline_as_compute_symbol_style_does() {
    // The square stands for a marker symbol, whose outline computeSymbolStyle bounds,
    // maprendering.c:69-71: MS_MAX with MINWIDTH, then MS_MIN with MAXWIDTH, here at a
    // factor of 1. A NaN width becomes MINWIDTH, and a MAXWIDTH below the MINWIDTH wins.
    let outline = |style: &Style| {
        let mut canvas = Canvas::new(&[], 101, 101, 2.0);
        let point = Geometry::Point(Coord { x: 50.0, y: 50.0 });
        let ops = draw_ops(
            RenderMode::Point,
            &point,
            style,
            Pass::Plain,
            &unit_view(),
            &mut canvas,
        );
        stroke(&ops[1]).0
    };
    assert_eq!(outline(&bounded_nan(2.0, 5.0, Some(BLACK))), 2.0);
    let crossed = Style {
        width: Some(Bound::Value(4.0)),
        ..bounded_nan(6.0, 3.0, Some(BLACK))
    };
    assert_eq!(outline(&crossed), 3.0, "MAXWIDTH last");
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

/// The operations of `style`, of symbol 1, at the point (50, 50) of [`unit_view`], with
/// `pixmaps` for `symbols`, and the pixmaps the canvas left undrawn.
fn pixmap_ops_of(
    style: &Style,
    symbols: &[Symbol],
    pixmaps: &Pixmaps,
) -> (Vec<DrawOp>, Vec<(usize, String)>) {
    let mut canvas = Canvas::new(symbols, 101, 101, 1.0).with_pixmaps(pixmaps);
    let point = Geometry::Point(Coord { x: 50.0, y: 50.0 });
    let ops = draw_ops(
        RenderMode::Point,
        &point,
        style,
        Pass::Plain,
        &unit_view(),
        &mut canvas,
    );
    let refused = canvas
        .named_pixmaps_refused()
        .into_iter()
        .map(|(index, (_, why))| (index, why))
        .collect();
    (ops, refused)
}

#[test]
fn a_pixmap_is_its_image_at_scale_1_unturned_and_resampled_otherwise() {
    let image = Rc::new(PremultipliedImage::new(4, 3, vec![9; 48]).unwrap());
    let pixmaps: Pixmaps = [("pin.png".to_owned(), Ok(Rc::clone(&image)))].into();
    let symbols = [pixmap_symbol(Some("pin.png"))];
    let style = |size: Option<f64>, angle: Option<f64>| Style {
        symbol: Some(Bound::Value(SymbolRef::Index(1))),
        size: size.map(Bound::Value),
        angle: angle.map(Bound::Value),
        offset: Some([Bound::Value(3.0), Bound::Value(-2.0)]),
        color: Some(Bound::Value(BLUE)),
        ..Style::default()
    };
    // Moved by the offset to (53, 48): MS_NINT(53 - 2) = 51 and MS_NINT(48 - 1.5) =
    // MS_NINT(46.5) = 46, half to even.
    for (size, angle) in [(None, None), (Some(3.0), Some(0.0)), (None, Some(360.0))] {
        let (ops, refused) = pixmap_ops_of(&style(size, angle), &symbols, &pixmaps);
        assert!(refused.is_empty());
        match &ops[..] {
            [
                DrawOp::Pixmap {
                    image: drawn,
                    left,
                    top,
                },
            ] => {
                assert!(Rc::ptr_eq(drawn, &image), "the image itself, not a copy");
                assert_eq!((*left, *top), (51, 46), "{size:?} {angle:?}");
            }
            other => panic!("{size:?} {angle:?}: {other:?}"),
        }
    }
    // A size other than the image's height scales it, an angle turns it: resampled into the
    // square about the point moved by the offset, (53, 48), agg2RenderPixmapSymbol's
    // half-side MS_NINT(4 × scale × 1.415) / 2 + 1 and the inverse of its transformation,
    // computed in trans_affine's order of operations at the pin.
    // SIZE 6: scale 6 / 3 = 2, MS_NINT(11.32) = 11, 11 / 2 + 1 = 6; the inverse of
    // translate(-2, -1.5), rotate(-0), scale(2), translate(53, 48).
    // ANGLE 30 at scale 1: MS_NINT(5.66) = 6, 6 / 2 + 1 = 4; rotate(-30 × MS_DEG_TO_RAD).
    let cases = [
        (Some(6.0), None, 6, [0.5, -0.0, -0.0, 0.5, -24.5, -22.5]),
        (
            None,
            Some(30.0),
            4,
            [
                0.866_025_403_784_438_7,
                0.499_999_999_999_999_94,
                -0.499_999_999_999_999_94,
                0.866_025_403_784_438_7,
                -19.899_346_400_575_258,
                -66.569_219_381_653_06,
            ],
        ),
    ];
    for (size, angle, half_side, [sx, shy, shx, sy, tx, ty]) in cases {
        let (ops, refused) = pixmap_ops_of(&style(size, angle), &symbols, &pixmaps);
        assert!(refused.is_empty());
        match &ops[..] {
            [
                DrawOp::ResampledPixmap {
                    image: drawn,
                    at,
                    half,
                    inverse,
                },
            ] => {
                assert!(Rc::ptr_eq(drawn, &image), "the image itself, not a copy");
                assert_eq!(*at, p(53.0, 48.0));
                assert_eq!(*half, half_side, "{size:?} {angle:?}");
                let expected = forager_render::Affine {
                    sx,
                    shy,
                    shx,
                    sy,
                    tx,
                    ty,
                };
                assert_eq!(*inverse, expected, "{size:?} {angle:?}");
            }
            other => panic!("{size:?} {angle:?}: {other:?}"),
        }
    }
    // A size of 0 is a scale of 0, which msDrawMarkerSymbol draws nothing at.
    let (ops, refused) = pixmap_ops_of(&style(Some(0.0), None), &symbols, &pixmaps);
    assert!(ops.is_empty() && refused.is_empty(), "{ops:?}");
}

#[test]
fn a_pixmap_along_a_line_needs_no_colour_and_a_polygon_s_draws_no_line() {
    // msDrawLineSymbol draws a symbol with a GAP whatever its colours (src/maprendering.c:
    // 666-688), and the cache's later pass draws a pixmap without a COLOR (src/mapdraw.c:
    // 1487-1494), its first style's again only with one (1453-1456). msDrawShadeSymbol hands
    // a pixmap to no line (753-762). The 4 × 3 image unscaled every 10 pixels along (0, 50),
    // (30, 50): at 5, 15, 25, blended at MS_NINT(x - 2), MS_NINT(50 - 1.5) = 48.
    let image = Rc::new(PremultipliedImage::new(4, 3, vec![9; 48]).unwrap());
    let pixmaps: Pixmaps = [("pin.png".to_owned(), Ok(Rc::clone(&image)))].into();
    let symbols = [
        pixmap_symbol(Some("pin.png")),
        pixmap_symbol(Some("gone.png")),
    ];
    let style = |index: u32, gap: f64| Style {
        symbol: Some(Bound::Value(SymbolRef::Index(index))),
        gap: Some(gap),
        outline_color: Some(Bound::Value(BLACK)),
        ..Style::default()
    };
    let drawn = |style: &Style, pass: Pass| {
        let mut canvas = Canvas::new(&symbols, 101, 101, 1.0).with_pixmaps(&pixmaps);
        let mut ops = Vec::new();
        let line = vec![vec![p(0.0, 50.0), p(30.0, 50.0)]];
        line_symbol(&mut ops, line, style, pass, &mut canvas);
        let placed: Vec<_> = ops
            .iter()
            .map(|op| match op {
                DrawOp::Pixmap { left, top, .. } => (*left, *top),
                other => panic!("{other:?}"),
            })
            .collect();
        let refused: Vec<_> = canvas.named_pixmaps_refused().into_keys().collect();
        (placed, refused)
    };
    let along = (vec![(3, 48), (13, 48), (23, 48)], vec![]);
    assert_eq!(drawn(&style(1, 10.0), Pass::Plain), along);
    assert_eq!(drawn(&style(1, 10.0), Pass::PlainIfColoredOrPixmap), along);
    assert_eq!(
        drawn(&style(1, 10.0), Pass::PlainIfColored),
        (vec![], vec![])
    );
    // An image not loaded is refused, and nothing drawn.
    assert_eq!(drawn(&style(2, 10.0), Pass::Plain), (vec![], vec![2]));
    // The engine loads the image of a line layer's style with a GAP, and of no other.
    assert_eq!(
        marked_pixmap(RenderMode::Line, &style(1, -10.0), &symbols),
        Some("pin.png")
    );
    assert_eq!(
        marked_pixmap(RenderMode::Line, &style(1, 0.0), &symbols),
        None
    );
    assert_eq!(
        marked_pixmap(RenderMode::Polygon, &style(1, 10.0), &symbols),
        Some("pin.png")
    );
    // A polygon's pixmap without a COLOR draws no outline, markers or stroke, only its tile.
    let mut tiles = Tiles::default();
    let mut canvas = Canvas::new(&symbols, 101, 101, 1.0)
        .with_pixmaps(&pixmaps)
        .with_tiles(&mut tiles);
    let mut ops = Vec::new();
    let ring = vec![vec![
        p(10.0, 10.0),
        p(90.0, 10.0),
        p(90.0, 90.0),
        p(10.0, 10.0),
    ]];
    assert!(shade(
        &mut ops,
        ring,
        &style(1, 10.0),
        &unit_view(),
        &mut canvas
    ));
    assert!(canvas.named_pixmaps_refused().is_empty());
    match ops.as_slice() {
        [DrawOp::TiledFill { tile, .. }] => {
            assert_eq!((tile.width(), tile.height()), (10, 10), "the GAP's");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_pixmap_s_scale_is_bounded_by_its_size_bounds_times_the_resolution_factor() {
    // computeSymbolStyle, maprendering.c:62-65, at a scale and resolution factor of 2, for an
    // image 3 high: SIZE 6 × 2 = 12 bounded by MAXSIZE 5 × 2 = 10, scale 10 / 3; SIZE 1 × 2 =
    // 2 raised by MINSIZE 4 × 2 = 8, scale 8 / 3. The half-side of the 4 × 3 image: MS_NINT(4
    // × 10 / 3 × 1.415 = 18.866...) = 19, 19 / 2 + 1 = 10; MS_NINT(4 × 8 / 3 × 1.415 =
    // 15.093...) = 15, 15 / 2 + 1 = 8. The inverse, by trans_affine's invert, scales by
    // (10 / 3) × 1 / ((10 / 3) × (10 / 3)) = 0.29999999999999993 in doubles, and 0.375.
    let image = Rc::new(PremultipliedImage::new(4, 3, vec![9; 48]).unwrap());
    let pixmaps: Pixmaps = [("pin.png".to_owned(), Ok(Rc::clone(&image)))].into();
    let symbols = [pixmap_symbol(Some("pin.png"))];
    let bounded = |size: f64| Style {
        symbol: Some(Bound::Value(SymbolRef::Index(1))),
        size: Some(Bound::Value(size)),
        min_size: Some(4.0),
        max_size: Some(5.0),
        ..Style::default()
    };
    for (size, half_side, scale) in [(6.0, 10, 0.299_999_999_999_999_93), (1.0, 8, 0.375)] {
        let mut canvas = Canvas::new(&symbols, 101, 101, 2.0).with_pixmaps(&pixmaps);
        let point = Geometry::Point(Coord { x: 50.0, y: 50.0 });
        let ops = draw_ops(
            RenderMode::Point,
            &point,
            &bounded(size),
            Pass::Plain,
            &unit_view(),
            &mut canvas,
        );
        match &ops[..] {
            [DrawOp::ResampledPixmap { half, inverse, .. }] => {
                assert_eq!(*half, half_side, "SIZE {size}");
                assert_eq!(inverse.sx, scale, "SIZE {size}");
                assert_eq!(inverse.sy, scale, "SIZE {size}");
            }
            other => panic!("SIZE {size}: {other:?}"),
        }
    }
}

#[test]
fn a_pixmap_without_its_image_draws_nothing_and_the_canvas_says_why() {
    let pixmaps: Pixmaps = [(
        "gone.png".to_owned(),
        Err("the image gone.png does not load".into()),
    )]
    .into();
    let symbols = [
        pixmap_symbol(Some("gone.png")),
        pixmap_symbol(None),
        pixmap_symbol(Some("never.png")),
    ];
    let style = |index| Style {
        symbol: Some(Bound::Value(SymbolRef::Index(index))),
        color: Some(Bound::Value(BLUE)),
        ..Style::default()
    };
    let (ops, refused) = pixmap_ops_of(&style(1), &symbols, &pixmaps);
    assert!(ops.is_empty());
    assert_eq!(
        refused,
        [(1, "the image gone.png does not load".to_owned())]
    );
    let (ops, refused) = pixmap_ops_of(&style(2), &symbols, &pixmaps);
    assert!(ops.is_empty());
    assert_eq!(
        refused,
        [(2, "the pixmap symbol names no image".to_owned())]
    );
    let (ops, refused) = pixmap_ops_of(&style(3), &symbols, &pixmaps);
    assert!(ops.is_empty());
    assert_eq!(
        refused,
        [(
            3,
            "the image never.png is not loaded for this render".to_owned()
        )]
    );
}

#[test]
fn a_pixmap_is_loaded_where_it_is_drawn_as_a_mark() {
    let symbols = [pixmap_symbol(Some("pin.png"))];
    let style = Style {
        symbol: Some(Bound::Value(SymbolRef::Index(1))),
        ..Style::default()
    };
    let transformed = Style {
        geom_transform: Some(GeomTransform::Vertices),
        ..style.clone()
    };
    assert_eq!(
        marked_pixmap(RenderMode::Point, &style, &symbols),
        Some("pin.png")
    );
    assert_eq!(marked_pixmap(RenderMode::Line, &style, &symbols), None);
    assert_eq!(
        marked_pixmap(RenderMode::Polygon, &style, &symbols),
        Some("pin.png")
    );
    assert_eq!(
        marked_pixmap(RenderMode::Line, &transformed, &symbols),
        Some("pin.png")
    );
    assert_eq!(
        marked_pixmap(RenderMode::Point, &Style::default(), &symbols),
        None
    );
}

/// The paths of the strokes `draw_ops` draws for `geometry` in `mode`, outlined in black.
fn outline_paths(mode: RenderMode, geometry: &Geometry, view: &View) -> Vec<Vec<Point>> {
    let style = colored(None, Some(BLACK));
    let mut canvas = Canvas::new(&[], 101, 101, 1.0);
    draw_ops(mode, geometry, &style, Pass::Plain, view, &mut canvas)
        .into_iter()
        .flat_map(|op| match op {
            DrawOp::Stroke { paths, .. } => paths,
            _ => panic!("an outline is a stroke"),
        })
        .collect()
}

#[test]
fn a_polygon_s_outline_drops_the_vertices_less_than_a_pixel_from_the_one_kept_before() {
    // msTransformShapeSimplify, src/mapprimitive.cpp:1002-1040: a ring keeps its first two
    // vertices, (10, 10) and (10.5, 10) half a pixel on; drops (11.5, 10), exactly a pixel
    // from the one kept before, as 1 > 1 fails, and (30, 10.5); keeps (30, 10), (30, 40), and
    // (10, 40), more than a pixel from the one kept before; and keeps its last two, (10, 39.5)
    // half a pixel from (10, 40), and the closing (10, 10).
    let ring = coords(&[
        (10.0, 90.0),
        (10.5, 90.0),
        (11.5, 90.0),
        (30.0, 90.0),
        (30.0, 89.5),
        (30.0, 60.0),
        (10.0, 60.0),
        (10.0, 60.5),
        (10.0, 90.0),
    ]);
    let expected = vec![vec![
        p(10.0, 10.0),
        p(10.5, 10.0),
        p(30.0, 10.0),
        p(30.0, 40.0),
        p(10.0, 40.0),
        p(10.0, 39.5),
        p(10.0, 10.0),
    ]];
    let polygon = Geometry::Polygon(vec![ring.clone()]);
    assert_eq!(
        outline_paths(RenderMode::Polygon, &polygon, &unit_view()),
        expected
    );
    // A polygon shape is simplified as a ring in a line layer too (958 against 1002: the
    // shape's type decides, not the layer's), and in each polygon of a multipolygon.
    assert_eq!(
        outline_paths(RenderMode::Line, &polygon, &unit_view()),
        expected
    );
    let multi = Geometry::MultiPolygon(vec![vec![ring]]);
    assert_eq!(
        outline_paths(RenderMode::Polygon, &multi, &unit_view()),
        expected
    );
}

#[test]
fn a_ring_of_fewer_than_four_vertices_is_not_drawn() {
    // msTransformShapeSimplify, src/mapprimitive.cpp:1012-1015: such a ring keeps no point;
    // the ring of four beside it is drawn.
    let three = coords(&[(10.0, 90.0), (50.0, 90.0), (10.0, 90.0)]);
    let four = coords(&[(10.0, 50.0), (50.0, 50.0), (50.0, 10.0), (10.0, 50.0)]);
    let polygon = Geometry::Polygon(vec![three, four]);
    assert_eq!(
        outline_paths(RenderMode::Polygon, &polygon, &unit_view()),
        vec![vec![
            p(10.0, 50.0),
            p(50.0, 50.0),
            p(50.0, 90.0),
            p(10.0, 50.0)
        ]]
    );
}

#[test]
fn a_line_drops_the_vertices_less_than_a_pixel_from_the_one_kept_before() {
    // msTransformShapeSimplify, src/mapprimitive.cpp:958-1001: a line keeps its first vertex,
    // drops (10.5, 50) half a pixel on, keeps (30, 50), and keeps a last vertex half a pixel
    // from it, (30.5, 50); a last vertex equal to the one kept before it is dropped; a line
    // left with fewer than two vertices is not drawn.
    let dense = Geometry::LineString(coords(&[
        (10.0, 50.0),
        (10.5, 50.0),
        (30.0, 50.0),
        (30.5, 50.0),
    ]));
    assert_eq!(
        outline_paths(RenderMode::Line, &dense, &unit_view()),
        vec![vec![p(10.0, 50.0), p(30.0, 50.0), p(30.5, 50.0)]]
    );
    let repeated = Geometry::MultiLineString(vec![
        coords(&[(10.0, 50.0), (30.0, 50.0), (30.0, 50.0)]),
        coords(&[(10.0, 20.0), (10.0, 20.0)]),
        coords(&[(10.0, 20.0)]),
    ]);
    assert_eq!(
        outline_paths(RenderMode::Line, &repeated, &unit_view()),
        vec![vec![p(10.0, 50.0), p(30.0, 50.0)]]
    );
}

#[test]
fn a_line_s_and_a_ring_s_vertices_are_placed_by_the_inverse_of_the_cellsize() {
    // MS_MAP2IMAGE_X_IC_DBL multiplies by inv_cs = 1 / cellsize, src/mapprimitive.cpp:953:
    // x = 0.3 on a cellsize of 0.1 is 0.3 × 10 = 3, where 0.3 / 0.1 is 2.9999999999999996.
    let view = View::for_image(
        Extent {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 10.0,
            max_y: 10.0,
        },
        101,
        101,
    )
    .unwrap();
    assert_eq!(view.cellsize, 0.1);
    let line = Geometry::LineString(coords(&[(0.3, 5.0), (8.0, 5.0)]));
    assert_eq!(outline_paths(RenderMode::Line, &line, &view)[0][0].x, 3.0);
    let ring = coords(&[(0.3, 5.0), (8.0, 5.0), (8.0, 1.0), (0.3, 5.0)]);
    let polygon = Geometry::Polygon(vec![ring]);
    assert_eq!(
        outline_paths(RenderMode::Polygon, &polygon, &view)[0][0].x,
        3.0
    );
}
