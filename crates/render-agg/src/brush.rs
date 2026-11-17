// How a line is brushed with a tile follows agg2RenderLineTiled of MapServer's src/mapagg.cpp, at
// the release pinned in scripts/fetch-msautotest.sh; its bilinear filter is a port of
// pattern_filter_bilinear_rgba, and the saturation of its coordinates of line_coord_sat, of the
// AGG 2.4 MapServer bundles (ADR-0046).
// Copyright (c) 1996-2007 Regents of the University of Minnesota.
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
//
// Anti-Grain Geometry - Version 2.4
// Copyright (C) 2002-2005 Maxim Shemanarev (http://www.antigrain.com)
// Permission to copy, use, modify, sell and distribute this software is granted provided this
// copyright notice appears in all copies. This software is provided "as is" without express or
// implied warranty, and with no claim as to its suitability for any purpose.

//! A line brushed with its brush tile, as `agg2RenderLineTiled` brushes it: agg-rust's
//! outline-image rasterizer, a port of AGG 2.4's bilinear pattern filter, integer coordinates
//! saturated as `line_coord_sat` saturates them, the clip of `clip.rs`, and the pattern's start
//! carried across a cut as AGG reaches it.

use agg_rust::color::Rgba8;
use agg_rust::line_aa_basics::{LINE_MAX_COORD, LINE_MAX_LENGTH, LineParameters};
use agg_rust::pattern_filters_rgba::PatternFilter;
use agg_rust::rasterizer_outline_aa::RasterizerOutlineAa;
use agg_rust::renderer_outline_aa::OutlineAaRenderer;
use agg_rust::renderer_outline_image::{
    ImagePatternSource, LineImagePattern, RendererOutlineImage,
};
use forager_render::clip::{self, Rect};
use forager_render::{Point, PremultipliedImage, Tile};
use forager_style_ir::c::lrint;

use crate::pixfmt::PixfmtMapserver;
use crate::{AggRenderer, built_tile};

/// AGG's `line_subpixel_shift` and `line_subpixel_scale`: a line's coordinates are in 1/256
/// pixel.
const SHIFT: u32 = 8;
const SCALE: i32 = 1 << SHIFT;

/// AGG 2.4's `pattern_filter_bilinear_rgba<rgba8>`: each colour the four pixels about the point
/// weighted by its subpixel position, plus half the weights' sum before the shift, so that it
/// rounds where agg-rust's own filter truncates.
pub(crate) struct BilinearRgba8;

impl PatternFilter for BilinearRgba8 {
    fn dilation() -> u32 {
        1
    }

    fn pixel_low_res(buf: &[Vec<Rgba8>], x: i32, y: i32) -> Rgba8 {
        buf[y as usize][x as usize]
    }

    fn pixel_high_res(buf: &[Vec<Rgba8>], p: &mut Rgba8, x: i32, y: i32) {
        let (x_lr, y_lr) = ((x >> SHIFT) as usize, (y >> SHIFT) as usize);
        let (x, y) = ((x & (SCALE - 1)) as u32, (y & (SCALE - 1)) as u32);
        let scale = SCALE as u32;
        let mut sum = [scale * scale / 2; 4];
        let weighted = [
            (buf[y_lr][x_lr], (scale - x) * (scale - y)),
            (buf[y_lr][x_lr + 1], x * (scale - y)),
            (buf[y_lr + 1][x_lr], (scale - x) * y),
            (buf[y_lr + 1][x_lr + 1], x * y),
        ];
        for (c, weight) in weighted {
            for (s, v) in sum.iter_mut().zip([c.r, c.g, c.b, c.a]) {
                *s += weight * u32::from(v);
            }
        }
        let [r, g, b, a] = sum.map(|s| (s >> (SHIFT * 2)) as u8);
        *p = Rgba8 { r, g, b, a };
    }
}

/// A tile's image as the source `line_image_pattern::create` copies: its premultiplied pixels as
/// they are stored, as MapServer hands it the tile's pixel format.
struct Source<'a>(&'a PremultipliedImage);

