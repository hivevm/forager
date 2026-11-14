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

fn n(value: f64) -> TransformNumber {
    TransformNumber::Number(value)
}

fn shape() -> Box<TransformShape> {
    Box::new(TransformShape::Shape)
}

fn generalize(inner: TransformShape, tolerance: TransformNumber) -> TransformShape {
    TransformShape::Generalize(Box::new(inner), tolerance)
}

fn densify(inner: TransformShape, tolerance: TransformNumber) -> TransformShape {
    TransformShape::Densify(Box::new(inner), tolerance)
}

fn smooth(size: f64, iterations: f64, preprocessing: Option<Preprocessing>) -> TransformShape {
    TransformShape::SmoothSia {
        shape: shape(),
        size: n(size),
        iterations: n(iterations),
        preprocessing,
    }
}

fn op(op: NumberOp, left: TransformNumber, right: TransformNumber) -> TransformNumber {
    TransformNumber::Op(op, Box::new(left), Box::new(right))
}

/// `transform` applied to `geometry` at a cellsize of 1, with a budget of its own.
fn apply(transform: &TransformShape, geometry: &Geometry) -> Result<Geometry, Undrawn> {
    apply_at(transform, geometry, 1.0)
}

fn apply_at(
    transform: &TransformShape,
    geometry: &Geometry,
    cellsize: f64,
) -> Result<Geometry, Undrawn> {
    transformed(transform, geometry, cellsize, &mut Budget::default())
}

/// The reason of an [`Undrawn::Failed`], which the test expects.
fn failed(result: Result<impl std::fmt::Debug, Undrawn>) -> String {
    match result {
        Err(Undrawn::Failed(why)) => why,
        other => panic!("expected a failure, got {other:?}"),
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
        &generalize(TransformShape::Shape, n(2.0)),
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
        apply(&generalize(TransformShape::Shape, n(2.0)), &two),
        Ok(line(&[(0.0, 0.0), (9.0, 0.0)]))
    );
    let holed = polygon(&[
        &[(0.0, 0.0), (9.0, 0.0), (9.0, 9.0), (0.0, 0.0)],
        &[(2.0, 1.0), (3.0, 1.0), (3.0, 2.0), (2.0, 1.0)],
    ]);
    assert_eq!(
        apply(&generalize(TransformShape::Shape, n(1.0)), &holed),
        Ok(polygon(&[&[
            (0.0, 0.0),
            (9.0, 0.0),
            (9.0, 9.0),
            (0.0, 0.0)
        ]]))
    );
    // A point stays as it is; a first part without points becomes that empty part.
    let point = Geometry::Point(c(3.0, 4.0));
    assert_eq!(
        apply(&generalize(TransformShape::Shape, n(5.0)), &point),
        Ok(point)
    );
    let empty = Geometry::MultiLineString(vec![Vec::new(), coords(&[(0.0, 0.0), (1.0, 1.0)])]);
    assert_eq!(
        apply(&generalize(TransformShape::Shape, n(5.0)), &empty),
        Ok(Geometry::LineString(Vec::new()))
    );
}

#[test]
fn densify_cuts_each_segment_longer_than_the_tolerance_into_equal_ones() {
    // msDensify: the segment of 10 is longer than 3, so floor(10 / 3) = 3 points are added at
    // (k + 1) * 2.5 / 10 of it; the segment of 3 is not longer than 3 and stays.
    let drawn = apply(&densify(TransformShape::Shape, n(3.0)), &line(ELL));
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
    assert_eq!(
        apply(&densify(TransformShape::Shape, n(2.5)), &ring),
        Ok(expected)
    );
}

#[test]
fn densify_keeps_a_part_of_one_point_and_refuses_one_of_none() {
    // msDensify: a part of one point runs no segment and copies its point; a part of none has its
    // point[0] read past its end in C, which the engine refuses.
    let one = Geometry::MultiLineString(vec![coords(&[(5.0, 5.0)]), coords(ELL)]);
    let drawn = apply(&densify(TransformShape::Shape, n(3.0)), &one).unwrap();
    let Geometry::MultiLineString(parts) = drawn else {
        panic!("{drawn:?}")
    };
    assert_eq!(parts[0], coords(&[(5.0, 5.0)]));
    assert_eq!(parts[1].len(), 6);
    let none = Geometry::MultiLineString(vec![Vec::new(), coords(ELL)]);
    let why = failed(apply(&densify(TransformShape::Shape, n(3.0)), &none));
    assert!(why.contains("densify on a part without points"), "{why}");
}

