//! The clip of every path to the image grown by a margin, and the reach of a glyph past the image
//! (ARCH-one-clip-to-image): properties of the display list, which the bounds of a hatch measure
//! with and every renderer clips with, so that a bound and a renderer measure the same part of a
//! path. They keep the coordinates a renderer is handed within agg-rust's fixed-point limits
//! (REN-agg-rust-rasterizer).

use crate::{PathCommand, Point};

/// Pixels beyond the image that a clipped path may reach, besides the reach of its stroke.
const CLIP_MARGIN: f64 = 2.0;

// Guard: agg-rust 1.2.0
/// How far, in pixels, a point of a glyph path may lie outside the image: agg-rust holds
/// coordinates in 24.8 fixed point, which a farther point would overflow. The text path leaves out
/// a glyph that reaches farther, and a renderer draws no path that does.
///
/// A refusal (ARCH-third-party-failures-guarded), with [`within_glyph_reach`], read from agg-rust
/// 1.2.0: the scanline rasterizer takes a point into 24.8 fixed point by `iround`, which saturates
/// at `i32` (`rasterizer_sl_clip.rs:20-22`, `basics.rs:15-21`), and its clip box subtracts such
/// points in `i32` (`rasterizer_sl_clip.rs:128`), which overflows. At 2^22 pixels a point is 2^30
/// subpixels, so the difference of two points, the image's and the reach's, stays within `i32`.
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

// Guard: agg-rust 1.2.0
/// The width a stroke of `width` is drawn at on an image of `image_width` × `image_height`
/// pixels, its magnitude bounded: a wider stroke covers the image all the same, and the margin
/// stays small enough for agg-rust's fixed-point coordinates. A caller that draws a negative
/// width keeps its sign itself.
///
/// An exact guard (ARCH-third-party-failures-guarded), read from agg-rust 1.2.0: the clip's margin
/// grows with the width, and past some 8.4e6 pixels `iround` saturates a point's 24.8 fixed point
/// at `i32` (`rasterizer_sl_clip.rs:20-22`, `basics.rs:15-21`). Short of that,
/// `RasterizerCellsAa::line` walks a cell for every pixel a line crosses
/// (`rasterizer_cells_aa.rs:348-358`), so a stroke a trillion pixels wide, unbounded, runs past two
/// minutes in a debug build.
pub fn drawn_width(width: f64, image_width: u32, image_height: u32) -> f64 {
    width
        .abs()
        .min(2.0 * (f64::from(image_width) + f64::from(image_height)))
}

// Guard: agg-rust 1.2.0
/// An axis-aligned rectangle in pixels: outside this module, only the image grown by the margin,
/// [`Rect::around_image`], so that no clip skips the margin.
///
/// The clip of every path to it, by [`line`], [`ring`], and [`segment`], is a guard
/// (ARCH-one-clip-to-image, ARCH-third-party-failures-guarded), read from agg-rust 1.2.0: the
/// scanline rasterizer takes a point into 24.8 fixed point by `iround`, which saturates at `i32`
/// (`rasterizer_sl_clip.rs:20-22`, `basics.rs:15-21`), and `RasterizerCellsAa::line` walks a cell
/// for every pixel a line crosses (`rasterizer_cells_aa.rs:348-358`), so a fill reaching ten
/// million pixels out runs for more than 40 seconds unclipped in a debug build. The clip moves no
/// pixel of a path it keeps. It refuses a point that is no finite number, a segment whose
/// coordinate difference overflows, and a crossing that is not finite.
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