impl ImagePatternSource for Source<'_> {
    fn width(&self) -> f64 {
        f64::from(self.0.width())
    }

    fn height(&self) -> f64 {
        f64::from(self.0.height())
    }

    fn pixel(&self, x: i32, y: i32) -> Rgba8 {
        let i = (y as usize * self.0.width() as usize + x as usize) * 4;
        let p = &self.0.pixels()[i..i + 4];
        Rgba8 {
            r: p[0],
            g: p[1],
            b: p[2],
            a: p[3],
        }
    }
}

/// `line_coord_sat::conv`: the coordinate in subpixels, held to AGG's `line_max_coord` before it
/// is rounded, as `saturation<line_max_coord>::iround` holds it, and rounded with `lrint`, half
/// to even, as MapServer's AGG rounds.
pub(crate) fn line_coord_sat(x: f64) -> i32 {
    let v = x * f64::from(SCALE);
    let limit = f64::from(LINE_MAX_COORD);
    if v < -limit {
        -LINE_MAX_COORD
    } else if v > limit {
        LINE_MAX_COORD
    } else {
        lrint(v) as i32
    }
}

/// The distance between two vertices in subpixels as AGG's `line_aa_vertex` measures it, the
/// length a segment moves the pattern's start by.
fn distance(a: (i32, i32), b: (i32, i32)) -> i64 {
    let dx = f64::from(b.0) - f64::from(a.0);
    let dy = f64::from(b.1) - f64::from(a.1);
    (dx * dx + dy * dy).sqrt().round() as i64
}

/// The vertices AGG's `vertex_sequence` keeps of `vertices`, by their indices, each with the
/// length of its segment to the next: `add` drops the last vertex kept where it lies 384
/// subpixels or closer to the one before it, and `close` of an open line drops, while the last
/// two lie that close, the one before the last.
fn filtered(vertices: &[(i32, i32)]) -> Vec<(usize, i64)> {
    let near =
        |a: usize, b: usize| distance(vertices[a], vertices[b]) <= i64::from(SCALE + SCALE / 2);
    let mut kept: Vec<usize> = Vec::with_capacity(vertices.len());
    for i in 0..vertices.len() {
        if let [.., before, last] = kept[..]
            && near(before, last)
        {
            kept.pop();
        }
        kept.push(i);
    }
    while let [.., before, last] = kept[..]
        && near(before, last)
    {
        kept.pop();
        kept.pop();
        // `modify_last`: the last takes the place of the one before it, through `add`.
        if let [.., before, previous] = kept[..]
            && near(before, previous)
        {
            kept.pop();
        }
        kept.push(last);
    }
    if kept.len() < 2 {
        return Vec::new();
    }
    let mut segments: Vec<(usize, i64)> = kept
        .windows(2)
        .map(|pair| (pair[0], distance(vertices[pair[0]], vertices[pair[1]])))
        .collect();
    segments.push((kept[kept.len() - 1], 0));
    segments
}

/// AGG's start at each vertex of a path of `vertices`, the path starting at `carried`, and the
/// start the path leaves to the next: each vertex the filter keeps at the lengths of the kept
/// segments before it; one it drops, the start of the vertex kept before it plus the distance to
/// it; the path's first at `carried`.
fn starts(vertices: &[(i32, i32)], carried: i64) -> (Vec<i64>, i64) {
    let kept = filtered(vertices);
    let mut starts = Vec::with_capacity(vertices.len());
    let mut at = carried;
    let mut kept = kept.iter().peekable();
    let mut last_kept = 0;
    for i in 0..vertices.len() {
        if let Some(&&(index, len)) = kept.peek()
            && index == i
        {
            starts.push(at);
            last_kept = i;
            at += len;
            kept.next();
        } else {
            let base = starts.get(last_kept).copied().unwrap_or(carried);
            starts.push(base + distance(vertices[last_kept], vertices[i]));
        }
    }
    let left = starts.get(last_kept).copied().unwrap_or(carried);
    (starts, left)
}

