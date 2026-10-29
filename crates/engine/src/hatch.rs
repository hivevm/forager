// The hatch of a polygon follows the hatch branch of MapServer's msDrawShadeSymbol(),
// src/maprendering.c, and createHatch() and msHatchPolygon(), src/mapagg.cpp, of the release
// pinned in scripts/fetch-msautotest.sh.
// Copyright (c) 1996-2005 Regents of the University of Minnesota.
// Copyright (c) 1996-2007 Regents of the University of Minnesota (src/mapagg.cpp).
// Copyright (c) 1996-2011 Regents of the University of Minnesota (src/maprendering.c).
// Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
// associated documentation files (the "Software"), to deal in the Software without restriction,
// including without limitation the rights to use, copy, modify, merge, publish, distribute,
// sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions: The above copyright notice and this
// permission notice shall be included in all copies of this Software or works derived from this
// Software. THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED,
// INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE
// AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

//! The hatch of a polygon: the lines MapServer lays over the polygon's bounding box, at the
//! style's spacing, width, and angle, for the renderer to stroke and clip to the polygon. Its
//! trigonometry is libm's, the same code on every target (Q-2).

use forager_render::{Hatch, Point};
use forager_style_ir::{Rgba, Style};

use crate::draw::fixed;
use crate::symbol::{ms_max, ms_min};

/// MapServer's `MS_DEG_TO_RAD`, `src/mapserver.h`.
const DEG_TO_RAD: f64 = 0.017_453_292_519_943_295;

/// MapServer's default `MINSIZE`, `MAXSIZE`, `MINWIDTH`, and `MAXWIDTH` of a style.
const MIN_SIZE: f64 = 0.0;
const MAX_SIZE: f64 = 500.0;
const MIN_WIDTH: f64 = 0.0;
const MAX_WIDTH: f64 = 32.0;

/// The most lines one hatch lays over the image (Q-9): a hatch a pixel apart over the largest
/// image has some 5800.
const MAX_LINES: f64 = 1_048_576.0;

/// What a hatch style draws on a polygon.
#[derive(Debug, PartialEq)]
pub(crate) enum Hatched {
    Drawn(Hatch),
    /// Nothing, as MapServer draws nothing: no line crosses the image, or the width or the
    /// angle is no number to draw with.
    Nothing,
    /// Nothing, where MapServer would not end or the cost is past its bound (Q-9): a spacing
    /// not above 0, a dash pattern whose period is not above 0, or more lines or dashes than
    /// the bounds allow. The engine reports it.
    Refused,
}

/// msDrawShadeSymbol()'s parameters of a hatch at a scale factor of 1: the spacing is the
/// `SIZE`, 1 without one, bounded by `MAXSIZE` and then `MINSIZE`; the width the `WIDTH`, 1
/// without one, bounded by `MAXWIDTH` and then `MINWIDTH`; the pattern scaled by the factor the
/// bounds applied to the width. MapServer divides by the width as written; where that is 0, the
/// lengths are infinite, and AGG draws a line whose first dash is infinite as solid, so the
/// pattern is dropped. A size, width, or angle bound to a property or an expression is drawn as
/// if unset.
pub(crate) fn hatch(
    rings: Vec<Vec<Point>>,
    style: &Style,
    color: Rgba,
    image: (f64, f64),
    reference: Point,
) -> Hatched {
    let written_width = fixed(&style.width).unwrap_or(1.0);
    let width = if written_width <= 0.0 {
        1.0
    } else {
        written_width
    };
    let width = ms_min(width, style.max_width.unwrap_or(MAX_WIDTH));
    let width = ms_max(width, style.min_width.unwrap_or(MIN_WIDTH));
    let size = fixed(&style.size).unwrap_or(-1.0);
    let spacing = if size <= 0.0 { 1.0 } else { size };
    let spacing = ms_min(spacing, style.max_size.unwrap_or(MAX_SIZE));
    let spacing = ms_max(spacing, style.min_size.unwrap_or(MIN_SIZE));
    let pattern = if written_width == 0.0 {
        Vec::new()
    } else {
        style
            .pattern
            .iter()
            .map(|p| p * width / written_width)
            .collect()
    };
    let angle = fixed(&style.angle).unwrap_or(0.0);
    if !(spacing > 0.0 && spacing.is_finite()) {
        return Hatched::Refused;
    }
    if !(width > 0.0 && width.is_finite() && angle.is_finite()) {
        return Hatched::Nothing;
    }
    let Some(lines) = hatch_polygon(&rings, spacing, width, angle, image, reference) else {
        return Hatched::Refused;
    };
    if lines.is_empty() {
        return Hatched::Nothing;
    }
    let hatch = Hatch {
        rings,
        lines,
        color,
        width,
        pattern,
    };
    // The image's size in whole pixels, as the renderer checks the same bound.
    if hatch.is_bounded(image.0 as u32, image.1 as u32) {
        Hatched::Drawn(hatch)
    } else {
        Hatched::Refused
    }
}