#[test]
fn densify_fails_on_a_point_and_on_a_tolerance_not_above_zero() {
    // msDensify returns NULL for a shape neither line nor polygon and for tolerance <= 0, and the
    // parser fails ("Executing densify failed.").
    let why = failed(apply(
        &densify(TransformShape::Shape, n(2.0)),
        &Geometry::Point(c(1.0, 1.0)),
    ));
    assert!(why.contains("densify on a point shape"), "{why}");
    for tolerance in [0.0, -5.0] {
        let why = failed(apply(
            &densify(TransformShape::Shape, n(tolerance)),
            &line(ELL),
        ));
        assert!(why.contains("not above 0"), "{tolerance}: {why}");
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
        apply(&TransformShape::Outer(shape()), &rings_of_three()),
        Ok(polygon(&[&square, &island]))
    );
    assert_eq!(
        apply(&TransformShape::Inner(shape()), &rings_of_three()),
        Ok(polygon(&[&hole]))
    );
    // A ring alone is outer, whatever it holds.
    let alone = polygon(&[&[(0.0, 0.0), (1.0, 0.0), (0.0, 0.0)]]);
    assert_eq!(
        apply(&TransformShape::Outer(shape()), &alone),
        Ok(alone.clone())
    );
    assert_eq!(
        apply(&TransformShape::Inner(shape()), &alone),
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
    assert_eq!(
        apply(&TransformShape::Outer(shape()), &holed),
        Ok(polygon(&[&square]))
    );
    assert_eq!(
        apply(&TransformShape::Inner(shape()), &holed),
        Ok(polygon(&[&hole]))
    );
}

#[test]
fn outer_and_inner_fail_on_a_shape_that_is_no_polygon_and_on_a_ring_too_short() {
    // msRings2Shape returns NULL for a shape that is no polygon; msIsOuterRing reads a ring's
    // second point, past a ring of one, where there is another ring.
    let why = failed(apply(&TransformShape::Outer(shape()), &line(ELL)));
    assert!(why.contains("outer on a shape that is no polygon"), "{why}");
    let short = polygon(&[
        &[(0.0, 0.0), (5.0, 0.0), (0.0, 5.0), (0.0, 0.0)],
        &[(1.0, 1.0)],
    ]);
    let why = failed(apply(&TransformShape::Inner(shape()), &short));
    assert!(why.contains("ring of 1 points"), "{why}");
}

#[test]
fn smoothsia_averages_each_point_with_its_neighbours_at_half_weight() {
    // msSmoothShapeSIA(shape, 3, 1, NULL): a window of 3, coefficients 0.5, 1, 0.5 summing to 2;
    // the ends, whose window runs off the line, are kept. (3, 1) = ((0 + 4 + 2) / 2, (0 + 0 + 2)
    // / 2) and (5, 3) = ((2 + 4 + 4) / 2, (0 + 4 + 2) / 2).
    let zigzag = line(&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (8.0, 4.0)]);
    let once = line(&[(0.0, 0.0), (3.0, 1.0), (5.0, 3.0), (8.0, 4.0)]);
    assert_eq!(apply(&smooth(3.0, 1.0, None), &zigzag), Ok(once.clone()));
    // A size of 5 is cut to the part's count less one, 3.
    assert_eq!(apply(&smooth(5.0, 1.0, None), &zigzag), Ok(once.clone()));
    // Two iterations smooth once, as the C smooths the shape it was given on both its first and
    // its second pass; three smooth twice: (2.75, 1.25) = ((0 + 3 + 2.5) / 2, (0 + 1 + 1.5) / 2),
    // (5.25, 2.75) = ((1.5 + 5 + 4) / 2, (0.5 + 3 + 2) / 2).
    assert_eq!(apply(&smooth(3.0, 2.0, None), &zigzag), Ok(once));
    assert_eq!(
        apply(&smooth(3.0, 3.0, None), &zigzag),
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
    assert_eq!(apply(&smooth(5.0, 1.0, None), &spike), Ok(expected));
}

#[test]
fn smoothsia_wraps_its_window_around_a_ring() {
    // initLineWindow: first and last equal, a ring; nextLineWindow takes the point before the
    // first as numpoints - 2, the one after the last as 1. Each corner moves to the square's
    // centre by 1: (1, 1) = ((0 + 0 + 2) / 2, (2 + 0 + 0) / 2).
    let square = polygon(&[&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0), (0.0, 0.0)]]);
    let expected = polygon(&[&[(1.0, 1.0), (3.0, 1.0), (3.0, 3.0), (1.0, 3.0), (1.0, 1.0)]]);
    assert_eq!(apply(&smooth(3.0, 1.0, None), &square), Ok(expected));
    // A line of fewer than three points is copied.
    let short = line(&[(0.0, 0.0), (4.0, 4.0)]);
    assert_eq!(apply(&smooth(3.0, 1.0, None), &short), Ok(short.clone()));
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
    // The straight line preprocessed with `all` as above, to x 0, 2, 3, 4, 5, 6, 8, then
    // smoothed by 0.5, 1, 0.5: (0 + 2 + 1.5) / 2 = 1.75, (1 + 3 + 2) / 2 = 3, …,
    // (2.5 + 6 + 4) / 2 = 6.25.
    let straight = line(&[(0.0, 0.0), (2.0, 0.0), (4.0, 0.0), (6.0, 0.0), (8.0, 0.0)]);
    let xs = [0.0, 1.75, 3.0, 4.0, 5.0, 6.25, 8.0];
    let expected = Geometry::LineString(xs.iter().map(|&x| c(x, 0.0)).collect());
    assert_eq!(
        apply(&smooth(3.0, 1.0, Some(Preprocessing::All)), &straight),
        Ok(expected)
    );
    // Without a preprocessing, the straight line stays as it is.
    assert_eq!(
        apply(&smooth(3.0, 1.0, None), &straight),
        Ok(straight.clone())
    );
}

