//! The expected shapes are worked by hand from the C functions each test names, at the pin: the
//! arithmetic is chosen exact in doubles, so that a shape is held equal, not near.

use super::*;

fn c(x: f64, y: f64) -> Coord {
    Coord { x, y }
}

fn coords(points: &[(f64, f64)]) -> Vec<Coord> {
    points.iter().map(|&(x, y)| c(x, y)).collect()
}

fn line(points: &[(f64, f64)]) -> Geometry {
    Geometry::LineString(coords(points))
}

fn polygon(rings: &[&[(f64, f64)]]) -> Geometry {
    Geometry::Polygon(rings.iter().map(|r| coords(r)).collect())
}

/// `expression` applied to `geometry` at a cellsize of 1, with a budget of its own.
fn apply(expression: &str, geometry: &Geometry) -> Result<Geometry, Undrawn> {
    apply_at(expression, geometry, 1.0)
}

fn apply_at(expression: &str, geometry: &Geometry, cellsize: f64) -> Result<Geometry, Undrawn> {
    LayerTransform::parse(expression)?.apply(geometry, cellsize, &mut Budget::default())
}

/// The reason of an [`Undrawn::Unsupported`], which the test expects.
fn unsupported(result: Result<impl std::fmt::Debug, Undrawn>) -> String {
    match result {
        Err(Undrawn::Unsupported(why)) => why,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

fn limit(result: Result<impl std::fmt::Debug, Undrawn>) -> String {
    match result {
        Err(Undrawn::Limit(why)) => why,
        other => panic!("expected a limit, got {other:?}"),
    }
}

const ELL: &[(f64, f64)] = &[(0.0, 0.0), (10.0, 0.0), (10.0, 3.0)];

#[test]
fn generalize_keeps_the_points_at_least_the_tolerance_from_the_last_kept_and_the_last() {
    // msGeneralize: sqTolerance 4; (1,0) at 1 dropped, (2,0) at 4 kept (>=), (2,1) at 1 dropped,
    // the last point kept whatever its distance.
    let drawn = apply(
        "(generalize([shape], 2))",
        &line(&[(0.0, 0.0), (1.0, 0.0), (2.0, 0.0), (2.0, 1.0), (5.0, 1.0)]),
    );
    assert_eq!(drawn, Ok(line(&[(0.0, 0.0), (2.0, 0.0), (5.0, 1.0)])));
}

#[test]
fn generalize_keeps_the_first_part_alone() {
    // msGeneralize builds one line of shape->line[0]; a polygon's other rings, a line's other
    // parts, are dropped, and the type is kept.
    let two = Geometry::MultiLineString(vec![
        coords(&[(0.0, 0.0), (1.0, 0.0), (9.0, 0.0)]),
        coords(&[(0.0, 5.0), (9.0, 5.0)]),
    ]);
    assert_eq!(
        apply("generalize([shape], 2)", &two),
        Ok(line(&[(0.0, 0.0), (9.0, 0.0)]))
    );
    let holed = polygon(&[
        &[(0.0, 0.0), (9.0, 0.0), (9.0, 9.0), (0.0, 0.0)],
        &[(2.0, 1.0), (3.0, 1.0), (3.0, 2.0), (2.0, 1.0)],
    ]);
    assert_eq!(
        apply("generalize([shape], 1)", &holed),
        Ok(polygon(&[&[
            (0.0, 0.0),
            (9.0, 0.0),
            (9.0, 9.0),
            (0.0, 0.0)
        ]]))
    );
    // A point stays as it is; a first part without points becomes that empty part.
    let point = Geometry::Point(c(3.0, 4.0));
    assert_eq!(apply("generalize([shape], 5)", &point), Ok(point));
    let empty = Geometry::MultiLineString(vec![Vec::new(), coords(&[(0.0, 0.0), (1.0, 1.0)])]);
    assert_eq!(
        apply("generalize([shape], 5)", &empty),
        Ok(Geometry::LineString(Vec::new()))
    );
}

#[test]
fn densify_cuts_each_segment_longer_than_the_tolerance_into_equal_ones() {
    // msDensify: the segment of 10 is longer than 3, so floor(10 / 3) = 3 points are added at
    // (k + 1) * 2.5 / 10 of it; the segment of 3 is not longer than 3 and stays.
    let drawn = apply("(densify([shape], 3))", &line(ELL));
    let expected = line(&[
        (0.0, 0.0),
        (2.5, 0.0),
        (5.0, 0.0),
        (7.5, 0.0),
        (10.0, 0.0),
        (10.0, 3.0),
    ]);
    assert_eq!(drawn, Ok(expected));
    // A ring's every segment, the closing one included: 8 / 2.5 gives 3 points 2 apart.
    let ring = polygon(&[&[(0.0, 0.0), (8.0, 0.0), (0.0, 0.0)]]);
    let expected = polygon(&[&[
        (0.0, 0.0),
        (2.0, 0.0),
        (4.0, 0.0),
        (6.0, 0.0),
        (8.0, 0.0),
        (6.0, 0.0),
        (4.0, 0.0),
        (2.0, 0.0),
        (0.0, 0.0),
    ]]);
    assert_eq!(apply("densify([shape], 2.5)", &ring), Ok(expected));
}

#[test]
fn densify_keeps_a_part_of_one_point_and_refuses_one_of_none() {
    // msDensify: a part of one point runs no segment and copies its point; a part of none has its
    // point[0] read past its end in C, which the engine refuses.
    let one = Geometry::MultiLineString(vec![coords(&[(5.0, 5.0)]), coords(ELL)]);
    let drawn = apply("densify([shape], 3)", &one).unwrap();
    let Geometry::MultiLineString(parts) = drawn else {
        panic!("{drawn:?}")
    };
    assert_eq!(parts[0], coords(&[(5.0, 5.0)]));
    assert_eq!(parts[1].len(), 6);
    let none = Geometry::MultiLineString(vec![Vec::new(), coords(ELL)]);
    let why = unsupported(apply("densify([shape], 3)", &none));
    assert!(why.contains("densify on a part without points"), "{why}");
}

#[test]
fn densify_fails_on_a_point_and_on_a_tolerance_not_above_zero() {
    // msDensify returns NULL for a shape neither line nor polygon and for tolerance <= 0, and the
    // parser fails ("Executing densify failed.").
    let why = unsupported(apply("densify([shape], 2)", &Geometry::Point(c(1.0, 1.0))));
    assert!(why.contains("densify on a point shape"), "{why}");
    for expression in ["densify([shape], 0)", "densify([shape],-5)"] {
        let why = unsupported(apply(expression, &line(ELL)));
        assert!(why.contains("not above 0"), "{expression}: {why}");
    }
}

/// An outer square, a hole whose first point lies on the square's right edge, and an island.
fn rings_of_three() -> Geometry {
    polygon(&[
        &[
            (0.0, 0.0),
            (10.0, 0.0),
            (10.0, 10.0),
            (0.0, 10.0),
            (0.0, 0.0),
        ],
        &[(10.0, 5.0), (8.0, 4.0), (8.0, 6.0), (10.0, 5.0)],
        &[(20.0, 20.0), (22.0, 20.0), (22.0, 22.0), (20.0, 20.0)],
    ])
}

#[test]
fn outer_and_inner_keep_the_rings_msisouterring_finds_so() {
    // msIsOuterRing: the square's first two points lie in no other ring: outer. The hole's first
    // point (10, 5) lies on the square's right edge, which the point-in-ring test counts outside
    // (10 < 10 fails), its second (8, 4) inside; the two differ, so its third (8, 6), inside,
    // decides: one ring holds it, inner. The island lies in none: outer.
    let [square, hole, island] = [
        vec![
            (0.0, 0.0),
            (10.0, 0.0),
            (10.0, 10.0),
            (0.0, 10.0),
            (0.0, 0.0),
        ],
        vec![(10.0, 5.0), (8.0, 4.0), (8.0, 6.0), (10.0, 5.0)],
        vec![(20.0, 20.0), (22.0, 20.0), (22.0, 22.0), (20.0, 20.0)],
    ];
    assert_eq!(
        apply("(outer([shape]))", &rings_of_three()),
        Ok(polygon(&[&square, &island]))
    );
    assert_eq!(
        apply("(inner([shape]))", &rings_of_three()),
        Ok(polygon(&[&hole]))
    );
    // A ring alone is outer, whatever it holds.
    let alone = polygon(&[&[(0.0, 0.0), (1.0, 0.0), (0.0, 0.0)]]);
    assert_eq!(apply("outer([shape])", &alone), Ok(alone.clone()));
    assert_eq!(
        apply("inner([shape])", &alone),
        Ok(Geometry::Polygon(vec![]))
    );
}

#[test]
fn a_ring_whose_first_two_points_lie_inside_another_is_inner() {
    // msIsOuterRing: the hole's first two points, (2, 2) and (4, 2), lie strictly inside the
    // square, the same result twice and true, which toggles the status to inner; the square's lie
    // in no other ring.
    let square = [
        (0.0, 0.0),
        (10.0, 0.0),
        (10.0, 10.0),
        (0.0, 10.0),
        (0.0, 0.0),
    ];
    let hole = [(2.0, 2.0), (4.0, 2.0), (4.0, 4.0), (2.0, 2.0)];
    let holed = polygon(&[&square, &hole]);
    assert_eq!(apply("outer([shape])", &holed), Ok(polygon(&[&square])));
    assert_eq!(apply("inner([shape])", &holed), Ok(polygon(&[&hole])));
}

#[test]
fn outer_and_inner_fail_on_a_shape_that_is_no_polygon_and_on_a_ring_too_short() {
    // msRings2Shape returns NULL for a shape that is no polygon; msIsOuterRing reads a ring's
    // second point, past a ring of one, where there is another ring.
    let why = unsupported(apply("outer([shape])", &line(ELL)));
    assert!(why.contains("outer on a shape that is no polygon"), "{why}");
    let short = polygon(&[
        &[(0.0, 0.0), (5.0, 0.0), (0.0, 5.0), (0.0, 0.0)],
        &[(1.0, 1.0)],
    ]);
    let why = unsupported(apply("inner([shape])", &short));
    assert!(why.contains("ring of 1 points"), "{why}");
}

#[test]
fn smoothsia_averages_each_point_with_its_neighbours_at_half_weight() {
    // msSmoothShapeSIA(shape, 3, 1, NULL): a window of 3, coefficients 0.5, 1, 0.5 summing to 2;
    // the ends, whose window runs off the line, are kept. (3, 1) = ((0 + 4 + 2) / 2, (0 + 0 + 2)
    // / 2) and (5, 3) = ((2 + 4 + 4) / 2, (0 + 4 + 2) / 2).
    let zigzag = line(&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (8.0, 4.0)]);
    let once = line(&[(0.0, 0.0), (3.0, 1.0), (5.0, 3.0), (8.0, 4.0)]);
    assert_eq!(apply("(smoothsia([shape]))", &zigzag), Ok(once.clone()));
    // A size of 5 is cut to the part's count less one, 3.
    assert_eq!(apply("smoothsia([shape], 5)", &zigzag), Ok(once.clone()));
    // Two iterations smooth once, as the C smooths the shape it was given on both its first and
    // its second pass; three smooth twice: (2.75, 1.25) = ((0 + 3 + 2.5) / 2, (0 + 1 + 1.5) / 2),
    // (5.25, 2.75) = ((1.5 + 5 + 4) / 2, (0.5 + 3 + 2) / 2).
    assert_eq!(apply("smoothsia([shape], 3, 2)", &zigzag), Ok(once));
    assert_eq!(
        apply("smoothsia([shape], 3, 3)", &zigzag),
        Ok(line(&[(0.0, 0.0), (2.75, 1.25), (5.25, 2.75), (8.0, 4.0)]))
    );
}

#[test]
fn smoothsia_weighs_a_wider_window_by_halves() {
    // A window of 5: coefficients 0.25, 0.5, 1, 0.5, 0.25 summing to 2.5. At (2, 0): x (0.5 + 2 +
    // 1.5 + 1) / 2.5 = 2, y 0.25 * 8 / 2.5 = 0.8; at (3, 0): x (0.25 + 1 + 3 + 2 + 1.25) / 2.5 = 3,
    // y 0.5 * 8 / 2.5 = 1.6. The two points at each end are kept.
    let spike = line(&[
        (0.0, 0.0),
        (1.0, 0.0),
        (2.0, 0.0),
        (3.0, 0.0),
        (4.0, 8.0),
        (5.0, 0.0),
    ]);
    let expected = line(&[
        (0.0, 0.0),
        (1.0, 0.0),
        (2.0, 0.8),
        (3.0, 1.6),
        (4.0, 8.0),
        (5.0, 0.0),
    ]);
    assert_eq!(apply("smoothsia([shape], 5)", &spike), Ok(expected));
}

#[test]
fn smoothsia_wraps_its_window_around_a_ring() {
    // initLineWindow: first and last equal, a ring; nextLineWindow takes the point before the
    // first as numpoints - 2, the one after the last as 1. Each corner moves to the square's
    // centre by 1: (1, 1) = ((0 + 0 + 2) / 2, (2 + 0 + 0) / 2).
    let square = polygon(&[&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0), (0.0, 0.0)]]);
    let expected = polygon(&[&[(1.0, 1.0), (3.0, 1.0), (3.0, 3.0), (1.0, 3.0), (1.0, 1.0)]]);
    assert_eq!(apply("smoothsia([shape])", &square), Ok(expected));
    // A line of fewer than three points is copied.
    let short = line(&[(0.0, 0.0), (4.0, 4.0)]);
    assert_eq!(apply("smoothsia([shape])", &short), Ok(short.clone()));
}

