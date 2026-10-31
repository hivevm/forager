//! The clip of a segment to the image enlarged by a margin (ADR-0015), one for the bounds of a
//! hatch and for the renderer that draws it, so that both measure the same part of a line.

use crate::Point;

/// Pixels beyond the image that a clipped path may reach, besides the reach of its stroke.
pub const CLIP_MARGIN: f64 = 2.0;

/// The width a stroke of `width` is drawn at on an image of `image_width` × `image_height`
/// pixels: a wider stroke covers the image all the same, and the margin stays small enough for
/// agg-rust's fixed-point coordinates.
pub fn drawn_width(width: f64, image_width: u32, image_height: u32) -> f64 {
    width.min(2.0 * (f64::from(image_width) + f64::from(image_height)))
}

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

/// The coordinate a line of a rectangle's edge fixes.
#[derive(Clone, Copy, Debug)]
pub enum Axis {
    X,
    Y,
}

/// The point where the segment from `a` to `b` crosses the line on which the coordinate `axis`
/// is `value`. That coordinate is set exactly and the other is interpolated from the end nearer to
/// the line: from the far end, `a + t * (b - a)` cancels where one end lies orders of magnitude
/// farther out than the other, and the crossing would land pixels or the whole image away.
pub fn crossing(a: Point, b: Point, axis: Axis, value: f64) -> Point {
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
