//! The expected shapes are worked by hand from the C functions each test names, at the pin: the
//! arithmetic is chosen exact in doubles, so that a shape is held equal, not near.

use forager_style_ir::Geometry;

use super::super::Kind;
use super::super::tests::{coords, line, polygon};
use super::*;

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
#[expect(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    reason = "the clock bounds the test's run time and reaches no output (Q-2)"
)]
fn smoothsia_passes_nothing_over_parts_too_short_to_smooth() {
    // Verifies: Q-3, Q-9
    // The render fuzz target's timeout: parts of 0, 0, and 1 points, which msSmoothShapeSIA
    // leaves as they are on every one of its 89 409 257 passes, each charged one visit.
    let short = Shape {
        kind: Kind::Line,
        parts: vec![Vec::new(), Vec::new(), coords(&[(1.0, 2.0)])],
    };
    let started = std::time::Instant::now();
    let smoothed = smooth_sia(short.clone(), 3, 89_409_258, None, &mut Budget::default());
    assert_eq!(smoothed, Ok(short));
    assert!(
        started.elapsed().as_secs_f64() < 1.0,
        "{:?}",
        started.elapsed()
    );
}

#[test]
#[expect(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    reason = "the clock bounds the test's run time and reaches no output (Q-2)"
)]
fn smoothsia_passes_over_the_parts_it_smooths_and_reads_the_others_once() {
    // Verifies: Q-3, Q-9
    // One part of 4 points among 100 000 empty ones: the passes smooth the one alone, as a shape
    // of it alone is smoothed, and keep the empty parts where they were.
    let zigzag = coords(&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (8.0, 4.0)]);
    let alone = Shape {
        kind: Kind::Line,
        parts: vec![zigzag.clone()],
    };
    let alone = smooth_sia(alone, 3, 10_000, None, &mut Budget::default()).unwrap();
    let mut parts = vec![zigzag];
    parts.extend(std::iter::repeat_n(Vec::new(), 100_000));
    let many = Shape {
        kind: Kind::Line,
        parts,
    };
    let started = std::time::Instant::now();
    let smoothed = smooth_sia(many, 3, 10_000, None, &mut Budget::default()).unwrap();
    let elapsed = started.elapsed();
    assert_eq!(smoothed.parts[0], alone.parts[0]);
    assert_eq!(smoothed.parts.len(), 100_001);
    assert!(smoothed.parts[1..].iter().all(Vec::is_empty));
    assert!(elapsed.as_secs_f64() < 1.0, "{elapsed:?}");
}