#[test]
fn smoothsia_preprocessing_adds_midpoints_where_its_window_loops() {
    // processShapePathDistance, a window of 5. `all` adds the midpoints on either side of each
    // point whose window is whole, the middle one (4, 0) of five: (3, 0) and (5, 0).
    let straight = line(&[(0.0, 0.0), (2.0, 0.0), (4.0, 0.0), (6.0, 0.0), (8.0, 0.0)]);
    let all = preprocess(&Shape::of(&straight).unwrap(), true, &mut Budget::default());
    let expected = Shape::of(&line(&[
        (0.0, 0.0),
        (2.0, 0.0),
        (3.0, 0.0),
        (4.0, 0.0),
        (5.0, 0.0),
        (6.0, 0.0),
        (8.0, 0.0),
    ]));
    assert_eq!(all, expected);
    // `angle` adds them only where the window's path is over 1.3 times the distance between its
    // ends: not on a straight line (8 / 8), but on a zigzag (4 * sqrt(2) / 4).
    let angle = |g: &Geometry| preprocess(&Shape::of(g).unwrap(), false, &mut Budget::default());
    assert_eq!(angle(&straight), Shape::of(&straight));
    let zigzag = line(&[(0.0, 0.0), (1.0, 1.0), (2.0, 0.0), (3.0, 1.0), (4.0, 0.0)]);
    let expected = Shape::of(&line(&[
        (0.0, 0.0),
        (1.0, 1.0),
        (1.5, 0.5),
        (2.0, 0.0),
        (2.5, 0.5),
        (3.0, 1.0),
        (4.0, 0.0),
    ]));
    assert_eq!(angle(&zigzag), expected);
}