/// msHatchPolygon(): the lines of the hatch over the polygon's bounding box grown by the reach of
/// the stroke, in pixels of the image of `image` width and height; `None` past [`MAX_LINES`].
/// The lines are counted from the reference point, the pixel of the map's origin, where
/// msPrepareImage() puts the image's `refpt`.
/// Only the lines that cross the image, grown by the stroke, are laid: the others are MapServer's
/// too but draw nothing, and a polygon far larger than the image would have countless of them.
pub(crate) fn hatch_polygon(
    rings: &[Vec<Point>],
    spacing: f64,
    width: f64,
    angle: f64,
    image: (f64, f64),
    reference: Point,
) -> Option<Vec<[Point; 2]>> {
    let mut points = rings
        .iter()
        .flatten()
        .filter(|p| p.x.is_finite() && p.y.is_finite());
    let Some(first) = points.next() else {
        return Some(Vec::new());
    };
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (first.x, first.y, first.x, first.y);
    for p in points {
        min_x = min_x.min(p.x);
        min_y = min_y.min(p.y);
        max_x = max_x.max(p.x);
        max_y = max_y.max(p.y);
    }
    let grow = width * 0.7072;
    // MapServer's `(int)` of the box's size, plus 1.
    let size_x = (max_x - min_x + grow * 2.0).trunc() + 1.0;
    let size_y = (max_y - min_y + grow * 2.0).trunc() + 1.0;
    let (ox, oy) = (min_x - grow, min_y - grow);
    if !(size_x.is_finite() && size_y.is_finite()) {
        return Some(Vec::new());
    }
    let margin = width + 2.0;
    let visible = Visible {
        min_x: -margin - ox,
        min_y: -margin - oy,
        max_x: image.0 + margin - ox,
        max_y: image.1 + margin - oy,
    };
    let lines = create_hatch(
        [ox, oy],
        reference,
        [size_x, size_y],
        angle,
        spacing,
        &visible,
    )?;
    Some(
        lines
            .into_iter()
            .map(|[a, b]| {
                [
                    Point {
                        x: a.x + ox,
                        y: a.y + oy,
                    },
                    Point {
                        x: b.x + ox,
                        y: b.y + oy,
                    },
                ]
            })
            .collect(),
    )
}

/// The part of the image the lines must cover, in the coordinates of the hatch's box.
struct Visible {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}

/// The values `first + k × step`, k counted up from 0, from the last not above `low` up to the
/// first past `high`, as MapServer reaches them by adding `step` over and over: from `first`
/// itself while that is not below `low`, else from the step just before it. `None` when they are
/// more than [`MAX_LINES`], or where a step no longer moves the value, so far from the image's
/// origin that MapServer's loop would never end.
fn steps(first: f64, step: f64, low: f64, high: f64) -> Option<impl Iterator<Item = f64>> {
    let from = if first < low {
        first + ((low - first) / step).floor() * step
    } else {
        first
    };
    let count = (high - from) / step;
    if count.is_nan() || count > MAX_LINES || from + step == from {
        return None;
    }
    // At most one more than counted, which rounding can add.
    let count = count.max(0.0) as usize + 2;
    Some(
        std::iter::successors(Some(from), move |v| Some(v + step))
            .take(count)
            .take_while(move |v| *v <= high),
    )
}