/// A piece of a path that the clip keeps: its points, the index of the path's segment it starts
/// on, and whether its first and last points are cut out of a segment.
#[derive(Debug, PartialEq)]
struct Piece {
    points: Vec<Point>,
    first_segment: usize,
    cut_end: bool,
}

/// The pieces of `points`, its points that are no number dropped, inside `rect`, as
/// [`clip::line`] cuts them, segment by segment through [`clip::segment`], with what the start of
/// each needs.
fn pieces(points: &[Point], rect: &Rect) -> Vec<Piece> {
    let mut pieces: Vec<Piece> = Vec::new();
    let mut run: Option<Piece> = None;
    for (k, pair) in points.windows(2).enumerate() {
        let Some((a, b)) = clip::segment(pair[0], pair[1], rect) else {
            pieces.extend(run.take().filter(|p| p.points.len() > 1));
            continue;
        };
        let continues = run.as_ref().is_some_and(|p| p.points.last() == Some(&a));
        if !continues {
            pieces.extend(run.take().filter(|p| p.points.len() > 1));
            run = Some(Piece {
                points: vec![a],
                first_segment: k,
                cut_end: false,
            });
        }
        if let Some(piece) = run.as_mut() {
            piece.points.push(b);
            piece.cut_end = b != pair[1];
        }
    }
    pieces.extend(run.filter(|p| p.points.len() > 1));
    pieces
}

/// The start of a piece, in subpixels, before it is reduced: AGG's start at the end of the
/// piece's first segment the filter keeps, less that segment's length. Where that segment ends
/// at the piece's cut end, AGG's start at the vertex the piece's first segment starts from, plus
/// the distance to the piece's first point. `None` where the filter keeps no segment of the
/// piece, which draws nothing. `vertices` and `starts` are the whole path's, `piece` its points
/// in subpixels.
fn piece_start(
    piece: &[(i32, i32)],
    first_segment: usize,
    cut_end: bool,
    vertices: &[(i32, i32)],
    starts: &[i64],
) -> Option<i64> {
    let kept = filtered(piece);
    let &(_, len) = kept.first()?;
    let &(end, _) = kept.get(1)?;
    if cut_end && end == piece.len() - 1 {
        return Some(starts[first_segment] + distance(vertices[first_segment], piece[0]));
    }
    Some(starts[first_segment + end] - len)
}

/// agg-rust's outline-image renderer, its start reduced by whole widths of the pattern after each
/// segment, which moves no pixel, so that it stays within `i32` however long the path is (Q-3).
struct Carried<'r, 'p, 'b> {
    inner: RendererOutlineImage<'r, PixfmtMapserver<'b>, LineImagePattern<BilinearRgba8>>,
    pattern: &'p LineImagePattern<BilinearRgba8>,
}

impl Carried<'_, '_, '_> {
    /// Sets the start, in subpixels, reduced by whole widths of the pattern.
    fn set_start(&mut self, start: i64) {
        let start = start.rem_euclid(i64::from(self.pattern.pattern_width()));
        self.inner.set_start_x(start as f64 / f64::from(SCALE));
    }

    fn reduce(&mut self) {
        let start = (self.inner.start_x() * f64::from(SCALE)) as i64;
        self.set_start(start);
    }
}

impl OutlineAaRenderer for Carried<'_, '_, '_> {
    fn accurate_join_only(&self) -> bool {
        self.inner.accurate_join_only()
    }

    fn line0(&mut self, lp: &LineParameters) {
        self.inner.line0(lp);
        self.reduce();
    }

    fn line1(&mut self, lp: &LineParameters, sx: i32, sy: i32) {
        self.inner.line1(lp, sx, sy);
        self.reduce();
    }

    fn line2(&mut self, lp: &LineParameters, ex: i32, ey: i32) {
        self.inner.line2(lp, ex, ey);
        self.reduce();
    }

    fn line3(&mut self, lp: &LineParameters, sx: i32, sy: i32, ex: i32, ey: i32) {
        self.inner.line3(lp, sx, sy, ex, ey);
        self.reduce();
    }

    fn semidot(&mut self, cmp: fn(i32) -> bool, xc1: i32, yc1: i32, xc2: i32, yc2: i32) {
        self.inner.semidot(cmp, xc1, yc1, xc2, yc2);
    }

    fn pie(&mut self, xc: i32, yc: i32, x1: i32, y1: i32, x2: i32, y2: i32) {
        self.inner.pie(xc, yc, x1, y1, x2, y2);
    }
}