#[test]
fn smoothsia_preprocessing_closes_a_ring_on_its_new_first_point() {
    // On a ring every window is whole; the last point becomes the first point of the new ring,
    // the midpoint (0, 2) added before (0, 0).
    let square = polygon(&[&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0), (0.0, 0.0)]]);
    let all = preprocess(&Shape::of(&square).unwrap(), true, &mut Budget::default());
    let expected = Shape::of(&polygon(&[&[
        (0.0, 2.0),
        (0.0, 0.0),
        (2.0, 0.0),
        (2.0, 0.0),
        (4.0, 0.0),
        (4.0, 2.0),
        (4.0, 2.0),
        (4.0, 4.0),
        (2.0, 4.0),
        (2.0, 4.0),
        (0.0, 4.0),
        (0.0, 2.0),
        (0.0, 2.0),
    ]]));
    assert_eq!(all, expected);
}

#[test]
fn smoothsia_runs_its_preprocessing_before_its_passes() {
    // The straight line preprocessed with `ALL` (strcasecmp) as above, to x 0, 2, 3, 4, 5, 6, 8,
    // then smoothed by 0.5, 1, 0.5: (0 + 2 + 1.5) / 2 = 1.75, (1 + 3 + 2) / 2 = 3, …,
    // (2.5 + 6 + 4) / 2 = 6.25.
    let straight = line(&[(0.0, 0.0), (2.0, 0.0), (4.0, 0.0), (6.0, 0.0), (8.0, 0.0)]);
    let xs = [0.0, 1.75, 3.0, 4.0, 5.0, 6.25, 8.0];
    let expected = Geometry::LineString(xs.iter().map(|&x| c(x, 0.0)).collect());
    assert_eq!(
        apply("smoothsia([shape], 3, 1, 'ALL')", &straight),
        Ok(expected)
    );
    // Another text preprocesses nothing.
    assert_eq!(
        apply("smoothsia([shape], 3, 1, \"none\")", &straight),
        Ok(straight.clone())
    );
}

