// The placement of symbols along a line follows MapServer's msImagePolylineMarkers() and the
// branch of msDrawLineSymbol() that calls it, src/maprendering.c of the release pinned in
// scripts/fetch-msautotest.sh.
// Copyright (c) 1996-2005 Regents of the University of Minnesota.
// Copyright (c) 1996-2011 Regents of the University of Minnesota.
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

//! Symbols along a line: a vector or an ellipse symbol repeated every `GAP` pixels from the
//! `INITIALGAP`, upright for a positive gap and turned with each segment for a negative one, as
//! MapServer places them. The positions of the markers outside the image are not visited one by
//! one but skipped, and the markers along the lines of one layer, with the symbols its
//! `GEOMTRANSFORM`s place, amount to at most [`MAX_MARKER_POINTS`] points (Q-9).

use forager_render::{DrawOp, Point};
use forager_style_ir::{Style, Symbol};

use crate::bounds;
use crate::draw::Canvas;
use crate::symbol;

/// The most points the markers along the lines of one layer, and the symbols a `GEOMTRANSFORM`
/// places on its lines and polygons, amount to, over all its features and styles (Q-9). The
/// markers of a line, or of a transformed shape, that would pass it are not drawn, nor any of the
/// layer's after them, and the render reports it. MapServer places markers without a bound, so that a
/// tiny `GAP` on a long line, or a feature of many small rings, draws them by the billion; a
/// million points are some 10 000 markers of an ellipse of 30 pixels, or 200 000 of a five-point
/// arrow.
pub const MAX_MARKER_POINTS: usize = 1_000_000;

/// What is left to one layer of [`MAX_MARKER_POINTS`].
#[derive(Debug)]
pub(crate) struct MarkerBudget {
    remaining: usize,
    spent: bool,
}

impl MarkerBudget {
    pub(crate) fn new() -> Self {
        Self::of(MAX_MARKER_POINTS)
    }

    /// A budget of `points`.
    pub(crate) fn of(points: usize) -> Self {
        Self {
            remaining: points,
            spent: false,
        }
    }

    /// Whether markers were left out for want of points.
    pub(crate) fn spent(&self) -> bool {
        self.spent
    }

    /// Takes `points` from what is left: false, and spent, where they exceed it or it is spent
    /// already, and then nothing is taken.
    pub(crate) fn take(&mut self, points: usize) -> bool {
        if self.spent || points > self.remaining {
            self.remaining = 0;
            self.spent = true;
            return false;
        }
        self.remaining -= points;
        true
    }
}

/// MapServer's `MS_PI`, `src/mapserver.h`.
const MS_PI: f64 = std::f64::consts::PI;

/// Where msImagePolylineMarkers draws the symbol of one marker, and turned how far.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Marker {
    pub at: Point,
    /// Radians, counter-clockwise.
    pub rotation: f64,
}

/// What msImagePolylineMarkers is given beside the line.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Placement {
    /// The distance between markers, in pixels.
    pub spacing: f64,
    /// The distance to the first marker; a negative one asks for half the spacing.
    pub initial_gap: f64,
    /// Whether each marker turns with its segment.
    pub auto_angle: bool,
    /// The style's rotation, in radians.
    pub rotation: f64,
    /// The symbol's width and height in pixels, at least 1 each.
    pub size: (f64, f64),
    /// The symbol's `ANCHORPOINT`.
    pub anchor: [f64; 2],
    /// The image's width and height in pixels.
    pub image: (f64, f64),
}