/// agg-rust's `max_half_width` of `render_line_image`: the pixels it steps across the line from
/// each pixel along it, at most.
const MAX_HALF_WIDTH: i64 = 64;

/// How far the join point `bisectrix(l1, l2)` sets lies from the vertex the lines `l1` and `l2`
/// meet at, in subpixels, at most, for lines `l1` and `l2` subpixels long. It is
/// `d2 - d1 × l2 / l1`, `d1` and `d2` the lines' vectors, each at most its length plus ½ since
/// `line_aa_vertex` rounds it, and `l1` at least 385, the filter's 384 plus one: `2 × l2 + l2 /
/// 770`, plus 3 for the halves and for `iround`. Where that point lies within one pixel,
/// `bisectrix` takes the mean of the two lines' perpendiculars instead, at most `(l1 + l2) / 2`
/// plus 1 for its shift.
fn join_reach(l1: i64, l2: i64) -> i64 {
    (2 * l2 + l2 / 770 + 3).max((l1 + l2) / 2 + 1)
}

/// Whether agg-rust's renderer stays within `i32` on a piece whose segments, as AGG's filter keeps
/// them, are `segments` subpixels long, with `pattern`. Where it would pass, AGG's C++ is
/// undefined, and the piece is not drawn (Q-3). Two bounds decide it.
///
/// The pattern's place: `render_line_image` offsets the start, below one width of the pattern
/// when a segment begins, by the pattern's width times its extent across the line plus 2, moves
/// it by up to the segment's length within the segment, and adds the pixel's distance along it,
/// again up to that length.
///
/// The joins: `DistanceInterpolator4` multiplies, in `line_mr` units of 16 subpixels, the pixel
/// less the join point by the join point less the segment's end, coordinate by coordinate, for
/// the start join `sx`, `sy` and the end join `ex`, `ey`. It steps by those terms from pixel to
/// pixel, which keeps its value that difference of products at each pixel. Only `line3` builds
/// it, since agg-rust's `line0` to `line2` draw nothing for this renderer. For each segment:
///
/// - the join point lies from the segment's end by at most [`join_reach`] of the segment and the
///   one before it at the start, and of the segment and the one after it at the end, or by its
///   own length plus 1, the perpendicular `rasterizer_outline_aa` gives an end without a
///   neighbour;
/// - `line3_no_clip` halves a segment longer than `line_max_length`, and each join point's
///   distance with it, until the part is no longer; a join between two parts is a perpendicular
///   halved with them, and one `fix_degenerate_bisectrix` replaces the part's perpendicular, each
///   at most the part's length; the halving's shifts add at most 3 subpixels;
/// - the pixels lie within the part grown by `256 × (2 × extent + 68)` subpixels: the
///   `extent + 1` steps before its start and after its end, each up to 1.5 pixels, a pixel's
///   rounding, the [`MAX_HALF_WIDTH`] pixels across, and one step of the interpolator;
/// - so `v`, the join point less the part's end it joins, is at most `j`, the larger of the join
///   points' distances and the part's length, plus 3, and `u`, the pixel less that end, at most
///   the part's length plus that growth plus 3, each in `line_mr` units at most its subpixels
///   `>> 4` plus 3 for the shifts' rounding;
/// - the pixel less the join point is `u - v`, so a product of one coordinate of it and one of
///   `v` is at most `|u| × |v| + |v|² / 2`, and the difference of the two products,
///   `u × v` crosswise, at most `|u| × |v|`; that sum stays within `i32`.
fn within_i32(pattern: &LineImagePattern<BilinearRgba8>, segments: &[i64]) -> bool {
    let width = i64::from(pattern.pattern_width());
    let extent = i64::from((pattern.line_width() + SCALE) >> SHIFT);
    let longest = segments.iter().copied().max().unwrap_or(0);
    if (extent + 3) * width + 2 * longest > i64::from(i32::MAX) {
        return false;
    }
    let growth = i64::from(SCALE) * (2 * extent + 4 + MAX_HALF_WIDTH);
    segments.iter().enumerate().all(|(i, &len)| {
        let start = match i.checked_sub(1) {
            Some(before) => join_reach(segments[before], len),
            None => len + 1,
        };
        let end = match segments.get(i + 1) {
            Some(&after) => join_reach(len, after),
            None => len + 1,
        };
        let (mut part, mut joins) = (len, start.max(end));
        while part > i64::from(LINE_MAX_LENGTH) {
            part >>= 1;
            joins >>= 1;
        }
        let v = ((joins.max(part) + 3) >> 4) + 3;
        let u = ((part + growth + 3) >> 4) + 3;
        u * v + v * v / 2 <= i64::from(i32::MAX)
    })
}