#[test]
fn transforms_nest_innermost_first() {
    // densify as above, then generalize by 4 (sqTolerance 16): (2.5, 0) and (7.5, 0) lie 2.5 from
    // the last kept.
    assert_eq!(
        apply("(generalize(densify([shape], 3), 4))", &line(ELL)),
        Ok(line(&[(0.0, 0.0), (5.0, 0.0), (10.0, 0.0), (10.0, 3.0)]))
    );
    // outer drops the hole; generalize by 11 (sqTolerance 121) keeps the square, the first ring
    // left, of its points the first, (10, 10) at 200, and the last; (10, 0) and (0, 10) lie 100
    // from the last kept.
    assert_eq!(
        apply("generalize(outer([shape]), 11)", &rings_of_three()),
        Ok(polygon(&[&[(0.0, 0.0), (10.0, 10.0), (0.0, 0.0)]]))
    );
    // Upper case, as the lexer is generated case-insensitive.
    assert_eq!(
        apply("(GENERALIZE(DENSIFY([SHAPE], 3), 4))", &line(ELL)),
        Ok(line(&[(0.0, 0.0), (5.0, 0.0), (10.0, 0.0), (10.0, 3.0)]))
    );
}

#[test]
fn numbers_are_computed_as_the_grammar_reduces_them() {
    let densified_by_3 = apply("densify([shape], 3)", &line(ELL)).unwrap();
    for expression in [
        // `*` before `-`.
        "densify([shape], 9 - 2 * 3)",
        // `%` of the numbers cut to `int`s: 7.9 % 4.2 is 7 % 4.
        "densify([shape], 7.9 % 4.2)",
        // `^` before the unary `-`, which the rule `'-' math_exp %prec NEG { $$ = $2; }` leaves
        // as it is: `- 3` is 3, where `-3`, one token, is a tolerance below 0.
        "densify([shape], - 3)",
        "densify([shape], (12 / 2 ^ 2))",
        // `-` and `/` associate to the left, 9 - 3 - 3 and 36 / 6 / 2, `^` to the right,
        // 3 ^ (1 ^ 2): the other way they would be 9, 12, and 9.
        "densify([shape], 9 - 3 - 3)",
        "densify([shape], 36 / 6 / 2)",
        "densify([shape], 3 ^ 1 ^ 2)",
        // `[map_cellsize]`, at a cellsize of 1.
        "densify([shape], [map_cellsize] * 3)",
    ] {
        assert_eq!(
            apply(expression, &line(ELL)).as_ref(),
            Ok(&densified_by_3),
            "{expression}"
        );
    }
    assert_eq!(
        apply_at("densify([shape], [map_cellsize])", &line(ELL), 3.0),
        Ok(densified_by_3)
    );
    let why = unsupported(apply("densify([shape], 3 / 0)", &line(ELL)));
    assert!(why.contains("divides by zero"), "{why}");
    let why = unsupported(apply("densify([shape], 3 % 0.5)", &line(ELL)));
    assert!(why.contains("3 % 0"), "{why}");
}