/// The markers of `style` with its vector or ellipse `symbol` along `paths`, in pixels already
/// offset, as msDrawLineSymbol draws a symbol with a `GAP` on the canvas's image, with
/// computeSymbolStyle at the canvas's factor: `initial_gap` is the `INITIALGAP` scaled as the
/// width is. The markers' points are taken from the canvas's budget; where they would exceed what
/// is left, none is drawn, and none after them once it is spent.
pub(crate) fn draw(
    ops: &mut Vec<DrawOp>,
    paths: &[Vec<Point>],
    style: &Style,
    symbol: &Symbol,
    initial_gap: f64,
    canvas: &mut Canvas,
) {
    let image = canvas.image;
    let budget = &mut canvas.markers;
    if budget.spent {
        return;
    }
    let s = symbol::symbol_style(style, symbol, canvas.factor);
    // What one marker costs; a symbol that draws nothing has no markers to place.
    let mut probe = Vec::new();
    symbol::render(&mut probe, Point { x: 0.0, y: 0.0 }, symbol, &s);
    if probe.is_empty() {
        return;
    }
    let cost = probe.iter().map(points).sum::<usize>().max(1);
    let gap = style.gap.unwrap_or(0.0);
    let [size_x, size_y] = symbol.extent.unwrap_or([0.0, 0.0]);
    let placement = Placement {
        spacing: if gap < 0.0 { -s.gap } else { s.gap },
        initial_gap,
        auto_angle: gap < 0.0,
        rotation: s.rotation,
        size: (
            bounds::ms_max(1.0, size_x * s.scale),
            bounds::ms_max(1.0, size_y * s.scale),
        ),
        anchor: symbol.anchor_point.unwrap_or([0.5, 0.5]),
        image,
    };
    let Some(markers) = place(paths, &placement, budget.remaining / cost) else {
        budget.remaining = 0;
        budget.spent = true;
        return;
    };
    budget.remaining -= markers.len() * cost;
    let mut turned = s;
    for marker in markers {
        turned.rotation = marker.rotation;
        symbol::render(ops, marker.at, symbol, &turned);
    }
}

/// The points of a drawing operation.
pub(crate) fn points(op: &DrawOp) -> usize {
    match op {
        DrawOp::Stroke { paths, .. } => paths.iter().map(Vec::len).sum(),
        DrawOp::Fill { rings, .. } => rings.iter().map(Vec::len).sum(),
        DrawOp::Hatch(hatch) => {
            hatch.rings.iter().map(Vec::len).sum::<usize>() + 2 * hatch.lines.len()
        }
        DrawOp::Glyphs { .. } => 1,
    }
}

/// msImagePolylineMarkers' placement: along each line, a marker every `spacing` pixels from the
/// initial gap, each segment carrying on the distance the one before left over; a marker whose
/// point lies more than the symbol's size outside the image is not drawn. A line on which no
/// marker falls, without an initial gap, and longer than the symbol is wide, takes one marker at
/// half its length. `None` when more than `limit` markers would be drawn, or when infinitely many
/// would fall on one point, where MapServer would not return.
pub(crate) fn place(paths: &[Vec<Point>], p: &Placement, limit: usize) -> Option<Vec<Marker>> {
    let mut markers = Vec::new();
    for line in paths {
        let mut line_in = false;
        let mut line_length = 0.0;
        let mut current = if p.initial_gap < 0.0 {
            p.spacing / 2.0
        } else {
            p.initial_gap
        };
        for pair in line.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let length = distance(a, b);
            line_length += length;
            if length == 0.0 {
                continue;
            }
            let (rx, ry) = ((b.x - a.x) / length, (b.y - a.y) / length);
            let rotation = turned(p, rx, ry);
            // The markers on this segment, at `current + k · spacing` up to its length.
            let count = count(current, length, p.spacing)?;
            if count > 0.0 {
                line_in = true;
                let (ox, oy) = anchor_offset(p, rotation);
                let origin = Point {
                    x: a.x + ox,
                    y: a.y + oy,
                };
                let (first, last) = visible(origin, rx, ry, current, count, p);
                if last >= first {
                    let span = last - first + 1.0;
                    if span > (limit - markers.len()) as f64 + 4.0 {
                        return None;
                    }
                    for i in 0..span as usize {
                        // The first marker lies at the leftover itself, whatever the spacing.
                        let k = first + i as f64;
                        let distance = if k == 0.0 {
                            current
                        } else {
                            current + k * p.spacing
                        };
                        let at = Point {
                            x: a.x + distance * rx + ox,
                            y: a.y + distance * ry + oy,
                        };
                        if outside(at, p) {
                            continue;
                        }
                        if markers.len() == limit {
                            return None;
                        }
                        markers.push(Marker { at, rotation });
                    }
                }
                current += count * p.spacing;
            }
            current -= length;
        }
        if p.initial_gap < 0.0
            && !line_in
            && line_length > p.size.0
            && let Some(marker) = halfway(line, line_length / 2.0, p)
        {
            if markers.len() == limit {
                return None;
            }
            markers.push(marker);
        }
    }
    Some(markers)
}

/// The length of a segment, as msImagePolylineMarkers computes it.
fn distance(a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    libm::sqrt(dx * dx + dy * dy)
}

/// The rotation of a marker on the segment of direction `(rx, ry)`: the style's, and with a
/// negative gap the segment's angle added, counter-clockwise on the image whose y runs down.
fn turned(p: &Placement, rx: f64, ry: f64) -> f64 {
    if !p.auto_angle {
        return p.rotation;
    }
    let theta = libm::asin(ry);
    let theta = if rx < 0.0 { theta + MS_PI } else { -theta };
    p.rotation + theta
}