impl AggRenderer {
    /// Brushes `paths` with `tile` as `agg2RenderLineTiled` brushes a shape's lines: the tile
    /// copied into AGG's `line_image_pattern` through [`BilinearRgba8`], drawn by agg-rust's
    /// `renderer_outline_image` through `rasterizer_outline_aa` with no clip box, a scale of 1,
    /// and a start of 0 carried over every segment and path. Each path is clipped first to the
    /// image grown by `reach` (ADR-0027) and fed as integers by [`line_coord_sat`]. A path the clip
    /// leaves whole starts where AGG starts it; each piece of one it cuts starts where AGG starts
    /// it at the piece's first segment (see [`piece_start`]).
    pub(crate) fn brushed_line(&mut self, paths: &[Vec<Point>], tile: &Tile, reach: f64) {
        let image = tile.image(built_tile);
        let pattern = LineImagePattern::<BilinearRgba8>::with_source(&Source(image));
        let reach = if reach > 0.0 { reach } else { 0.0 };
        let rect = Rect::around_image(self.width, self.height, reach);
        self.with_agg(|renderer| {
            let mut rasterizer = RasterizerOutlineAa::new();
            let mut carried = Carried {
                inner: RendererOutlineImage::new(renderer, &pattern),
                pattern: &pattern,
            };
            let mut carry = 0;
            for path in paths {
                let points: Vec<Point> = path
                    .iter()
                    .copied()
                    .filter(|p| p.x.is_finite() && p.y.is_finite())
                    .collect();
                let subpixels = |p: &Point| (line_coord_sat(p.x), line_coord_sat(p.y));
                let vertices: Vec<(i32, i32)> = points.iter().map(subpixels).collect();
                let (starts, left) = starts(&vertices, carry);
                carry = left;
                for piece in pieces(&points, &rect) {
                    let fed: Vec<(i32, i32)> = piece.points.iter().map(subpixels).collect();
                    let Some(start) =
                        piece_start(&fed, piece.first_segment, piece.cut_end, &vertices, &starts)
                    else {
                        continue;
                    };
                    // The filter's last vertex carries no segment.
                    let mut segments: Vec<i64> =
                        filtered(&fed).iter().map(|&(_, len)| len).collect();
                    segments.pop();
                    if !within_i32(&pattern, &segments) {
                        continue;
                    }
                    carried.set_start(start);
                    let (x, y) = fed[0];
                    rasterizer.move_to(x, y);
                    for &(x, y) in &fed[1..] {
                        rasterizer.line_to(x, y);
                    }
                    rasterizer.render(&mut carried, false);
                }
            }
        });
    }
}

#[cfg(test)]
mod tests;