#[test]
fn the_functions_that_need_a_geometry_library_are_refused_by_name() {
    for name in ["simplify", "simplifypt", "buffer", "centerline"] {
        let why = unsupported(LayerTransform::parse(&format!("({name}([shape], 25))")));
        assert!(
            why.contains(&format!("calls {name}, which needs a geometry library")),
            "{why}"
        );
    }
    let why = unsupported(LayerTransform::parse("(difference([shape], [shape]))"));
    assert!(why.contains("calls difference"), "{why}");
    let why = unsupported(LayerTransform::parse("(javascript([shape], 'f.js'))"));
    assert!(why.contains("calls javascript"), "{why}");
    // Nested inside a drawn function, too.
    let why = unsupported(LayerTransform::parse(
        "(smoothsia(centerline(densify([shape], 5))))",
    ));
    assert!(why.contains("calls centerline"), "{why}");
}

#[test]
fn a_form_the_parse_does_not_read_is_refused() {
    for (expression, reason) in [
        (
            "(area([shape]))",
            "calls `area`, which is no shape function",
        ),
        (
            "(generalize([shape], [width]))",
            "binds `[width]` where a number is due",
        ),
        (
            "(generalize([shape], [data_cellsize]))",
            "binds `[data_cellsize]`",
        ),
        ("([geom])", "binds `[geom]` where a shape is due"),
        ("(generalize([shape]))", "generalize without its tolerance"),
        ("(outer([shape], 2))", "outer given too many arguments"),
        (
            "(smoothsia([shape], 3, 1, 'all', 2))",
            "smoothsia given too many",
        ),
        (
            "(smoothsia([shape], 3, 1, 4))",
            "gives smoothsia the number 4 where a quoted text is due",
        ),
        ("(generalize([shape], 2)) 3", "the number 3 after the shape"),
        (
            "(generalize([shape], 2)",
            "the end of the expression where `)` is due",
        ),
        ("(1 + 2)", "the number 1 where a shape is due"),
        (
            "(generalize([shape], 2 3))",
            "the number 3 where `)` is due",
        ),
        // `-2`, one token, follows `[map_cellsize]` as MapServer's lexer reads it.
        (
            "(densify([shape], [map_cellsize]-2))",
            "the number -2 where `)` is due",
        ),
        ("(smoothsia([shape], 3, 1, 'a\\'b'))", "quoted text"),
    ] {
        let why = unsupported(LayerTransform::parse(expression));
        assert!(why.contains(reason), "{expression}: {why}");
    }
}

