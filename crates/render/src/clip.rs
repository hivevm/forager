//! The clip of every path to the image grown by a margin, and the reach of a glyph past the image
//! (ADR-0039): properties of the display list, which the bounds of a hatch measure with and every
//! renderer clips with, so that a bound and a renderer measure the same part of a path. They keep
//! the coordinates a renderer is handed within agg-rust's fixed-point limits (ADR-0015).

use crate::{PathCommand, Point};

/// Pixels beyond the image that a clipped path may reach, besides the reach of its stroke.
const CLIP_MARGIN: f64 = 2.0;

/// How far, in pixels, a point of a glyph path may lie outside the image: agg-rust holds
/// coordinates in 24.8 fixed point, which a farther point would overflow. The text path leaves out
/// a glyph that reaches farther, and a renderer draws no path that does.
pub const MAX_GLYPH_REACH: f64 = 4_194_304.0;

/// Whether every point of `path` lies within [`MAX_GLYPH_REACH`] of `image`,
/// `[min x, min y, max x, max y]`; a point that is no number does not. The bound is the image's
/// alone, whatever the outline, so that it holds below agg-rust's limit. The text path keeps a
/// glyph, and a renderer draws a glyph path, only when this holds of the same image, so that the
/// renderer leaves out no label the text path kept.
pub fn within_glyph_reach(image: [f64; 4], path: &[PathCommand]) -> bool {
    let within = |p: &Point| {
        p.x >= image[0] - MAX_GLYPH_REACH
            && p.y >= image[1] - MAX_GLYPH_REACH
            && p.x <= image[2] + MAX_GLYPH_REACH
            && p.y <= image[3] + MAX_GLYPH_REACH
    };
    path.iter().all(|command| match command {
        PathCommand::MoveTo(p) | PathCommand::LineTo(p) => within(p),
        PathCommand::QuadTo(c, p) => within(c) && within(p),
        PathCommand::CubicTo(c0, c1, p) => within(c0) && within(c1) && within(p),
        PathCommand::Close => true,
    })
}

/// The width a stroke of `width` is drawn at on an image of `image_width` × `image_height`
/// pixels: a wider stroke covers the image all the same, and the margin stays small enough for
/// agg-rust's fixed-point coordinates.
pub fn drawn_width(width: f64, image_width: u32, image_height: u32) -> f64 {
    width.min(2.0 * (f64::from(image_width) + f64::from(image_height)))
}

/// An axis-aligned rectangle in pixels: outside this module, only the image grown by the margin,
/// [`Rect::around_image`], so that no clip skips the margin.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}

impl Rect {
    /// The corners, `[min x, min y, max x, max y]`.
    pub fn bounds(&self) -> [f64; 4] {
        [self.min_x, self.min_y, self.max_x, self.max_y]
    }

    /// The image of `width` × `height` pixels grown by the margin past the image and by `reach`
    /// beyond it on every side, the reach of a stroke past its path.
    pub fn around_image(width: u32, height: u32, reach: f64) -> Self {
        let grow = CLIP_MARGIN + reach;
        Self {
            min_x: -grow,
            min_y: -grow,
            max_x: f64::from(width) + grow,
            max_y: f64::from(height) + grow,
        }
    }

    /// The point moved onto the rectangle where the rounding of a crossing left it outside.
    fn clamp(&self, p: Point) -> Point {
        Point {
            x: p.x.clamp(self.min_x, self.max_x),
            y: p.y.clamp(self.min_y, self.max_y),
        }
    }

    /// The four edges, in the order every clip meets them.
    fn edges(&self) -> [Edge; 4] {
        let edge = |axis, value, is_min| Edge {
            axis,
            value,
            is_min,
        };
        [
            edge(Axis::X, self.min_x, true),
            edge(Axis::X, self.max_x, false),
            edge(Axis::Y, self.min_y, true),
            edge(Axis::Y, self.max_y, false),
        ]
    }
}

/// The coordinate a line of a rectangle's edge fixes.
#[derive(Clone, Copy, Debug)]
enum Axis {
    X,
    Y,
}

impl Axis {
    /// The coordinate of `p` along this axis, and the other.
    fn along_across(self, p: Point) -> (f64, f64) {
        match self {
            Axis::X => (p.x, p.y),
            Axis::Y => (p.y, p.x),
        }
    }
}

/// An edge of a rectangle: the line on which the coordinate `axis` is `value`, and on which side
/// of it the rectangle lies.
#[derive(Clone, Copy, Debug)]
struct Edge {
    axis: Axis,
    value: f64,
    /// Whether the rectangle lies where the coordinate is at least `value`, not at most.
    is_min: bool,
}

