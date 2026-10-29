//! The geometry of symbols at a point, each branch of the MapServer functions it follows checked
//! against values worked out by hand from their code.

use super::*;

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
    symbol_ops(&mut ops, AT, style, symbol);
    ops
}

#[test]
fn the_degree_is_mapservers_constant() {
    assert_eq!(DEG_TO_RAD, "0.0174532925199432958".parse::<f64>().unwrap());
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
    assert_eq!(mark(&index(3), &symbols), Mark::Square, "a pixmap");
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
    let s = symbol_style(&outlined, &triangle(true));
    assert_eq!(
        (s.fill, s.outline, s.outline_width),
        (Some(RED), Some(BLACK), 3.0)
    );
    let s = symbol_style(&style(Some(10.0), None), &triangle(true));
    assert_eq!((s.fill, s.outline, s.outline_width), (Some(RED), None, 0.0));
}

#[test]
fn a_symbol_not_filled_is_outlined_with_the_colour_or_else_the_outline_colour() {
    // computeSymbolStyle, the branch of a symbol not filled: the outline is the style's width,
    // 1 by default.
    let s = symbol_style(&style(Some(10.0), None), &triangle(false));
    assert_eq!((s.fill, s.outline, s.outline_width), (None, Some(RED), 1.0));
    let only_outline = Style {
        outline_color: Some(Bound::Value(BLACK)),
        width: Some(Bound::Value(100.0)),
        ..Style::default()
    };
    let s = symbol_style(&only_outline, &triangle(false));
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
    assert_eq!(symbol_style(&style(None, None), &triangle(true)).scale, 1.0);
    assert_eq!(
        symbol_style(&style(Some(10.0), None), &triangle(true)).scale,
        5.0
    );
    let mut bounded = style(Some(10.0), None);
    bounded.max_size = Some(4.0);
    assert_eq!(symbol_style(&bounded, &triangle(true)).scale, 2.0);
    bounded.min_size = Some(8.0);
    assert_eq!(
        symbol_style(&bounded, &triangle(true)).scale,
        2.0,
        "MAXSIZE applies last"
    );
    assert_eq!(
        symbol_style(&style(None, Some(90.0)), &triangle(true)).rotation,
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
    let s = symbol_style(&small, &triangle(false));
    assert_eq!((s.scale, s.outline_width), (3.0, 2.0));
}

#[test]
fn a_vector_symbol_is_centred_scaled_and_turned_counter_clockwise() {
    // agg2RenderVectorSymbol: translate by half the extent, scale by 10 / 2, turn by -90° in
    // AGG's y-down sense, move to the point. The apex, below the centre, turns to its right.
    let ops = ops(&style(Some(10.0), Some(90.0)), &triangle(true));
    let [DrawOp::Fill { rings, color }] = ops.as_slice() else {
        panic!("{ops:?}");
    };
    assert_eq!(*color, RED);
    close(&rings[0], &[(45.0, 55.0), (45.0, 45.0), (55.0, 50.0)]);
}

#[test]
fn a_vector_symbol_not_filled_is_an_open_outline() {
    let ops = ops(&style(Some(10.0), None), &triangle(false));
    let [
        DrawOp::Outline {
            paths,
            closed,
            color,
            width,
        },
    ] = ops.as_slice()
    else {
        panic!("{ops:?}");
    };
    assert_eq!((*closed, *color, *width), (false, RED, 1.0));
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
    let [DrawOp::Outline { paths, .. }] = ops.as_slice() else {
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
fn the_marker_size_cuts_the_size_to_whole_pixels() {
    // msGetMarkerSize: `size` is an int; with SIZE, the width follows the height; without, the
    // symbol's own extent.
    let wide = symbol(
        SymbolKind::Vector,
        vec![vec![[0.0, 0.0], [4.0, 2.0]]],
        [4.0, 2.0],
        true,
    );
    assert_eq!(marker_size(&style(Some(10.7), None), &wide), (20.0, 10.0));
    assert_eq!(marker_size(&style(None, None), &wide), (4.0, 2.0));
    // The bounds cut again: MINSIZE 4.5 raises 3 to 4.
    let mut raised = style(Some(3.0), None);
    raised.min_size = Some(4.5);
    assert_eq!(marker_size(&raised, &wide), (8.0, 4.0));
}

#[test]
fn an_ellipse_is_aggs_polygon_of_its_scaled_extent() {
    // agg2RenderEllipseSymbol: a circle 1 across at size 12 has a radius of 6, which AGG's
    // calc_num_steps draws with uround(2π / (2 acos(6 / 6.125))) = 16 vertices, from the angle 0.
    let ops = ops(&style(Some(12.0), None), &circle(true));
    let [DrawOp::Fill { rings, color }] = ops.as_slice() else {
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
        DrawOp::Outline {
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
