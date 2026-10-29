//! Clipping before rasterization (ADR-0015): agg-rust hangs or overflows on coordinates far
//! outside the image, so every path is cut to the image enlarged by a margin first, and points
//! that are not finite are dropped.

use forager_render::Point;

/// An axis-aligned rectangle in pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl Rect {
    /// The image of `width` × `height` pixels, enlarged by `margin` on every side.
    pub fn around_image(width: u32, height: u32, margin: f64) -> Self {
        Self {
            min_x: -margin,
            min_y: -margin,
            max_x: f64::from(width) + margin,
            max_y: f64::from(height) + margin,
        }
    }

    /// The point moved onto the rectangle where the rounding of a crossing left it outside.
    fn clamp(&self, p: Point) -> Point {
        Point {
            x: p.x.clamp(self.min_x, self.max_x),
            y: p.y.clamp(self.min_y, self.max_y),
        }
    }
}

/// The point where the segment from `a` to `b` crosses the line on which the coordinate `axis`
/// is `value`. That coordinate is set exactly and the other is interpolated from the end nearer to
/// the line: from the far end, `a + t * (b - a)` cancels where one end lies orders of magnitude
/// farther out than the other, and the crossing would land pixels or the whole image away.
fn crossing(a: Point, b: Point, axis: Axis, value: f64) -> Point {
    let along_across = |p: Point| match axis {
        Axis::X => (p.x, p.y),
        Axis::Y => (p.y, p.x),
    };
    let ((a_along, a_across), (b_along, b_across)) = (along_across(a), along_across(b));
    let (near_along, near_across) = if (value - a_along).abs() <= (value - b_along).abs() {
        (a_along, a_across)
    } else {
        (b_along, b_across)
    };
    let other = near_across + (value - near_along) / (b_along - a_along) * (b_across - a_across);
    match axis {
        Axis::X => Point { x: value, y: other },
        Axis::Y => Point { x: other, y: value },
    }
}

#[derive(Clone, Copy)]
enum Axis {
    X,
    Y,
}

fn finite(points: &[Point]) -> impl Iterator<Item = Point> + '_ {
    points
        .iter()
        .copied()
        .filter(|p| p.x.is_finite() && p.y.is_finite())
}

/// Clips a closed ring with Sutherland–Hodgman. The result may run along the rectangle's edges,
/// which lie outside the image when the margin covers the stroke.
pub fn clip_ring(ring: &[Point], rect: &Rect) -> Vec<Point> {
    let mut points: Vec<Point> = finite(ring).collect();
    for edge in [Edge::MinX, Edge::MaxX, Edge::MinY, Edge::MaxY] {
        let Some(&last) = points.last() else {
            break;
        };
        let mut output = Vec::with_capacity(points.len() + 4);
        let mut previous = last;
        for &current in &points {
            match (edge.inside(previous, rect), edge.inside(current, rect)) {
                (true, true) => output.push(current),
                (true, false) => output.push(edge.intersect(previous, current, rect)),
                (false, true) => {
                    output.push(edge.intersect(previous, current, rect));
                    output.push(current);
                }
                (false, false) => {}
            }
            previous = current;
        }
        points = output;
    }
    // Huge coordinates can still overflow an intersection; such points are dropped as well.
    points.retain(|p| p.x.is_finite() && p.y.is_finite());
    if points.len() < 3 { Vec::new() } else { points }
}

#[derive(Clone, Copy)]
enum Edge {
    MinX,
    MaxX,
    MinY,
    MaxY,
}

impl Edge {
    fn inside(self, p: Point, r: &Rect) -> bool {
        match self {
            Edge::MinX => p.x >= r.min_x,
            Edge::MaxX => p.x <= r.max_x,
            Edge::MinY => p.y >= r.min_y,
            Edge::MaxY => p.y <= r.max_y,
        }
    }

    fn intersect(self, a: Point, b: Point, r: &Rect) -> Point {
        match self {
            Edge::MinX => crossing(a, b, Axis::X, r.min_x),
            Edge::MaxX => crossing(a, b, Axis::X, r.max_x),
            Edge::MinY => crossing(a, b, Axis::Y, r.min_y),
            Edge::MaxY => crossing(a, b, Axis::Y, r.max_y),
        }
    }
}

/// Clips an open polyline with Liang–Barsky, segment by segment, into the runs inside.
pub fn clip_line(line: &[Point], rect: &Rect) -> Vec<Vec<Point>> {
    let points: Vec<Point> = finite(line).collect();
    let mut runs: Vec<Vec<Point>> = Vec::new();
    let mut run: Vec<Point> = Vec::new();
    for pair in points.windows(2) {
        match clip_segment(pair[0], pair[1], rect) {
            Some((a, b)) => {
                if run.last() != Some(&a) {
                    if run.len() > 1 {
                        runs.push(std::mem::take(&mut run));
                    }
                    run.clear();
                    run.push(a);
                }
                run.push(b);
            }
            None => {
                if run.len() > 1 {
                    runs.push(std::mem::take(&mut run));
                }
                run.clear();
            }
        }
    }
    if run.len() > 1 {
        runs.push(run);
    }
    runs
}

