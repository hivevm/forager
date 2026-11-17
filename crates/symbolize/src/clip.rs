// The clip of a polygon or a line before it is drawn follows msDrawShape(), src/mapdraw.c,
// msClipPolygonRect(), msClipPolylineRect(), and clipLine(), src/mapprimitive.cpp, of the release
// pinned in scripts/fetch-msautotest.sh.
// Copyright (c) 1996-2005 Regents of the University of Minnesota.
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

//! msDrawShape()'s clip: the class's clip buffers, and the clip of a hatched polygon before it is
//! hatched and of a line shape before it is stroked, by those buffers, beyond the map's extent.

use forager_render::Point;
use forager_style_ir::c::{lrint, ms_max, ms_nint};
use forager_style_ir::{Coord, Style, Symbol};

use crate::bounds;
use crate::extent::View;
use crate::style::fixed;
use crate::symbol::{self, Factors, Pixmaps};

/// How far beyond the image MapServer clips a class's polygons and lines before it draws them,
/// and where.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Clip {
    /// msDrawShape()'s `clip_buf` of the class for a polygon, in pixels.
    pub buffer: f64,
    /// msDrawShape()'s `clip_buf` of the class for a line, in pixels: without the styles'
    /// offsets and the 2 a polygon adds.
    pub line_buffer: f64,
    /// Whether a style of the class has a `GEOMTRANSFORM`, which makes MapServer clip a polygon
    /// and a line in pixels, after the transformation to the image, instead of in map units
    /// before it.
    pub in_pixels: bool,
}

impl Clip {
    /// msDrawShape()'s clip buffers of a polygon and of a line drawn in the styles of a class at
    /// `factors`, the styles' scale factor and the image's resolution factor, which are one here:
    /// per style, the largest of its symbol's default size, its `SIZE`, and its `WIDTH`, for a
    /// polygon grown by its larger `OFFSET` unless that is a line offset, scaled, or its `MINSIZE`
    /// or `MINWIDTH` scaled where larger, plus 1, rounded as `lrint`; the largest of the styles,
    /// plus 2 for a polygon. A size
    /// or width bound to a property or an expression counts as unset, MapServer's -1 and 1;
    /// MapServer computes the buffer before msBindLayerToShape() binds the feature's values, so it
    /// counts the values bound for the feature drawn before. The default size of a pixmap symbol
    /// is its image's height, the image among `pixmaps`; of one not among them, and of an SVG
    /// symbol, 1.
    pub fn for_class(
        styles: &[Style],
        symbols: &[Symbol],
        pixmaps: Option<&Pixmaps>,
        Factors {
            scale: factor,
            resolution,
        }: Factors,
    ) -> Self {
        // `int clip_buf`, src/mapdraw.c:2463, of a polygon and of a line.
        let (mut polygon, mut line) = (0_i32, 0_i32);
        for style in styles {
            let default_size = symbol::default_size_of(style, symbols, pixmaps);
            let size = fixed(&style.size).unwrap_or(-1.0);
            let width = fixed(&style.width).unwrap_or(1.0);
            let largest = ms_max(default_size, ms_max(size, width));
            let unscaled = bounds::clip_buffer_minimum(style, resolution);
            let mut offset = 0.0;
            if style.line_offset.is_none()
                && let Some([x, y]) = &style.offset
            {
                let (x, y) = (x.value().unwrap_or(0.0), y.value().unwrap_or(0.0));
                offset = ms_max(y.abs(), x.abs());
            }
            // MS_MAX of the `int` and MS_NINT's `long`, compared as a `long` and narrowed into the
            // `int` (2497-2499).
            let grow = |buffer: &mut i32, largest: f64| {
                let nint = ms_max(largest * factor, unscaled) + 1.0;
                if i64::from(*buffer) <= lrint(nint) {
                    *buffer = ms_nint(nint);
                }
            };
            grow(&mut polygon, largest + offset);
            grow(&mut line, largest);
        }
        Self {
            buffer: f64::from(polygon) + 2.0,
            line_buffer: f64::from(line),
            in_pixels: styles.iter().any(|s| s.geom_transform.is_some()),
        }
    }
}

/// MapServer's `NEARZERO`, `src/mapprimitive.cpp`.
const NEARZERO: f64 = 1.0e-30;

/// msClipPolygonRect(): every ring cut to the rectangle from `min` to `max` by MapServer's
/// variant of Liang–Barsky, which keeps a ring's way around a corner it cuts off as that corner;
/// rings wholly inside, by the bounds of all of them, are kept as they are. A ring whose last
/// point is not its first is closed with its first point appended before it is cut, where
/// MapServer, whose rings are closed, cuts only the edges written; one wholly outside is
/// dropped.
pub(crate) fn clip_polygon_rect(rings: &[Vec<Point>], min: Point, max: Point) -> Vec<Vec<Point>> {
    let all = rings.iter().flatten();
    let inside = all
        .clone()
        .all(|p| p.x <= max.x && p.x >= min.x && p.y <= max.y && p.y >= min.y);
    if inside {
        return rings.to_vec();
    }
    let mut clipped = Vec::new();
    for ring in rings {
        let mut ring = ring.clone();
        if let (Some(&first), Some(&last)) = (ring.first(), ring.last())
            && first != last
        {
            ring.push(first);
        }
        let mut line: Vec<Point> = Vec::new();
        for edge in ring.windows(2) {
            clip_edge(&mut line, edge[0], edge[1], min, max);
        }
        if let Some(&first) = line.first() {
            line.push(first);
            clipped.push(line);
        }
    }
    clipped
}

