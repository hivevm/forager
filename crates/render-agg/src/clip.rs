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
        let at_x = |x: f64| Point {
            x,
            y: a.y + (x - a.x) / (b.x - a.x) * (b.y - a.y),
        };
        let at_y = |y: f64| Point {
            x: a.x + (y - a.y) / (b.y - a.y) * (b.x - a.x),
            y,
        };
        match self {
            Edge::MinX => at_x(r.min_x),
            Edge::MaxX => at_x(r.max_x),
            Edge::MinY => at_y(r.min_y),
            Edge::MaxY => at_y(r.max_y),
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

fn clip_segment(a: Point, b: Point, r: &Rect) -> Option<(Point, Point)> {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    if !(dx.is_finite() && dy.is_finite()) {
        // The difference of two huge coordinates overflows; such a segment is dropped.
        return None;
    }
    let mut t0: f64 = 0.0;
    let mut t1: f64 = 1.0;
    for (p, q) in [
        (-dx, a.x - r.min_x),
        (dx, r.max_x - a.x),
        (-dy, a.y - r.min_y),
        (dy, r.max_y - a.y),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
        }
    }
    if t0 > t1 {
        return None;
    }
    let at = |t: f64| Point {
        x: a.x + t * dx,
        y: a.y + t * dy,
    };
    // The unclipped end keeps its exact value, so that consecutive segments join.
    let start = if t0 == 0.0 { a } else { at(t0) };
    let end = if t1 == 1.0 { b } else { at(t1) };
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
