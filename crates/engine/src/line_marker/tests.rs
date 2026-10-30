//! The placement of symbols along a line, each branch of msImagePolylineMarkers checked against
//! positions and angles worked out by hand from its code.

use std::f64::consts::{FRAC_PI_2, PI};

use forager_style_ir::{Bound, Rgba, SymbolKind, SymbolRef};

use super::*;

fn p(x: f64, y: f64) -> Point {
    Point { x, y }
}

/// A placement every `spacing` pixels after `initial_gap`, of a symbol 4 × 6 centred, on an
/// image of 100 × 100.
fn placement(spacing: f64, initial_gap: f64, auto_angle: bool) -> Placement {
    Placement {
        spacing,
        initial_gap,
        auto_angle,
        rotation: 0.0,
        size: (4.0, 6.0),
        anchor: [0.5, 0.5],
        image: (100.0, 100.0),
    }
}

fn placed(paths: &[Vec<Point>], placement: &Placement) -> Vec<(f64, f64, f64)> {
    place(paths, placement, 1000)
        .expect("within the limit")
        .into_iter()
        .map(|m| (m.at.x, m.at.y, m.rotation))
        .collect()
}

fn close(actual: &[(f64, f64, f64)], expected: &[(f64, f64, f64)]) {
    assert_eq!(
        actual.len(),
        expected.len(),
        "{actual:?} is not {expected:?}"
    );
    for (a, e) in actual.iter().zip(expected) {
        assert!(
            (a.0 - e.0).abs() < 1e-9 && (a.1 - e.1).abs() < 1e-9 && (a.2 - e.2).abs() < 1e-12,
            "{actual:?} is not {expected:?}"
        );
    }
}

#[test]
fn without_an_initial_gap_the_first_marker_lies_half_the_spacing_in() {
    // initialgap < 0: current_length = spacing / 2, then every spacing while it fits.
    let line = [vec![p(0.0, 50.0), p(100.0, 50.0)]];
    let expected: Vec<_> = [10.0, 30.0, 50.0, 70.0, 90.0]
        .map(|x| (x, 50.0, 0.0))
        .to_vec();
    close(&placed(&line, &placement(20.0, -1.0, false)), &expected);
}

#[test]
fn an_initial_gap_places_the_first_and_the_end_takes_a_marker_that_reaches_it() {
    // `while (current_length <= length)`: a marker at the very end is drawn.
    let line = [vec![p(0.0, 50.0), p(100.0, 50.0)]];
    let expected: Vec<_> = [0.0, 20.0, 40.0, 60.0, 80.0, 100.0]
        .map(|x| (x, 50.0, 0.0))
        .to_vec();
    close(&placed(&line, &placement(20.0, 0.0, false)), &expected);
}

#[test]
fn the_distance_left_over_carries_into_the_next_segment_and_a_negative_gap_turns() {
    // Segment 1, 30 long: 5 and 25; 45 - 30 = 15 carries over. Segment 2 runs down the image
    // (ry = 1): theta = -asin(1) = -π/2, at 15 and 35.
    let line = [vec![p(0.0, 0.0), p(30.0, 0.0), p(30.0, 40.0)]];
    close(
        &placed(&line, &placement(20.0, 5.0, true)),
        &[
            (5.0, 0.0, 0.0),
            (25.0, 0.0, 0.0),
            (30.0, 15.0, -FRAC_PI_2),
            (30.0, 35.0, -FRAC_PI_2),
        ],
    );
    // A positive gap keeps the style's rotation on every segment.
    let upright = Placement {
        rotation: 0.25,
        ..placement(20.0, 5.0, false)
    };
    let rotations: Vec<f64> = placed(&line, &upright).iter().map(|m| m.2).collect();
    assert_eq!(rotations, vec![0.25; 4]);
}