#[test]
fn transforms_nest_innermost_first() {
    // densify as above, then generalize by 4 (sqTolerance 16): (2.5, 0) and (7.5, 0) lie 2.5 from
    // the last kept.
    let densified = densify(TransformShape::Shape, n(3.0));
    assert_eq!(
        apply(&generalize(densified, n(4.0)), &line(ELL)),
        Ok(line(&[(0.0, 0.0), (5.0, 0.0), (10.0, 0.0), (10.0, 3.0)]))
    );
    // outer drops the hole; generalize by 11 (sqTolerance 121) keeps the square, the first ring
    // left, of its points the first, (10, 10) at 200, and the last; (10, 0) and (0, 10) lie 100
    // from the last kept.
    let outer = TransformShape::Outer(shape());
    assert_eq!(
        apply(&generalize(outer, n(11.0)), &rings_of_three()),
        Ok(polygon(&[&[(0.0, 0.0), (10.0, 10.0), (0.0, 0.0)]]))
    );
}

#[test]
fn numbers_are_computed_as_the_rules_of_math_exp_compute_them() {
    let densified_by_3 = apply(&densify(TransformShape::Shape, n(3.0)), &line(ELL)).unwrap();
    let cellsize = TransformNumber::MapCellsize;
    for number in [
        op(NumberOp::Mul, cellsize.clone(), n(3.0)),
        op(NumberOp::Add, cellsize.clone(), n(2.0)),
        op(NumberOp::Sub, n(4.0), cellsize.clone()),
        op(
            NumberOp::Div,
            n(6.0),
            op(NumberOp::Add, cellsize.clone(), n(1.0)),
        ),
        // `%` of the numbers cut to `int`s: 7.9 % (1 + 3.2) is 7 % 4.
        op(
            NumberOp::Mod,
            n(7.9),
            op(NumberOp::Add, cellsize.clone(), n(3.2)),
        ),
        op(NumberOp::Pow, n(3.0), cellsize.clone()),
    ] {
        assert_eq!(
            apply(&densify(TransformShape::Shape, number.clone()), &line(ELL)).as_ref(),
            Ok(&densified_by_3),
            "{number:?}"
        );
    }
    assert_eq!(
        apply_at(
            &densify(TransformShape::Shape, cellsize.clone()),
            &line(ELL),
            3.0
        ),
        Ok(densified_by_3)
    );
    // `yyerror(p, "Division by zero.")`, and the remainders the x86-64 build traps on, fail.
    let by = |op_: NumberOp, divisor: f64| {
        let number = op(op_, n(3.0), op(NumberOp::Sub, cellsize.clone(), n(divisor)));
        failed(apply(&densify(TransformShape::Shape, number), &line(ELL)))
    };
    assert!(by(NumberOp::Div, 1.0).contains("divides by zero"));
    assert!(by(NumberOp::Mod, 0.5).contains("3 % 0"));
    let min = op(NumberOp::Mod, op(NumberOp::Mul, cellsize, n(-3e9)), n(-1.0));
    let why = failed(apply(&densify(TransformShape::Shape, min), &line(ELL)));
    assert!(why.contains("-2147483648 % -1"), "{why}");
}