/// Whether the parameter `t`, with `u = 1 - t`, lies past `t_other`, compared in the form that is
/// exact near them: t near 0, u near 1, where every t of an end far out rounds to 1.0 or its
/// neighbour.
fn past(t: f64, u: f64, t_other: f64, u_other: f64) -> bool {
    if t.max(t_other) <= 0.5 {
        t > t_other
    } else {
        u < u_other
    }
}

fn clip_segment(a: Point, b: Point, r: &Rect) -> Option<(Point, Point)> {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    if !(dx.is_finite() && dy.is_finite()) {
        // The difference of two huge coordinates overflows; such a segment is dropped.
        return None;
    }
    // Each parameter is kept twice, as t measured from `a` and as u = 1 - t measured from `b`:
    // where one end lies far farther out than the other, t rounds to 1 and u keeps the
    // difference, so a segment that misses the rectangle is not taken for one that ends on it.
    // Beside each parameter stands the edge it was met at, none for the segment's own end.
    let (mut t0, mut u0, mut entry): (f64, f64, Option<(Axis, f64)>) = (0.0, 1.0, None);
    let (mut t1, mut u1, mut exit): (f64, f64, Option<(Axis, f64)>) = (1.0, 0.0, None);
    for (p, qa, qb, edge) in [
        (-dx, a.x - r.min_x, b.x - r.min_x, (Axis::X, r.min_x)),
        (dx, r.max_x - a.x, r.max_x - b.x, (Axis::X, r.max_x)),
        (-dy, a.y - r.min_y, b.y - r.min_y, (Axis::Y, r.min_y)),
        (dy, r.max_y - a.y, r.max_y - b.y, (Axis::Y, r.max_y)),
    ] {
        if p == 0.0 {
            if qa < 0.0 {
                return None;
            }
            continue;
        }
        let (t, u) = (qa / p, -qb / p);
        if p < 0.0 {
            if past(t, u, t0, u0) {
                (t0, u0, entry) = (t, u, Some(edge));
            }
        } else if past(t1, u1, t, u) {
            (t1, u1, exit) = (t, u, Some(edge));
        }
    }
    if past(t0, u0, t1, u1) {
        return None;
    }
    // An end the segment keeps lies inside, but for a parameter that underflows to 0 at most a
    // subnormal distance out, and keeps its exact value, so that consecutive segments join; a
    // crossing is held to the rectangle against the rounding of its other
    // coordinate.
    let point = |edge: Option<(Axis, f64)>, end: Point| match edge {
        None => end,
        Some((axis, value)) => r.clamp(crossing(a, b, axis, value)),
    };
    let (start, end) = (point(entry, a), point(exit, b));
    let finite = |p: Point| p.x.is_finite() && p.y.is_finite();
    (finite(start) && finite(end)).then_some((start, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    fn inside(points: &[Point], r: &Rect) -> bool {
        points
            .iter()
            .all(|q| q.x >= r.min_x && q.x <= r.max_x && q.y >= r.min_y && q.y <= r.max_y)
    }

    #[test]
    fn a_ring_far_outside_is_cut_to_the_rectangle() {
        // Verifies: ADR-0015
        let r = Rect::around_image(64, 64, 2.0);
        let ring = clip_ring(&[p(-1e15, -1e15), p(1e15, 5.0), p(30.0, 1e15)], &r);
        assert!(ring.len() >= 3);
        assert!(inside(&ring, &r), "{ring:?}");
    }

    #[test]
    fn coordinates_near_the_limit_of_f64_never_yield_non_finite_points() {
        // Verifies: ADR-0015
        let r = Rect::around_image(64, 64, 2.0);
        for big in [1e200, 1e300, f64::MAX] {
            let ring = clip_ring(&[p(-big, -big), p(big, 5.0), p(30.0, big)], &r);
            assert!(inside(&ring, &r), "{big}: {ring:?}");
            for run in clip_line(&[p(-big, 1.0), p(big, 50.0), p(3.0, -big)], &r) {
                assert!(inside(&run, &r), "{big}: {run:?}");
            }
        }
    }

    #[test]
    fn a_segment_that_misses_the_rectangle_far_out_yields_nothing() {
        // Verifies: ADR-0015, Q-3
        // Inputs of the render fuzz target: one end lies so much farther out than the other that
        // the parameters at which the segment meets the edges round to 1.0, the other end, which
        // lies outside too; the segment misses the rectangle.
        for (line, r) in [
            (
                [
                    p(4.2356607143436245e307, -4.2356607143436245e307),
                    p(1.1167177796903676e90, -7.319297294523442e36),
                ],
                Rect::around_image(64, 3, 18.0),
            ),
            (
                [
                    p(-6.892022847183044e223, -1.3192564472220833e206),
                    p(-220661571.2063783, 5.140838393045592e63),
                ],
                Rect::around_image(96, 3, 2.0),
            ),
        ] {
            assert_eq!(clip_line(&line, &r), Vec::<Vec<Point>>::new(), "{line:?}");
            let ring = clip_ring(&[line[0], line[1], p(1.0, 1.0)], &r);
            assert!(inside(&ring, &r), "{line:?}: {ring:?}");
        }
    }

    #[test]
    fn a_segment_from_far_out_crosses_where_it_crosses() {
        // Verifies: ADR-0015
        let r = Rect::around_image(64, 64, 2.0);
        assert_eq!(
            clip_line(&[p(-1e19, 30.0), p(100.0, 30.0)], &r),
            vec![vec![p(-2.0, 30.0), p(66.0, 30.0)]]
        );
        assert_eq!(
            clip_line(&[p(-1e19, -1e19), p(100.0, 100.0)], &r),
            vec![vec![p(-2.0, -2.0), p(66.0, 66.0)]]
        );
        // Ends 1e17 to 1e19 out, where every t of a crossing rounds to 1.0 or its neighbour and
        // only u ranks the edges; the expected crossings are the exact ones, rounded.
        let near = |q: Point, e: Point| (q.x - e.x).abs() < 1e-9 && (q.y - e.y).abs() < 1e-9;
        for (line, r, expected) in [
            (
                [
                    p(2.724462101651667e18, -2.7043933580012513e18),
                    p(-139.76631398624681, 284.4106544082265),
                ],
                Rect::around_image(102, 154, 18.0),
                [p(120.0, 26.5578130157966), p(-18.0, 163.541286993255)],
            ),
            (
                [
                    p(-2.3760015071543967e18, 2.9094076710915876e18),
                    p(246.6954742644109, 29.93848751246685),
                ],
                Rect::around_image(154, 166, 100.0),
                [
                    p(53.91310312249169, 266.0),
                    p(246.6954742644109, 29.93848751246685),
                ],
            ),
        ] {
            let runs = clip_line(&line, &r);
            assert!(
                runs.len() == 1
                    && runs[0].len() == 2
                    && near(runs[0][0], expected[0])
                    && near(runs[0][1], expected[1]),
                "{line:?}: {runs:?}"
            );
        }
        // The exact crossings of the triangle's two edges from its far vertex are (-2, 8) and
        // (48, -2).
        let ring = clip_ring(&[p(-1e300, -1e300), p(50.0, 60.0), p(60.0, 10.0)], &r);
        for corner in [p(-2.0, 8.0), p(-2.0, -2.0), p(48.0, -2.0)] {
            assert!(
                ring.iter()
                    .any(|q| (q.x - corner.x).abs() < 1e-9 && (q.y - corner.y).abs() < 1e-9),
                "{corner:?}: {ring:?}"
            );
        }
    }

    #[test]
    fn a_ring_inside_is_kept_and_one_outside_vanishes() {
        let r = Rect::around_image(64, 64, 0.0);
        let square = [p(10.0, 10.0), p(20.0, 10.0), p(20.0, 20.0), p(10.0, 20.0)];
        assert_eq!(clip_ring(&square, &r).len(), 4);
        let outside = [p(100.0, 100.0), p(200.0, 100.0), p(200.0, 200.0)];
        assert!(clip_ring(&outside, &r).is_empty());
    }

    #[test]
    fn non_finite_points_are_dropped() {
        let r = Rect::around_image(64, 64, 0.0);
        let ring = clip_ring(
            &[
                p(f64::NAN, 1.0),
                p(10.0, 10.0),
                p(20.0, 10.0),
                p(15.0, f64::INFINITY),
                p(15.0, 20.0),
            ],
            &r,
        );
        assert_eq!(ring, vec![p(10.0, 10.0), p(20.0, 10.0), p(15.0, 20.0)]);
    }

    #[test]
    fn a_line_leaving_and_reentering_splits_into_runs() {
        let r = Rect::around_image(10, 10, 0.0);
        let runs = clip_line(&[p(2.0, 2.0), p(20.0, 2.0), p(20.0, 8.0), p(2.0, 8.0)], &r);
        assert_eq!(
            runs,
            vec![
                vec![p(2.0, 2.0), p(10.0, 2.0)],
                vec![p(10.0, 8.0), p(2.0, 8.0)]
            ]
        );
    }

    #[test]
    fn a_line_inside_stays_one_run() {
        let r = Rect::around_image(10, 10, 0.0);
        let line = [p(1.0, 1.0), p(5.0, 5.0), p(9.0, 1.0)];
        assert_eq!(clip_line(&line, &r), vec![line.to_vec()]);
    }
}