#[test]
fn a_reason_quotes_the_expression_s_own_text_readably_and_bounded() {
    // A word, a binding, or a text of the expression is quoted up to 32 characters and cut with an
    // ellipsis past them; a token is named as written, never as Rust's debug output.
    let long = "a".repeat(40);
    let cut = format!("{}\u{2026}", "a".repeat(32));
    for expression in [
        format!("({long}([shape]))"),
        format!("(generalize([shape], [{long}]))"),
        format!("(generalize([shape], '{long}'))"),
    ] {
        let why = unsupported(LayerTransform::parse(&expression));
        assert!(why.contains(&cut), "{expression}: {why}");
        assert!(!why.contains(&long), "{expression}: {why}");
        assert!(!why.contains("Token"), "{expression}: {why}");
    }
    let why = unsupported(LayerTransform::parse("(generalize([shape], ?))"));
    assert!(why.contains("computes a number of `?`"), "{why}");
}

#[test]
fn a_multipoint_or_a_collection_is_refused() {
    let multipoint = Geometry::MultiPoint(coords(&[(0.0, 0.0), (1.0, 1.0)]));
    let why = unsupported(apply("generalize([shape], 1)", &multipoint));
    assert!(why.contains("multipoint"), "{why}");
    let collection = Geometry::Collection(vec![line(ELL)]);
    let why = unsupported(apply("generalize([shape], 1)", &collection));
    assert!(why.contains("collection"), "{why}");
}