/// One edge of msClipPolygonRect(), the points it adds to the clipped ring.
fn clip_edge(line: &mut Vec<Point>, a: Point, b: Point, min: Point, max: Point) {
    let p = |x, y| Point { x, y };
    let (x1, y1, x2, y2) = (a.x, a.y, b.x, b.y);
    let mut dx = x2 - x1;
    if dx == 0.0 {
        dx = if x1 > min.x { -NEARZERO } else { NEARZERO };
    }
    let mut dy = y2 - y1;
    if dy == 0.0 {
        dy = if y1 > min.y { -NEARZERO } else { NEARZERO };
    }
    let (x_in, x_out) = if dx > 0.0 {
        (min.x, max.x)
    } else {
        (max.x, min.x)
    };
    let (y_in, y_out) = if dy > 0.0 {
        (min.y, max.y)
    } else {
        (max.y, min.y)
    };
    let t_in_x = (x_in - x1) / dx;
    let t_in_y = (y_in - y1) / dy;
    let (t_in1, t_in2) = if t_in_x < t_in_y {
        (t_in_x, t_in_y)
    } else {
        (t_in_y, t_in_x)
    };
    // The comparisons as MapServer writes them, so that a NaN takes the same way.
    if 1.0 >= t_in1 {
        if 0.0 < t_in1 {
            line.push(p(x_in, y_in));
        }
        if 1.0 >= t_in2 {
            let t_out_x = (x_out - x1) / dx;
            let t_out_y = (y_out - y1) / dy;
            let t_out = if t_out_x < t_out_y { t_out_x } else { t_out_y };
            if 0.0 < t_in2 || 0.0 < t_out {
                if t_in2 <= t_out {
                    if 0.0 < t_in2 {
                        if t_in_x > t_in_y {
                            line.push(p(x_in, y1 + t_in_x * dy));
                        } else {
                            line.push(p(x1 + t_in_y * dx, y_in));
                        }
                    }
                    if 1.0 > t_out {
                        if t_out_x < t_out_y {
                            line.push(p(x_out, y1 + t_out_x * dy));
                        } else {
                            line.push(p(x1 + t_out_y * dx, y_out));
                        }
                    } else {
                        line.push(p(x2, y2));
                    }
                } else if t_in_x > t_in_y {
                    line.push(p(x_in, y_out));
                } else {
                    line.push(p(x_out, y_in));
                }
            }
        }
    }
}

/// The rings of a feature in pixels as msDrawShape() hands them to a hatch: all of its polygons
/// as one shape, transformed to pixels as msTransformShapeSimplify() transforms a polygon's rings
/// (see [`View::ring_to_pixels`]), and, where the shape reaches beyond the map's extent, clipped
/// first to the extent grown by the class's clip buffer (see [`Clip::for_class`]) with
/// msClipPolygonRect(); in map units before the transformation to pixels, or, where a style of
/// the class has a `GEOMTRANSFORM`, to the image so grown after it. The hatch's box, and so where
/// its dashes fall, is the clipped shape's.
pub fn hatch_rings(
    rings: &[Vec<Coord>],
    view: &View,
    image: (f64, f64),
    clip: &Clip,
) -> Vec<Vec<Point>> {
    shape(
        rings,
        view,
        image,
        Some(clip.buffer),
        clip.in_pixels,
        clip_polygon_rect,
        View::ring_to_pixels,
    )
}

/// The lines of a line shape in pixels as msDrawShape() hands them to msDrawLineSymbol(): all of
/// its `parts` as one shape, transformed to pixels as msTransformShapeSimplify() transforms a
/// line (see [`View::line_to_pixels`]), and, where the shape reaches beyond the map's extent and
/// `clipped` is set, clipped first to the extent grown by the class's line clip buffer (see
/// [`Clip::for_class`]) with msClipPolylineRect(); in map units before the transformation to
/// pixels, or, where a style of the class has a `GEOMTRANSFORM`, to the image so grown after it.
/// A line's dashes and its symbols count from where the clip leaves it to begin. A layer of
/// `PROCESSING "POLYLINE_NO_CLIP"` is drawn with `clipped` unset
/// (`src/mapdraw.c:1341-1344, 2452`).
pub fn line_paths(
    parts: &[Vec<Coord>],
    view: &View,
    image: (f64, f64),
    clip: &Clip,
    clipped: bool,
) -> Vec<Vec<Point>> {
    shape(
        parts,
        view,
        image,
        clipped.then_some(clip.line_buffer),
        clip.in_pixels,
        clip_polyline_rect,
        View::line_to_pixels,
    )
}