impl Edge {
    /// Whether `p` lies on the rectangle's side of the edge, or on it.
    fn inside(self, p: Point) -> bool {
        let (along, _) = self.axis.along_across(p);
        if self.is_min {
            along >= self.value
        } else {
            along <= self.value
        }
    }

    /// The point where the segment from `a` to `b` crosses the edge's line. That coordinate is
    /// set exactly and the other is interpolated from the end nearer to the line: from the far
    /// end, `a + t * (b - a)` cancels where one end lies orders of magnitude farther out than the
    /// other, and the crossing would land pixels or the whole image away.
    fn crossing(self, a: Point, b: Point) -> Point {
        let value = self.value;
        let ((a_along, a_across), (b_along, b_across)) =
            (self.axis.along_across(a), self.axis.along_across(b));
        let (near_along, near_across) = if (value - a_along).abs() <= (value - b_along).abs() {
            (a_along, a_across)
        } else {
            (b_along, b_across)
        };
        let other =
            near_across + (value - near_along) / (b_along - a_along) * (b_across - a_across);
        match self.axis {
            Axis::X => Point { x: value, y: other },
            Axis::Y => Point { x: other, y: value },
        }
    }
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

/// The part of the segment from `a` to `b` inside the rectangle, by Liang–Barsky; `None` where
/// the segment misses it, or where an end or a crossing is not finite.
pub fn segment(a: Point, b: Point, r: &Rect) -> Option<(Point, Point)> {
    if !((b.x - a.x).is_finite() && (b.y - a.y).is_finite()) {
        // The difference of two huge coordinates overflows; such a segment is dropped.
        return None;
    }
    // Each parameter is kept twice, as t measured from `a` and as u = 1 - t measured from `b`:
    // where one end lies far farther out than the other, t rounds to 1 and u keeps the
    // difference, so a segment that misses the rectangle is not taken for one that ends on it.
    // Beside each parameter stands the edge it was met at, none for the segment's own end.
    let (mut t0, mut u0, mut entry): (f64, f64, Option<Edge>) = (0.0, 1.0, None);
    let (mut t1, mut u1, mut exit): (f64, f64, Option<Edge>) = (1.0, 0.0, None);
    for edge in r.edges() {
        let ((a_along, _), (b_along, _)) = (edge.axis.along_across(a), edge.axis.along_across(b));
        let d = b_along - a_along;
        let (p, qa, qb) = if edge.is_min {
            (-d, a_along - edge.value, b_along - edge.value)
        } else {
            (d, edge.value - a_along, edge.value - b_along)
        };
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
    let point = |edge: Option<Edge>, end: Point| match edge {
        None => end,
        Some(edge) => r.clamp(edge.crossing(a, b)),
    };
    let (start, end) = (point(entry, a), point(exit, b));
    (finite(start) && finite(end)).then_some((start, end))
}

fn finite(p: Point) -> bool {
    p.x.is_finite() && p.y.is_finite()
}

/// Clips an open polyline with Liang–Barsky, segment by segment, into the runs inside; points that
/// are not finite are dropped.
pub fn line(line: &[Point], rect: &Rect) -> Vec<Vec<Point>> {
    let points: Vec<Point> = line.iter().copied().filter(|p| finite(*p)).collect();
    let mut runs: Vec<Vec<Point>> = Vec::new();
    let mut run: Vec<Point> = Vec::new();
    for pair in points.windows(2) {
        match segment(pair[0], pair[1], rect) {
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

/// Clips a closed ring with Sutherland–Hodgman; points that are not finite are dropped. The
/// result may run along the rectangle's edges, which lie outside the image when the margin covers
/// the stroke.
pub fn ring(ring: &[Point], rect: &Rect) -> Vec<Point> {
    let mut points: Vec<Point> = ring.iter().copied().filter(|p| finite(*p)).collect();
    for edge in rect.edges() {
        let Some(&last) = points.last() else {
            break;
        };
        let mut output = Vec::with_capacity(points.len() + 4);
        let mut previous = last;
        for &current in &points {
            match (edge.inside(previous), edge.inside(current)) {
                (true, true) => output.push(current),
                (true, false) => output.push(edge.crossing(previous, current)),
                (false, true) => {
                    output.push(edge.crossing(previous, current));
                    output.push(current);
                }
                (false, false) => {}
            }
            previous = current;
        }
        points = output;
    }
    // Huge coordinates can still overflow an intersection; such points are dropped as well.
    points.retain(|p| finite(*p));
    if points.len() < 3 { Vec::new() } else { points }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    /// The image of `width` × `height` pixels itself, not grown by the margin.
    fn image(width: u32, height: u32) -> Rect {
        Rect {
            min_x: 0.0,
            min_y: 0.0,
            max_x: f64::from(width),
            max_y: f64::from(height),
        }
    }

    fn inside(points: &[Point], r: &Rect) -> bool {
        points
            .iter()
            .all(|q| q.x >= r.min_x && q.x <= r.max_x && q.y >= r.min_y && q.y <= r.max_y)
    }

    #[test]
    fn a_ring_far_outside_is_cut_to_the_rectangle() {
        // Verifies: ADR-0015
        let r = Rect::around_image(64, 64, 0.0);
        let ring = ring(&[p(-1e15, -1e15), p(1e15, 5.0), p(30.0, 1e15)], &r);
        assert!(ring.len() >= 3);
        assert!(inside(&ring, &r), "{ring:?}");
    }

    #[test]
    fn coordinates_near_the_limit_of_f64_never_yield_non_finite_points() {
        // Verifies: ADR-0015
        let r = Rect::around_image(64, 64, 0.0);
        for big in [1e200, 1e300, f64::MAX] {
            let ring = ring(&[p(-big, -big), p(big, 5.0), p(30.0, big)], &r);
            assert!(inside(&ring, &r), "{big}: {ring:?}");
            for run in line(&[p(-big, 1.0), p(big, 50.0), p(3.0, -big)], &r) {
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
        for (points, r) in [
            (
                [
                    p(4.2356607143436245e307, -4.2356607143436245e307),
                    p(1.1167177796903676e90, -7.319297294523442e36),
                ],
                Rect::around_image(64, 3, 16.0),
            ),
            (
                [
                    p(-6.892022847183044e223, -1.3192564472220833e206),
                    p(-220661571.2063783, 5.140838393045592e63),
                ],
                Rect::around_image(96, 3, 0.0),
            ),
        ] {
            assert_eq!(line(&points, &r), Vec::<Vec<Point>>::new(), "{points:?}");
            let ring = ring(&[points[0], points[1], p(1.0, 1.0)], &r);
            assert!(inside(&ring, &r), "{points:?}: {ring:?}");
        }
    }

    #[test]
    fn a_segment_from_far_out_crosses_where_it_crosses() {
        // Verifies: ADR-0015
        let r = Rect::around_image(64, 64, 0.0);
        assert_eq!(
            line(&[p(-1e19, 30.0), p(100.0, 30.0)], &r),
            vec![vec![p(-2.0, 30.0), p(66.0, 30.0)]]
        );
        assert_eq!(
            line(&[p(-1e19, -1e19), p(100.0, 100.0)], &r),
            vec![vec![p(-2.0, -2.0), p(66.0, 66.0)]]
        );
        // Ends 1e17 to 1e19 out, where every t of a crossing rounds to 1.0 or its neighbour and
        // only u ranks the edges; the expected crossings are the exact ones, rounded.
        let near = |q: Point, e: Point| (q.x - e.x).abs() < 1e-9 && (q.y - e.y).abs() < 1e-9;
        for (points, r, expected) in [
            (
                [
                    p(2.724462101651667e18, -2.7043933580012513e18),
                    p(-139.76631398624681, 284.4106544082265),
                ],
                Rect::around_image(102, 154, 16.0),
                [p(120.0, 26.5578130157966), p(-18.0, 163.541286993255)],
            ),
            (
                [
                    p(-2.3760015071543967e18, 2.9094076710915876e18),
                    p(246.6954742644109, 29.93848751246685),
                ],
                Rect::around_image(154, 166, 98.0),
                [
                    p(53.91310312249169, 266.0),
                    p(246.6954742644109, 29.93848751246685),
                ],
            ),
        ] {
            let runs = line(&points, &r);
            assert!(
                runs.len() == 1
                    && runs[0].len() == 2
                    && near(runs[0][0], expected[0])
                    && near(runs[0][1], expected[1]),
                "{points:?}: {runs:?}"
            );
        }
        // The exact crossings of the triangle's two edges from its far vertex are (-2, 8) and
        // (48, -2).
        let ring = ring(&[p(-1e300, -1e300), p(50.0, 60.0), p(60.0, 10.0)], &r);
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
        let r = image(64, 64);
        let square = [p(10.0, 10.0), p(20.0, 10.0), p(20.0, 20.0), p(10.0, 20.0)];
        assert_eq!(ring(&square, &r).len(), 4);
        let outside = [p(100.0, 100.0), p(200.0, 100.0), p(200.0, 200.0)];
        assert!(ring(&outside, &r).is_empty());
    }

    #[test]
    fn non_finite_points_are_dropped() {
        let r = image(64, 64);
        let ring = ring(
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
        let r = image(10, 10);
        let runs = line(&[p(2.0, 2.0), p(20.0, 2.0), p(20.0, 8.0), p(2.0, 8.0)], &r);
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
        let r = image(10, 10);
        let points = [p(1.0, 1.0), p(5.0, 5.0), p(9.0, 1.0)];
        assert_eq!(line(&points, &r), vec![points.to_vec()]);
    }
}