/// The part of the segment from `a` to `b` inside the rectangle, by Liang–Barsky; `None` where the
/// segment misses it, or where an end or a crossing is not finite. It sets `dropped` where it drops
/// the segment for an end, a difference, or a crossing that is not finite, so that a renderer
/// reports the refusal (ARCH-third-party-failures-guarded); a segment that misses the rectangle
/// leaves it as it is.
pub fn segment(a: Point, b: Point, r: &Rect, dropped: &mut bool) -> Option<(Point, Point)> {
    if !((b.x - a.x).is_finite() && (b.y - a.y).is_finite()) {
        // The difference of two huge coordinates overflows; such a segment is dropped.
        *dropped = true;
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
    let kept = finite(start) && finite(end);
    *dropped |= !kept;
    kept.then_some((start, end))
}

fn finite(p: Point) -> bool {
    p.x.is_finite() && p.y.is_finite()
}

/// Clips an open polyline with Liang–Barsky, segment by segment, into the runs inside; points that
/// are not finite are dropped. It sets `dropped` where it drops a point, or a segment as
/// [`segment`] does.
pub fn line(line: &[Point], rect: &Rect, dropped: &mut bool) -> Vec<Vec<Point>> {
    let points: Vec<Point> = line.iter().copied().filter(|p| finite(*p)).collect();
    *dropped |= points.len() < line.len();
    let mut runs: Vec<Vec<Point>> = Vec::new();
    let mut run: Vec<Point> = Vec::new();
    for pair in points.windows(2) {
        match segment(pair[0], pair[1], rect, dropped) {
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
/// the stroke. It sets `dropped` where it drops a point, or a crossing that is not finite.
pub fn ring(ring: &[Point], rect: &Rect, dropped: &mut bool) -> Vec<Point> {
    let mut points: Vec<Point> = ring.iter().copied().filter(|p| finite(*p)).collect();
    *dropped |= points.len() < ring.len();
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
    let crossed = points.len();
    points.retain(|p| finite(*p));
    *dropped |= points.len() < crossed;
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
        // Verifies: REN-agg-rust-rasterizer
        let r = Rect::around_image(64, 64, 0.0);
        let ring = ring(
            &[p(-1e15, -1e15), p(1e15, 5.0), p(30.0, 1e15)],
            &r,
            &mut false,
        );
        assert!(ring.len() >= 3);
        assert!(inside(&ring, &r), "{ring:?}");
    }

    #[test]
    fn coordinates_near_the_limit_of_f64_never_yield_non_finite_points() {
        // Verifies: REN-agg-rust-rasterizer
        let r = Rect::around_image(64, 64, 0.0);
        for big in [1e200, 1e300, f64::MAX] {
            let ring = ring(&[p(-big, -big), p(big, 5.0), p(30.0, big)], &r, &mut false);
            assert!(inside(&ring, &r), "{big}: {ring:?}");
            for run in line(&[p(-big, 1.0), p(big, 50.0), p(3.0, -big)], &r, &mut false) {
                assert!(inside(&run, &r), "{big}: {run:?}");
            }
        }
    }

    #[test]
    fn a_segment_that_misses_the_rectangle_far_out_yields_nothing() {
        // Verifies: REN-agg-rust-rasterizer, Q-3
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
            assert_eq!(
                line(&points, &r, &mut false),
                Vec::<Vec<Point>>::new(),
                "{points:?}"
            );
            let ring = ring(&[points[0], points[1], p(1.0, 1.0)], &r, &mut false);
            assert!(inside(&ring, &r), "{points:?}: {ring:?}");
        }
    }

    /// What each clip says it dropped of its path, by `clip`, with `dropped` unset before.
    fn dropped(clip: impl FnOnce(&mut bool)) -> bool {
        let mut dropped = false;
        clip(&mut dropped);
        dropped
    }

    #[test]
    fn a_clip_says_what_it_drops_and_not_what_lies_outside() {
        // Verifies: ARCH-one-clip-to-image, ARCH-third-party-failures-guarded
        let r = Rect::around_image(64, 64, 0.0);
        let crossing = [p(-10.0, 30.0), p(100.0, 30.0), p(30.0, 100.0)];
        assert!(!dropped(|d| _ = line(&crossing, &r, d)));
        assert!(!dropped(|d| _ = ring(&crossing, &r, d)));
        assert!(!dropped(
            |d| _ = segment(p(-10.0, -10.0), p(-5.0, 90.0), &r, d)
        ));
        // A point that is no finite number.
        for far in [p(f64::NAN, 30.0), p(30.0, f64::INFINITY)] {
            let points = [p(10.0, 10.0), far, p(50.0, 10.0)];
            assert!(dropped(|d| _ = line(&points, &r, d)), "{far:?}");
            assert!(dropped(|d| _ = ring(&points, &r, d)), "{far:?}");
            assert!(dropped(|d| _ = segment(points[0], far, &r, d)), "{far:?}");
        }
        // A coordinate difference that overflows, of two finite points.
        let (a, b) = (p(1e308, 30.0), p(-1e308, 30.0));
        assert!(dropped(|d| _ = segment(a, b, &r, d)));
        assert!(dropped(|d| _ = line(&[a, b], &r, d)));
        // A crossing that is not finite: the triangle's diagonal, from corner to corner of
        // `f64`, crosses the left edge at a height no finite number gives, and the triangle,
        // which covers the image's half above the diagonal, is dropped whole.
        let max = 1.7e308;
        let wide = [p(-max, -max), p(-max, -3.0), p(max, max)];
        let mut said = false;
        let kept = ring(&wide, &r, &mut said);
        assert!(said, "{kept:?}");
        assert_eq!(kept, Vec::<Point>::new());
    }

    #[test]
    fn a_segment_from_far_out_crosses_where_it_crosses() {
        // Verifies: REN-agg-rust-rasterizer
        let r = Rect::around_image(64, 64, 0.0);
        assert_eq!(
            line(&[p(-1e19, 30.0), p(100.0, 30.0)], &r, &mut false),
            vec![vec![p(-2.0, 30.0), p(66.0, 30.0)]]
        );
        assert_eq!(
            line(&[p(-1e19, -1e19), p(100.0, 100.0)], &r, &mut false),
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
            let runs = line(&points, &r, &mut false);
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
        let ring = ring(
            &[p(-1e300, -1e300), p(50.0, 60.0), p(60.0, 10.0)],
            &r,
            &mut false,
        );
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
        assert_eq!(ring(&square, &r, &mut false).len(), 4);
        let outside = [p(100.0, 100.0), p(200.0, 100.0), p(200.0, 200.0)];
        assert!(ring(&outside, &r, &mut false).is_empty());
    }

    #[test]
    fn the_width_drawn_is_the_magnitude_bounded_by_twice_the_image_s_width_and_height() {
        // Verifies: REN-agg-rust-rasterizer
        assert_eq!(drawn_width(3.0, 100, 100), 3.0);
        assert_eq!(drawn_width(-3.0, 100, 100), 3.0);
        assert_eq!(drawn_width(1e7, 100, 100), 400.0);
        assert_eq!(drawn_width(-1e7, 100, 100), 400.0);
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
            &mut false,
        );
        assert_eq!(ring, vec![p(10.0, 10.0), p(20.0, 10.0), p(15.0, 20.0)]);
    }

    #[test]
    fn a_line_leaving_and_reentering_splits_into_runs() {
        let r = image(10, 10);
        let runs = line(
            &[p(2.0, 2.0), p(20.0, 2.0), p(20.0, 8.0), p(2.0, 8.0)],
            &r,
            &mut false,
        );
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
        assert_eq!(line(&points, &r, &mut false), vec![points.to_vec()]);
    }

    #[test]
    fn a_path_within_the_image_grown_by_its_margin_is_handed_on_as_it_is() {
        // Verifies: ARCH-third-party-failures-guarded
        // The ordinary input beside the clip's guard: a line and a ring on the image and in its
        // margin of 2 pixels are kept point for point, so the renderer draws them as unclipped.
        let r = Rect::around_image(64, 64, 0.0);
        let line_points = [p(-2.0, 10.0), p(30.5, 0.25), p(66.0, 66.0)];
        assert_eq!(
            line(&line_points, &r, &mut false),
            vec![line_points.to_vec()]
        );
        let ring_points = [p(-2.0, -2.0), p(66.0, -2.0), p(40.0, 50.5), p(3.0, 66.0)];
        assert_eq!(ring(&ring_points, &r, &mut false), ring_points.to_vec());
    }

    #[test]
    fn a_width_within_the_bound_is_drawn_as_it_is() {
        // Verifies: ARCH-third-party-failures-guarded
        // The ordinary input beside drawn_width's guard: up to twice the image's width and
        // height, 400 pixels on an image of 100 by 100, a width is drawn as given.
        for width in [0.25, 3.0, 399.5, 400.0] {
            assert_eq!(drawn_width(width, 100, 100), width);
        }
    }

    #[test]
    fn a_glyph_within_its_reach_is_kept_and_one_a_pixel_past_it_is_not() {
        // Verifies: ARCH-third-party-failures-guarded
        // The ordinary input beside the glyph's reach: a point 4,194,304 pixels, 2^22, off the
        // image is within it, one a pixel farther is not.
        let image = [0.0, 0.0, 64.0, 32.0];
        let at = |x: f64, y: f64| {
            [
                PathCommand::MoveTo(p(10.0, 10.0)),
                PathCommand::LineTo(p(x, y)),
            ]
        };
        assert!(within_glyph_reach(image, &at(-4_194_304.0, 10.0)));
        assert!(within_glyph_reach(image, &at(10.0, 32.0 + 4_194_304.0)));
        assert!(!within_glyph_reach(image, &at(-4_194_305.0, 10.0)));
        assert!(!within_glyph_reach(image, &at(10.0, 32.0 + 4_194_305.0)));
    }
}