#[test]
fn a_multipoint_or_a_collection_is_refused() {
    let multipoint = Geometry::MultiPoint(coords(&[(0.0, 0.0), (1.0, 1.0)]));
    let generalized = generalize(TransformShape::Shape, n(1.0));
    let why = failed(apply(&generalized, &multipoint));
    assert!(why.contains("multipoint"), "{why}");
    let collection = Geometry::Collection(vec![line(ELL)]);
    let why = failed(apply(&generalized, &collection));
    assert!(why.contains("collection"), "{why}");
}

#[test]
fn densify_past_the_points_a_layer_may_add_is_a_limit() {
    // 10 / 0.000001 would add 10 000 000 points, past 1 000 000; none is made.
    let why = limit(apply(
        &densify(TransformShape::Shape, n(0.000001)),
        &line(ELL),
    ));
    assert!(why.contains("adds more than 1000000 points"), "{why}");
    // The budget is the layer's: two shapes of about 600 000 points each pass it together.
    let transform = densify(TransformShape::Shape, n(0.00001));
    let six = line(&[(0.0, 0.0), (6.0, 0.0)]);
    let mut budget = Budget::default();
    assert!(transformed(&transform, &six, 1.0, &mut budget).is_ok());
    let why = limit(transformed(&transform, &six, 1.0, &mut budget));
    assert!(why.contains("adds more than"), "{why}");
}

#[test]
fn smoothsia_or_rings_past_the_points_a_layer_may_visit_is_a_limit() {
    // 2 000 000 000 iterations of 4 points by a window of 3 would visit 2.4e10 points.
    let zigzag = line(&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (8.0, 4.0)]);
    let why = limit(apply(&smooth(3.0, 2_000_000_000.0, None), &zigzag));
    assert!(why.contains("visits more than 100000000 points"), "{why}");
    // 5 001 rings of 4 points, each tested against all: 20 004 * 5 001 points, past 1e8.
    let ring = |i: f64| coords(&[(i, 0.0), (i + 0.5, 0.0), (i, 0.5), (i, 0.0)]);
    let many = Geometry::Polygon((0..5001).map(|i| ring(f64::from(i))).collect());
    let why = limit(apply(&TransformShape::Outer(shape()), &many));
    assert!(why.contains("visits more than"), "{why}");
}

#[test]
fn a_size_or_iterations_past_an_int_is_int_min_and_clamped() {
    // The (int) cast gives INT_MIN for a double past an `int` (ADR-0036), which
    // msSmoothShapeSIA raises to 3 and 1.
    let zigzag = line(&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (8.0, 4.0)]);
    let once = line(&[(0.0, 0.0), (3.0, 1.0), (5.0, 3.0), (8.0, 4.0)]);
    assert_eq!(apply(&smooth(1e10, 1e10, None), &zigzag), Ok(once.clone()));
    assert_eq!(apply(&smooth(1.0, 0.0, None), &zigzag), Ok(once));
}