/// A rectangle's clip of a polygon's rings or of a line's parts, from `min` to `max`.
type Clipper = fn(&[Vec<Point>], Point, Point) -> Vec<Vec<Point>>;

/// msDrawShape()'s clip of a shape of `parts`: each part transformed to pixels by `transform`,
/// a part it leaves empty dropped, and, where the shape reaches beyond the map's extent and a
/// `buffer` is given, cut by `clipper` first to the extent grown by `buffer` pixels; in map units
/// before the transformation, or, `in_pixels`, to the image of `image` width and height so grown
/// after it.
fn shape(
    parts: &[Vec<Coord>],
    view: &View,
    image: (f64, f64),
    buffer: Option<f64>,
    in_pixels: bool,
    clipper: Clipper,
    transform: fn(&View, &[Coord]) -> Vec<Point>,
) -> Vec<Vec<Point>> {
    let e = view.extent;
    let beyond = parts
        .iter()
        .flatten()
        .any(|c| c.x < e.min_x || c.y < e.min_y || c.x > e.max_x || c.y > e.max_y);
    let to_pixels = |parts: &[Vec<Point>]| -> Vec<Vec<Point>> {
        parts
            .iter()
            .map(|part| {
                let part: Vec<Coord> = part.iter().map(|p| Coord { x: p.x, y: p.y }).collect();
                transform(view, &part)
            })
            .filter(|part| !part.is_empty())
            .collect()
    };
    let points: Vec<Vec<Point>> = parts
        .iter()
        .map(|part| part.iter().map(|c| Point { x: c.x, y: c.y }).collect())
        .collect();
    let Some(buffer) = buffer.filter(|_| beyond) else {
        return to_pixels(&points);
    };
    if in_pixels {
        let (width, height) = image;
        let min = Point {
            x: -buffer,
            y: -buffer,
        };
        let max = Point {
            x: width + buffer,
            y: height + buffer,
        };
        return clipper(&to_pixels(&points), min, max);
    }
    let grow = buffer * view.cellsize;
    let min = Point {
        x: e.min_x - grow,
        y: e.min_y - grow,
    };
    let max = Point {
        x: e.max_x + grow,
        y: e.max_y + grow,
    };
    to_pixels(&clipper(&points, min, max))
}

/// msClipPolylineRect(): every line cut to the rectangle from `min` to `max`, segment by segment
/// with clipLine(), a line that leaves the rectangle ended where it leaves and one that comes back
/// begun where it enters; lines wholly inside, by the bounds of all of them, are kept as they are.
pub(crate) fn clip_polyline_rect(lines: &[Vec<Point>], min: Point, max: Point) -> Vec<Vec<Point>> {
    let inside = lines
        .iter()
        .flatten()
        .all(|p| p.x <= max.x && p.x >= min.x && p.y <= max.y && p.y >= min.y);
    if inside {
        return lines.to_vec();
    }
    let mut clipped = Vec::new();
    for points in lines {
        let Some(&first) = points.first() else {
            continue;
        };
        let mut line: Vec<Point> = Vec::new();
        let mut a = first;
        for &b in &points[1..] {
            if let Some((a1, b1)) = clip_line(a, b, min, max) {
                if line.is_empty() {
                    line.push(a1);
                }
                line.push(b1);
                if b1 != b {
                    clipped.push(std::mem::take(&mut line));
                }
            }
            a = b;
        }
        if !line.is_empty() {
            clipped.push(line);
        }
    }
    clipped
}

/// clipLine(): the segment from `a` to `b` cut to the rectangle from `min` to `max`, first in x,
/// then in y, by the slope of what the cut in x leaves; `None` where it lies wholly to one side.
fn clip_line(mut a: Point, mut b: Point, min: Point, max: Point) -> Option<(Point, Point)> {
    // CLIP_CHECK: -1 left of the range, 1 right of it, 0 within.
    let check = |lo: f64, v: f64, hi: f64| {
        if v < lo {
            -1
        } else if v > hi {
            1
        } else {
            0
        }
    };
    if (a.x < min.x && b.x < min.x) || (a.x > max.x && b.x > max.x) {
        return None;
    }
    let (c1, c2) = (check(min.x, a.x, max.x), check(min.x, b.x, max.x));
    for (side, edge) in [(-1, min.x), (1, max.x)] {
        if c1 == side || c2 == side {
            let slope = (b.y - a.y) / (b.x - a.x);
            let y = a.y + (edge - a.x) * slope;
            let end = if c1 == side { &mut a } else { &mut b };
            *end = Point { x: edge, y };
        }
    }
    if (a.y < min.y && b.y < min.y) || (a.y > max.y && b.y > max.y) {
        return None;
    }
    let (c1, c2) = (check(min.y, a.y, max.y), check(min.y, b.y, max.y));
    for (side, edge) in [(-1, min.y), (1, max.y)] {
        if c1 == side || c2 == side {
            let slope = (b.x - a.x) / (b.y - a.y);
            let x = a.x + (edge - a.y) * slope;
            let end = if c1 == side { &mut a } else { &mut b };
            *end = Point { x, y: edge };
        }
    }
    Some((a, b))
}

#[cfg(test)]
mod tests;