#[test]
fn a_nesting_past_the_bound_of_expressions_is_a_limit() {
    // 23 parentheses about `[shape]` nest 24 levels and parse; one more passes the bound (Q-9).
    let nested = |levels: usize| format!("{}[shape]{}", "(".repeat(levels), ")".repeat(levels));
    assert!(LayerTransform::parse(&nested(23)).is_ok());
    let why = limit(LayerTransform::parse(&nested(24)));
    assert!(why.contains("nests deeper than 24 levels"), "{why}");
}

#[test]
fn densify_past_the_points_a_layer_may_add_is_a_limit() {
    // 10 / 0.000001 would add 10 000 000 points, past 1 000 000; none is made.
    let why = limit(apply("densify([shape], 0.000001)", &line(ELL)));
    assert!(why.contains("adds more than 1000000 points"), "{why}");
    // The budget is the layer's: two shapes of about 600 000 points each pass it together.
    let transform = LayerTransform::parse("densify([shape], 0.00001)").unwrap();
    let six = line(&[(0.0, 0.0), (6.0, 0.0)]);
    let mut budget = Budget::default();
    assert!(transform.apply(&six, 1.0, &mut budget).is_ok());
    let why = limit(transform.apply(&six, 1.0, &mut budget));
    assert!(why.contains("adds more than"), "{why}");
}

#[test]
fn smoothsia_or_rings_past_the_points_a_layer_may_visit_is_a_limit() {
    // 2 000 000 000 iterations of 4 points by a window of 3 would visit 2.4e10 points.
    let zigzag = line(&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (8.0, 4.0)]);
    let why = limit(apply("smoothsia([shape], 3, 2000000000)", &zigzag));
    assert!(why.contains("visits more than 100000000 points"), "{why}");
    // 5 001 rings of 4 points, each tested against all: 20 004 * 5 001 points, past 1e8.
    let ring = |i: f64| coords(&[(i, 0.0), (i + 0.5, 0.0), (i, 0.5), (i, 0.0)]);
    let many = Geometry::Polygon((0..5001).map(|i| ring(f64::from(i))).collect());
    let why = limit(apply("outer([shape])", &many));
    assert!(why.contains("visits more than"), "{why}");
}

#[test]
fn a_size_or_iterations_past_an_int_is_int_min_and_clamped() {
    // The x86-64 build converts a double past an `int` to INT_MIN, which msSmoothShapeSIA raises
    // to 3 and 1.
    let zigzag = line(&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (8.0, 4.0)]);
    let once = line(&[(0.0, 0.0), (3.0, 1.0), (5.0, 3.0), (8.0, 4.0)]);
    assert_eq!(
        apply("smoothsia([shape], 1e10, 1e10)", &zigzag),
        Ok(once.clone())
    );
    assert_eq!(apply("smoothsia([shape], 1, 0)", &zigzag), Ok(once));
}