#[test]
fn the_angle_along_a_segment_is_mapservers_arcsine() {
    // theta = asin(ry), plus π where rx < 0, else negated; added to the style's rotation.
    let along = |a: Point, b: Point| {
        let turned = Placement {
            rotation: 0.5,
            ..placement(1000.0, 0.0, true)
        };
        placed(&[vec![a, b]], &turned)[0].2
    };
    assert!(
        (along(p(100.0, 0.0), p(0.0, 0.0)) - (0.5 + PI)).abs() < 1e-12,
        "leftwards"
    );
    assert!(
        (along(p(0.0, 40.0), p(0.0, 0.0)) - (0.5 + FRAC_PI_2)).abs() < 1e-12,
        "up"
    );
    let down_left = along(p(10.0, 0.0), p(0.0, 10.0));
    assert!(
        (down_left - (0.5 + PI / 4.0 + PI)).abs() < 1e-12,
        "down and left"
    );
}

#[test]
fn every_line_of_a_shape_starts_its_own_count() {
    let lines = [
        vec![p(0.0, 10.0), p(30.0, 10.0)],
        vec![p(0.0, 20.0), p(30.0, 20.0)],
    ];
    close(
        &placed(&lines, &placement(20.0, 3.0, false)),
        &[
            (3.0, 10.0, 0.0),
            (23.0, 10.0, 0.0),
            (3.0, 20.0, 0.0),
            (23.0, 20.0, 0.0),
        ],
    );
}

#[test]
fn a_line_too_short_for_a_marker_takes_one_at_half_its_length() {
    // spacing 40: current_length = 20 exceeds the 6 + 6 of the line; with no initial gap and
    // the line longer than the symbol is wide, one marker at 6, on the second segment, turned
    // with it, and not anchored.
    let line = [vec![p(10.0, 10.0), p(16.0, 10.0), p(16.0, 16.0)]];
    let anchored = Placement {
        anchor: [0.0, 0.0],
        ..placement(40.0, -1.0, true)
    };
    close(&placed(&line, &anchored), &[(16.0, 10.0, -FRAC_PI_2)]);
    // Not with an initial gap, nor on a line no longer than the symbol is wide.
    let short = [vec![p(10.0, 10.0), p(13.0, 10.0)]];
    assert!(placed(&short, &placement(40.0, -1.0, true)).is_empty());
    assert!(
        placed(
            &[vec![p(10.0, 10.0), p(16.0, 10.0)]],
            &placement(40.0, 7.0, true)
        )
        .is_empty()
    );
}

#[test]
fn the_anchor_point_lies_on_the_line_turned_with_the_symbol() {
    // ANCHORPOINT 0.5 1 of a symbol 4 × 6: ox = 0, oy = -3. On a segment running down the image
    // the rotation is -π/2: sin(π/2) = 1, cos(π/2) = 0, so the shift is (3, 0).
    let anchored = Placement {
        anchor: [0.5, 1.0],
        ..placement(20.0, 10.0, true)
    };
    close(
        &placed(&[vec![p(50.0, 0.0), p(50.0, 20.0)]], &anchored),
        &[(53.0, 10.0, -FRAC_PI_2)],
    );
    close(
        &placed(&[vec![p(0.0, 50.0), p(20.0, 50.0)]], &anchored),
        &[(10.0, 47.0, 0.0)],
    );
}

#[test]
fn markers_beyond_the_image_by_more_than_the_symbol_are_skipped_but_keep_the_count() {
    // From x = -1000 every 10: of the points, those from -4 to 104 lie within the symbol's
    // width of the image, 0, 10, … 100. The second segment, at x = 1000, lies beyond it.
    let line = [vec![p(-1000.0, 50.0), p(1000.0, 50.0), p(1000.0, 1000.0)]];
    let markers = placed(&line, &placement(10.0, 0.0, false));
    let xs: Vec<f64> = markers.iter().map(|m| m.0).collect();
    assert_eq!(
        xs,
        (0..=10).map(|i| f64::from(i) * 10.0).collect::<Vec<_>>()
    );
    // A line wholly outside places its markers nowhere and draws no fallback marker.
    assert!(
        placed(
            &[vec![p(-500.0, -500.0), p(-200.0, -500.0)]],
            &placement(10.0, -1.0, false)
        )
        .is_empty()
    );
}