/// How many of the distances `current + k · spacing` are no longer than `length`, as the loop
/// of msImagePolylineMarkers counts them: none past the end, one for a spacing that is not a
/// number or infinite, and `None` where the loop would not end.
fn count(current: f64, length: f64, spacing: f64) -> Option<f64> {
    if current > length || current.is_nan() {
        return Some(0.0);
    }
    if spacing.is_nan() || spacing == f64::INFINITY {
        return Some(1.0);
    }
    if spacing <= 0.0 {
        return None;
    }
    let mut n = ((length - current) / spacing).floor() + 1.0;
    if !n.is_finite() {
        return None;
    }
    // The quotient may round across a whole number; the comparisons decide as the loop does,
    // within the steps a rounding can take.
    for _ in 0..2 {
        if n > 1.0 && current + (n - 1.0) * spacing > length {
            n -= 1.0;
        } else if current + n * spacing <= length {
            n += 1.0;
        }
    }
    Some(n)
}

/// The shift of the drawn point that puts the symbol's anchor point on the line, turned with the
/// symbol.
fn anchor_offset(p: &Placement, rotation: f64) -> (f64, f64) {
    let [anchor_x, anchor_y] = p.anchor;
    if anchor_x == 0.5 && anchor_y == 0.5 {
        return (0.0, 0.0);
    }
    let ox = (0.5 - anchor_x) * p.size.0;
    let oy = (0.5 - anchor_y) * p.size.1;
    if rotation == 0.0 {
        return (ox, oy);
    }
    let (sin, cos) = (libm::sin(-rotation), libm::cos(-rotation));
    (ox * cos - oy * sin, ox * sin + oy * cos)
}

/// Whether msImagePolylineMarkers skips the point as outside the map: more than the symbol's
/// size beyond the image.
fn outside(at: Point, p: &Placement) -> bool {
    let (width, height) = p.size;
    (at.x < -width || at.x > p.image.0 + width) || (at.y < -height || at.y > p.image.1 + height)
}

/// The first and the last `k` of the `count` markers whose point may lie inside the bounds
/// [`outside`] tests, one more on either side for rounding; the point of the segment at the
/// distance `t` is `origin + t · (rx, ry)`. All of them where the bounds give no number.
fn visible(origin: Point, rx: f64, ry: f64, current: f64, count: f64, p: &Placement) -> (f64, f64) {
    let all = (0.0, count - 1.0);
    if !p.spacing.is_finite() {
        return all;
    }
    let (width, height) = p.size;
    let slab = |o: f64, r: f64, low: f64, high: f64| -> (f64, f64) {
        if r == 0.0 {
            if o < low || o > high {
                (f64::INFINITY, f64::NEG_INFINITY)
            } else {
                (f64::NEG_INFINITY, f64::INFINITY)
            }
        } else {
            let (t0, t1) = ((low - o) / r, (high - o) / r);
            (t0.min(t1), t0.max(t1))
        }
    };
    let (x0, x1) = slab(origin.x, rx, -width, p.image.0 + width);
    let (y0, y1) = slab(origin.y, ry, -height, p.image.1 + height);
    let (t0, t1) = (x0.max(y0), x1.min(y1));
    if t0.is_nan() || t1.is_nan() {
        return all;
    }
    if t0 > t1 {
        return (0.0, -1.0);
    }
    let first = (((t0 - current) / p.spacing).floor() - 1.0).max(0.0);
    let last = (((t1 - current) / p.spacing).ceil() + 1.0).min(count - 1.0);
    if first.is_nan() || last.is_nan() {
        return all;
    }
    (first, last)
}

/// The one marker of a line on which none fell: at `half` its length, on the segment that
/// passes it, neither anchored nor tested against the image, as msImagePolylineMarkers draws it.
fn halfway(line: &[Point], half: f64, p: &Placement) -> Option<Marker> {
    let mut before = 0.0;
    let mut after = 0.0;
    for pair in line.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let length = distance(a, b);
        after += length;
        if after > half {
            let offset = half - before;
            let (rx, ry) = ((b.x - a.x) / length, (b.y - a.y) / length);
            return Some(Marker {
                at: Point {
                    x: a.x + offset * rx,
                    y: a.y + offset * ry,
                },
                rotation: turned(p, rx, ry),
            });
        }
        before += length;
    }
    None
}

#[cfg(test)]
mod tests;