/// createHatch(): the lines `step` apart at `angle` degrees across a box of `size_x` × `size_y`
/// at the origin, the box's top left corner `(ox, oy)` on the image, each from the side of the
/// box it enters to the side it leaves; the lines through the reference point belong to the
/// hatch. Only those that cross `visible` are laid.
fn create_hatch(
    [ox, oy]: [f64; 2],
    reference: Point,
    [size_x, size_y]: [f64; 2],
    angle: f64,
    step: f64,
    visible: &Visible,
) -> Option<Vec<[Point; 2]>> {
    let (rx, ry) = (reference.x, reference.y);
    let p = |x, y| Point { x, y };
    let mut lines = Vec::new();
    // The angle in [0, 180[.
    let mut angle = angle % 360.0;
    if angle < 0.0 {
        angle += 360.0;
    }
    if angle >= 180.0 {
        angle -= 180.0;
    }

    if angle == 0.0 {
        let mut y0 = step - (oy - ry) % step;
        if oy - ry < 0.0 {
            y0 -= step;
        }
        let high = size_y.min(visible.max_y);
        for y in steps(y0, step, visible.min_y, high)?.take_while(|y| *y < size_y) {
            lines.push([p(0.0, y), p(size_x, y)]);
        }
        return Some(lines);
    }
    if angle == 90.0 {
        let mut x0 = step - (ox - rx) % step;
        if ox - rx < 0.0 {
            x0 -= step;
        }
        let high = size_x.min(visible.max_x);
        for x in steps(x0, step, visible.min_x, high)?.take_while(|x| *x < size_x) {
            lines.push([p(x, 0.0), p(x, size_y)]);
        }
        return Some(lines);
    }

    let theta = (90.0 - angle) * DEG_TO_RAD;
    let ct = libm::cos(theta);
    let mut st = libm::sin(theta);
    let invct = 1.0 / ct;
    let mut invst = 1.0 / st;
    let rmax = (size_x * size_x + size_y * size_y).sqrt();
    // The distance to the line through the reference point.
    let rref = rx * ct + ry * st;
    // The distance to the line through the box's top left corner, or its bottom left one.
    let rcorner = if angle < 90.0 {
        ox * ct + oy * st
    } else {
        ox * ct + (oy + size_y) * st
    };
    let mut r0 = step - (rcorner - rref) % step;
    if rcorner - rref < 0.0 {
        r0 -= step;
    }
    if angle >= 90.0 {
        st = -st;
        invst = -invst;
    }
    // Each line is `r = x cos θ + y sin θ`, y counted from the bottom for an angle of 90° or
    // more; the visible part spans the r of its corners.
    let corner_r = |x: f64, y: f64| {
        let y = if angle < 90.0 { y } else { size_y - y };
        x * ct + y * st
    };
    let corners = [
        corner_r(visible.min_x, visible.min_y),
        corner_r(visible.max_x, visible.min_y),
        corner_r(visible.min_x, visible.max_y),
        corner_r(visible.max_x, visible.max_y),
    ];
    let low = corners.iter().copied().fold(f64::INFINITY, f64::min);
    let high = corners
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max)
        .min(rmax);
    for r in steps(r0, step, low, high)?.take_while(|r| *r < rmax) {
        if let Some(line) = crossing(r, [ct, st, invct, invst], size_x, size_y) {
            lines.push(if angle < 90.0 {
                line
            } else {
                line.map(|q| p(q.x, size_y - q.y))
            });
        }
    }
    Some(lines)
}

/// Where the line `r = x cos θ + y sin θ` enters and leaves the box, its sides tested left,
/// right, then top and bottom while fewer than two meet it; `None` where it does not cross.
fn crossing(
    r: f64,
    [ct, st, invct, invst]: [f64; 4],
    size_x: f64,
    size_y: f64,
) -> Option<[Point; 2]> {
    let mut found: Vec<Point> = Vec::with_capacity(2);
    let y = r * invst;
    if (0.0..=size_y).contains(&y) {
        found.push(Point { x: 0.0, y });
    }
    let y = (r - size_x * ct) * invst;
    if (0.0..=size_y).contains(&y) {
        found.push(Point { x: size_x, y });
    }
    if found.len() < 2 {
        let x = r * invct;
        if (0.0..=size_x).contains(&x) {
            found.push(Point { x, y: 0.0 });
        }
    }
    if found.len() < 2 {
        let x = (r - size_y * st) * invct;
        if (0.0..=size_x).contains(&x) {
            found.push(Point { x, y: size_y });
        }
    }
    match found[..] {
        [a, b] if a != b => Some([a, b]),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