#[test]
fn a_tiny_gap_on_a_long_line_exceeds_the_limit_and_places_nothing() {
    // Verifies: Q-9
    // 0.001 pixels apart, the 100 visible pixels of the line would take 100 000 markers.
    let line = [vec![p(-1e9, 50.0), p(1e9, 50.0)]];
    assert_eq!(place(&line, &placement(0.001, 0.0, false), 1000), None);
    // Far past the image, as many markers cost nothing: they are skipped, not visited.
    let far = [vec![p(-1e12, 5000.0), p(1e12, 5000.0)]];
    assert_eq!(
        place(&far, &placement(0.001, 0.0, false), 1000),
        Some(Vec::new())
    );
    // A spacing of 0 would place markers on one point without end, as MapServer's loop would.
    let short = [vec![p(0.0, 50.0), p(10.0, 50.0)]];
    assert_eq!(place(&short, &placement(0.0, 0.0, false), 1000), None);
    // Not a number: one marker at the initial gap, as the loop draws before it adds the spacing.
    close(
        &placed(&short, &placement(f64::NAN, 2.0, false)),
        &[(2.0, 50.0, 0.0)],
    );
}

fn arrow() -> Symbol {
    Symbol {
        name: None,
        kind: SymbolKind::Vector,
        paths: vec![vec![[0.0, 0.0], [4.0, 1.0], [0.0, 2.0]]],
        extent: Some([4.0, 2.0]),
        filled: false,
        anchor_point: None,
        character: None,
        font: None,
        image: None,
        transparent: None,
        span: None,
    }
}

#[test]
fn the_gap_scales_with_the_size_as_computesymbolstyle_scales_it() {
    // s->gap = gap · target_size / style_size: SIZE 5 bounded by MAXSIZE 2.5 halves it.
    let style = Style {
        color: Some(Bound::Value(Rgba::opaque(0, 0, 0))),
        symbol: Some(Bound::Value(SymbolRef::Index(1))),
        size: Some(Bound::Value(5.0)),
        max_size: Some(2.5),
        gap: Some(-20.0),
        ..Style::default()
    };
    assert_eq!(symbol::symbol_style(&style, &arrow()).gap, -10.0);
    let mut ops = Vec::new();
    let line = [vec![p(0.0, 50.0), p(40.0, 50.0)]];
    let mut budget = MarkerBudget::new();
    draw(&mut ops, &line, &style, &arrow(), 0.0, IMAGE, &mut budget);
    assert_eq!(ops.len(), 5, "at 0, 10, 20, 30, and 40");
    assert!(!budget.spent());
}

const IMAGE: (f64, f64) = (100.0, 100.0);

#[test]
fn the_markers_of_a_layer_share_one_budget_of_points() {
    // Verifies: Q-9
    // Five arrows a line, each an outline of 3 points: 15 points a line. Of a budget of 20, the
    // first line takes 15; the second, 15 more, would pass it and draws nothing, and nor does any
    // line after, however few its markers.
    let style = Style {
        color: Some(Bound::Value(Rgba::opaque(0, 0, 0))),
        symbol: Some(Bound::Value(SymbolRef::Index(1))),
        gap: Some(10.0),
        ..Style::default()
    };
    let line = [vec![p(0.0, 50.0), p(40.0, 50.0)]];
    let mut budget = MarkerBudget::of(20);
    let mut ops = Vec::new();
    draw(&mut ops, &line, &style, &arrow(), 0.0, IMAGE, &mut budget);
    assert_eq!((ops.len(), budget.spent()), (5, false));
    draw(&mut ops, &line, &style, &arrow(), 0.0, IMAGE, &mut budget);
    assert_eq!((ops.len(), budget.spent()), (5, true));
    let short = [vec![p(0.0, 50.0), p(1.0, 50.0)]];
    draw(&mut ops, &short, &style, &arrow(), 0.0, IMAGE, &mut budget);
    assert_eq!(ops.len(), 5, "nothing once the budget is spent");
}

#[test]
fn a_symbol_that_draws_nothing_places_nothing() {
    let style = Style {
        symbol: Some(Bound::Value(SymbolRef::Index(1))),
        gap: Some(0.000_001),
        ..Style::default()
    };
    let mut ops = Vec::new();
    let line = [vec![p(0.0, 50.0), p(100.0, 50.0)]];
    let mut budget = MarkerBudget::new();
    draw(&mut ops, &line, &style, &arrow(), 0.0, IMAGE, &mut budget);
    assert!(ops.is_empty());
    assert!(!budget.spent());
}
