//! The reference renderer: the display list rasterized with agg-rust (ADR-0015).

// How a stroke's width, cap, join, and dashes are handed to the stroker follows MapServer's
// agg2RenderLine() and applyCJC(), src/mapagg.cpp of the release pinned in
// scripts/fetch-msautotest.sh; how a hatch is stroked and clipped to its polygon follows
// msHatchPolygon(), src/mapagg.cpp, there; how a layer's image is laid over the map follows
// aggCompositeRasterBufferNoPixman(), src/mapagg.cpp, and the blend_from() of the AGG it bundles;
// how the alpha is divided out follows saveAsPNG(), src/mapimageio.c.
// Copyright (c) 1996-2005 Regents of the University of Minnesota.
// Copyright (c) 1996-2007 Regents of the University of Minnesota (src/mapagg.cpp).
// Copyright (c) 2009 Thomas Bonfort (src/mapimageio.c).
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

#![warn(clippy::disallowed_types, clippy::disallowed_methods)]

mod clip;
mod glyphs;
mod hatch;
mod pixfmt;

use agg_rust::basics::{FillingRule, VertexSource};
use agg_rust::color::Rgba8;
use agg_rust::conv_dash::ConvDash;
use agg_rust::conv_stroke::ConvStroke;
use agg_rust::math_stroke::{LineCap as AggCap, LineJoin as AggJoin};
use agg_rust::path_storage::PathStorage;
use agg_rust::rasterizer_scanline_aa::{RasterizerScanlineAa, Scanline};
use agg_rust::renderer_base::RendererBase;
use agg_rust::renderer_scanline::render_scanlines_aa_solid;
use agg_rust::scanline_boolean_algebra::ScanlineGenerator;
use agg_rust::scanline_u::ScanlineU8;
use forager_render::{
    Dashes, DisplayItem, DrawOp, Hatch, LineCap, LineJoin, Point, Renderer, RgbaImage,
};
use forager_style_ir::Rgba;

use crate::clip::{Rect, clip_line, clip_ring};
use crate::pixfmt::{PixfmtMapserver, copy_or_blend_pix, premultiplied};

/// Pixels beyond the image that a clipped path may reach, besides half the stroke width.
const CLIP_MARGIN: f64 = 2.0;

/// Rasterizes into an RGBA buffer it owns. The buffer holds its colours premultiplied by their
/// alpha and blends them through [`PixfmtMapserver`], as MapServer's AGG buffer does;
/// [`Renderer::finish`] divides the alpha out, as MapServer's `saveAsPNG` does for an RGBA
/// image.
#[derive(Default)]
pub struct AggRenderer {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
    /// The image beneath the layer being drawn into `pixels`, between
    /// [`Renderer::begin_layer`] and [`Renderer::end_layer`].
    beneath: Option<Vec<u8>>,
}

impl AggRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Runs `f` on an agg-rust renderer attached to the pixel buffer through MapServer's pixel
    /// format.
    fn with_agg(&mut self, f: impl FnOnce(&mut RendererBase<PixfmtMapserver<'_>>)) {
        if self.pixels.is_empty()
            || self.pixels.len() != self.width as usize * self.height as usize * 4
        {
            return;
        }
        let mut renderer = RendererBase::new(PixfmtMapserver::new(
            &mut self.pixels,
            self.width,
            self.height,
        ));
        f(&mut renderer);
    }

    fn rasterize(&mut self, path: &mut PathStorage, color: Rgba, stroke: Option<&Stroke<'_>>) {
        let color = rgba8(color);
        self.with_agg(|renderer| {
            let mut rasterizer = RasterizerScanlineAa::new();
            let mut scanline = ScanlineU8::new();
            match stroke {
                Some(stroke) if stroke.dashes.pattern.len() >= 2 => {
                    let mut dashed = ConvDash::new(path);
                    add_dashes(&mut dashed, stroke.dashes);
                    let mut outline = ConvStroke::new(dashed);
                    stroke.apply(&mut outline);
                    rasterizer.add_path(&mut outline, 0);
                }
                Some(stroke) => {
                    let mut outline = ConvStroke::new(path);
                    stroke.apply(&mut outline);
                    rasterizer.add_path(&mut outline, 0);
                }
                None => {
                    rasterizer.filling_rule(FillingRule::EvenOdd);
                    rasterizer.add_path(path, 0);
                }
            }
            render_scanlines_aa_solid(&mut rasterizer, &mut scanline, renderer, &color);
        });
    }
}

/// What a stroke hands to agg-rust's stroker.
struct Stroke<'a> {
    width: f64,
    cap: LineCap,
    join: LineJoin,
    dashes: &'a Dashes,
}

impl Stroke<'_> {
    /// Sets the stroker's width, cap, and join as applyCJC() does: a join of `none` is a bevel,
    /// and a cap MapServer does not map, `triangle`, stays AGG's default, butt. The inner join
    /// and the miter limit stay AGG's defaults, a miter and 4. Forager sets up a fresh stroker for
    /// every path; MapServer keeps one per image and carries its state from line to line, so
    /// there a thin line after a wide round-capped one is round-capped.
    fn apply<VS: VertexSource>(&self, stroke: &mut ConvStroke<VS>) {
        stroke.set_width(self.width);
        stroke.set_line_join(match self.join {
            LineJoin::Round => AggJoin::Round,
            LineJoin::Miter => AggJoin::Miter,
            LineJoin::Bevel | LineJoin::None => AggJoin::Bevel,
        });
        match self.cap {
            LineCap::Butt => stroke.set_line_cap(AggCap::Butt),
            LineCap::Round => stroke.set_line_cap(AggCap::Round),
            LineCap::Square => stroke.set_line_cap(AggCap::Square),
            LineCap::Triangle => {}
        }
    }
}

/// Adds the dashes as agg2RenderLine() does: each length rounded to whole pixels, half to even
/// as `lrint`, and at least 1, one that is not finite 1; an odd last length dropped; and the
/// start moved back by the pattern's offset. AGG walks a start dash by dash, so the start is first reduced to one period
/// of the pattern, which draws the same, and each length is bounded by the largest `int`, the
/// type MapServer sums the pattern's length in, so that the length stays finite; a huge
/// INITIALGAP or PATTERN then costs no more than any other (Q-3, Q-9). A start before the
/// pattern keeps its sign, which AGG reads as a start for the first path only.
fn add_dashes<VS: VertexSource>(dashed: &mut ConvDash<VS>, dashes: &Dashes) {
    // lrint of a length that is not finite gives no number, which MapServer's bound turns to 1.
    let pixels = |length: f64| {
        if length.is_finite() {
            length.round_ties_even().clamp(1.0, f64::from(i32::MAX))
        } else {
            1.0
        }
    };
    let mut length = 0.0;
    for [dash, gap] in dashes.pattern.as_chunks::<2>().0 {
        let (dash, gap) = (pixels(*dash), pixels(*gap));
        dashed.add_dash(dash, gap);
        length += dash + gap;
    }
    let start = length - dashes.offset;
    if dashes.offset > 0.0 && start.is_finite() && length > 0.0 {
        dashed.dash_start(start.signum() * (start.abs() % length));
    }
}

fn add(path: &mut PathStorage, points: &[Point], close: bool) {
    let mut points = points.iter();
    let Some(first) = points.next() else { return };
    path.move_to(first.x, first.y);
    for p in points {
        path.line_to(p.x, p.y);
    }
    if close {
        path.close_polygon(0);
    }
}

impl AggRenderer {
    /// Strokes `paths`, clipped first to the image grown by the reach of the stroke.
    fn stroke(&mut self, paths: &[Vec<Point>], closed: bool, color: Rgba, stroke: Stroke<'_>) {
        if !(stroke.width.is_finite() && stroke.width > 0.0) {
            return;
        }
        // A wider stroke covers the image all the same, and the margin stays small enough for
        // agg-rust's fixed-point coordinates.
        let width = stroke.width.min(2.0 * f64::from(self.width + self.height));
        let stroke = Stroke { width, ..stroke };
        // A miter join reaches up to its limit of 4 half widths past the path, a bevel or round
        // join and a cap no more than about one.
        let reach = match stroke.join {
            LineJoin::Miter => 2.0 * width,
            LineJoin::Round | LineJoin::Bevel | LineJoin::None => width / 2.0,
        };
        let rect = Rect::around_image(self.width, self.height, CLIP_MARGIN + reach);
        let mut path = PathStorage::new();
        for points in paths {
            if closed {
                add(&mut path, &clip_ring(points, &rect), true);
            } else {
                for run in clip_line(points, &rect) {
                    add(&mut path, &run, false);
                }
            }
        }
        self.rasterize(&mut path, color, Some(&stroke));
    }
}

impl AggRenderer {
    /// The rings as a path, each clipped to the image enlarged by the margin.
    fn area(&self, rings: &[Vec<Point>]) -> PathStorage {
        let mut path = PathStorage::new();
        let rect = Rect::around_image(self.width, self.height, CLIP_MARGIN);
        for ring in rings {
            add(&mut path, &clip_ring(ring, &rect), true);
        }
        path
    }

    /// Blends the scanlines of `shape` in `color`, each coverage looked up in `gamma`.
    fn paint<G: ScanlineGenerator>(&mut self, shape: &mut G, color: Rgba, gamma: &[u8; 256]) {
        let color = rgba8(color);
        self.with_agg(|renderer| {
            if !shape.rewind_scanlines() {
                return;
            }
            let mut scanline = ScanlineU8::new();
            scanline.reset(shape.min_x(), shape.max_x());
            let mut covers = Vec::new();
            while shape.sweep_scanline(&mut scanline) {
                for span in scanline.begin() {
                    if let Ok(len) = usize::try_from(span.len)
                        && len > 0
                    {
                        covers.clear();
                        covers.extend(
                            scanline.covers()[span.cover_offset..span.cover_offset + len]
                                .iter()
                                .map(|&c| gamma[usize::from(c)]),
                        );
                        renderer.blend_solid_hspan(span.x, scanline.y(), span.len, &color, &covers);
                    }
                }
            }
        });
    }

    /// Fills a hatch as msHatchPolygon() does: the lines stroked with butt caps, each dashed
    /// from its first point with the pattern as given, the stroke intersected with the polygon
    /// as MapServer's Clipper intersects them (see [`hatch::intersect`]), and the intersection
    /// filled under the non-zero rule through MapServer's default gamma, as
    /// renderPolygonHatches() fills it. The polygon is clipped to the image grown by a margin and
    /// the lines to the image grown by the stroke before they are cut, a dashed line keeping the
    /// phase its dashes had at its first point. A hatch beyond the bounds of
    /// [`Hatch::is_bounded`] draws nothing (Q-9).
    fn hatch(&mut self, hatch: &Hatch) {
        if !(hatch.width.is_finite() && hatch.width > 0.0)
            || !hatch.is_bounded(self.width, self.height)
        {
            return;
        }
        let width = hatch.width.min(2.0 * f64::from(self.width + self.height));
        let fill_rect = Rect::around_image(self.width, self.height, CLIP_MARGIN);
        let rings: Vec<Vec<Point>> = hatch
            .rings
            .iter()
            .map(|ring| clip_ring(ring, &fill_rect))
            .collect();
        let rect = Rect::around_image(self.width, self.height, CLIP_MARGIN + width / 2.0);
        let Some(pieces) = hatch::pieces(
            &hatch.lines,
            &rect,
            hatch.dashes(),
            width,
            Hatch::MAX_DASHES,
        ) else {
            return;
        };
        let mut path = PathStorage::new();
        for contour in hatch::intersect(&rings, &pieces) {
            add(&mut path, &contour, true);
        }
        let mut area = RasterizerScanlineAa::new();
        area.filling_rule(FillingRule::NonZero);
        area.add_path(&mut path, 0);
        self.paint(&mut area, hatch.color, &gamma_table());
    }
}

/// MapServer's default `GAMMA` of its AGG output formats, `src/mapagg.cpp`.
const GAMMA: f64 = 0.75;

/// The coverage table of AGG's `gamma_linear(0, GAMMA)`, as a rasterizer builds it from the
/// function: a coverage at or past the gamma is full, one below it grows in proportion. MapServer
/// fills polygon shapes and hatches through it; a `GAMMA` of the output format or of the layer
/// is not carried yet.
fn gamma_table() -> [u8; 256] {
    std::array::from_fn(|cover| {
        let x = cover as f64 / 255.0;
        let y = if x >= GAMMA { 1.0 } else { x / GAMMA };
        // AGG's uround, which stays within 0 to 255 here.
        (y * 255.0 + 0.5) as u8
    })
}

/// A colour as aggColor() hands it to AGG: premultiplied by its alpha.
fn rgba8(color: Rgba) -> Rgba8 {
    premultiplied(color.r, color.g, color.b, color.a)
}

impl Renderer for AggRenderer {
    fn begin(&mut self, width: u32, height: u32, background: Rgba) {
        // An image whose size overflows is drawn as no image at all; the engine never asks for one.
        let pixels = (width as usize).checked_mul(height as usize);
        let fits = pixels.and_then(|n| n.checked_mul(4)).is_some()
            && i32::try_from(width).is_ok_and(|w| w.checked_mul(4).is_some());
        let (width, height) = if fits { (width, height) } else { (0, 0) };
        self.width = width;
        self.height = height;
        // aggColor() premultiplies the background, as every colour, by its alpha.
        let c = rgba8(background);
        let background = [c.r, c.g, c.b, c.a];
        self.pixels = background.repeat(width as usize * height as usize);
        self.beneath = None;
    }

    fn begin_layer(&mut self) {
        debug_assert!(self.beneath.is_none(), "layers do not nest");
        // msDrawLayer's temporary image, created without a background: cleared to no colour.
        let layer = vec![0; self.pixels.len()];
        self.beneath = Some(std::mem::replace(&mut self.pixels, layer));
    }

    fn end_layer(&mut self, opacity: u8) {
        let Some(beneath) = self.beneath.take() else {
            return;
        };
        let layer = std::mem::replace(&mut self.pixels, beneath);
        lay_over(&mut self.pixels, &layer, opacity);
    }

    fn draw(&mut self, item: &DisplayItem) {
        match &item.op {
            DrawOp::Fill { rings, color } => {
                let mut path = self.area(rings);
                self.rasterize(&mut path, *color, None);
            }
            DrawOp::Polygon { rings, color } => {
                let mut area = RasterizerScanlineAa::new();
                area.filling_rule(FillingRule::EvenOdd);
                area.add_path(&mut self.area(rings), 0);
                self.paint(&mut area, *color, &gamma_table());
            }
            DrawOp::Stroke {
                paths,
                closed,
                color,
                width,
                cap,
                join,
                dashes,
            } => {
                let stroke = Stroke {
                    width: *width,
                    cap: *cap,
                    join: *join,
                    dashes,
                };
                self.stroke(paths, *closed, *color, stroke);
            }
            DrawOp::Hatch(hatch) => self.hatch(hatch),
            DrawOp::Glyphs { path, color } => self.glyphs(path, *color),
        }
    }

    fn finish(mut self) -> RgbaImage {
        for p in self.pixels.as_chunks_mut::<4>().0.iter_mut() {
            unpremultiply(p);
        }
        RgbaImage {
            width: self.width,
            height: self.height,
            pixels: self.pixels,
        }
    }
}

/// Lays the premultiplied image `layer` over `pixels` at `cover`, as AGG's `blend_from` does
/// it pixel by pixel through `copy_or_blend_pix`; aggCompositeRasterBufferNoPixman,
/// src/mapagg.cpp, composites a layer of `COMPOP src-over` so.
fn lay_over(pixels: &mut [u8], layer: &[u8], cover: u8) {
    for (p, c) in pixels
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(layer.as_chunks::<4>().0.iter())
    {
        let c = Rgba8::new(c[0].into(), c[1].into(), c[2].into(), c[3].into());
        copy_or_blend_pix(p, &c, cover);
    }
}

/// A premultiplied pixel made straight as `saveAsPNG` (src/mapimageio.c) makes it for an RGBA
/// image: each colour divided by the alpha over 255 and truncated, a pixel of no alpha all 0. A
/// colour above its alpha, which rounding can leave, stays at 255.
fn unpremultiply(p: &mut [u8]) {
    let a = p[3];
    if a == 255 {
        return;
    }
    if a == 0 {
        p.fill(0);
        return;
    }
    let da = f64::from(a) / 255.0;
    for c in &mut p[..3] {
        *c = (f64::from(*c) / da).min(255.0) as u8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_alpha_is_divided_out_as_save_as_png_divides_it() {
        // Verifies: ADR-0034
        // 3 / (6 / 255) = 127.5, truncated.
        let mut p = [3, 3, 3, 6];
        unpremultiply(&mut p);
        assert_eq!(p, [127, 127, 127, 6]);
        let mut none = [5, 5, 5, 0];
        unpremultiply(&mut none);
        assert_eq!(none, [0, 0, 0, 0]);
        let mut opaque = [9, 8, 7, 255];
        unpremultiply(&mut opaque);
        assert_eq!(opaque, [9, 8, 7, 255]);
        // A colour above its alpha stays at 255.
        let mut over = [9, 0, 0, 8];
        unpremultiply(&mut over);
        assert_eq!(over, [255, 0, 0, 8]);
    }

    const WHITE: Rgba = Rgba::opaque(255, 255, 255);
    const RED: Rgba = Rgba::opaque(200, 80, 80);

    fn p(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    fn render(items: &[DrawOp]) -> RgbaImage {
        let mut renderer = AggRenderer::new();
        renderer.begin(64, 64, WHITE);
        for op in items {
            renderer.draw(&DisplayItem {
                op: op.clone(),
                feature_id: None,
            });
        }
        renderer.finish()
    }

    #[test]
    fn a_square_is_filled_and_its_hole_stays_empty() {
        let outer = vec![p(8.0, 8.0), p(56.0, 8.0), p(56.0, 56.0), p(8.0, 56.0)];
        let hole = vec![p(24.0, 24.0), p(40.0, 24.0), p(40.0, 40.0), p(24.0, 40.0)];
        let image = render(&[DrawOp::Fill {
            rings: vec![outer, hole],
            color: RED,
        }]);
        assert_eq!(image.pixel(12, 12), Some(RED));
        assert_eq!(image.pixel(32, 32), Some(WHITE));
        assert_eq!(image.pixel(2, 2), Some(WHITE));
    }

    #[test]
    fn a_polygon_shape_is_filled_through_the_gamma_and_a_fill_is_not() {
        // Verifies: ADR-0034
        // A square edge at x = 8.5 covers column 8 by half: 0.5 / 0.75 of the colour through
        // MapServer's default gamma of 0.75, half of it without. The cover 171 blends green 80
        // over 255 as blender_rgba_pre does: alpha (255 × 172) >> 8 = 171, and
        // (255 × 84 + 80 × 172) >> 8 = 137; the cover 128: (255 × 127 + 80 × 129) >> 8 = 166.
        let square = vec![p(8.5, 8.0), p(56.0, 8.0), p(56.0, 56.0), p(8.5, 56.0)];
        let green = |op: DrawOp| render(&[op]).pixel(8, 30).unwrap().g;
        let polygon = green(DrawOp::Polygon {
            rings: vec![square.clone()],
            color: RED,
        });
        let fill = green(DrawOp::Fill {
            rings: vec![square],
            color: RED,
        });
        assert_eq!((polygon, fill), (137, 166));
    }

    #[test]
    fn a_hatch_strokes_its_lines_inside_its_polygon_only() {
        // Two lines 2 wide across a square with a hole, dashed 10 on, 10 off from their first
        // points, the second's 20 left of the image; the odd last length is dropped.
        let outer = vec![p(8.0, 8.0), p(56.0, 8.0), p(56.0, 56.0), p(8.0, 56.0)];
        let hole = vec![p(24.0, 24.0), p(40.0, 24.0), p(40.0, 40.0), p(24.0, 40.0)];
        let image = render(&[DrawOp::Hatch(Hatch {
            rings: vec![outer, hole],
            lines: vec![
                [p(0.0, 16.0), p(64.0, 16.0)],
                [p(-20.0, 32.0), p(64.0, 32.0)],
            ],
            color: RED,
            width: 2.0,
            pattern: vec![10.0, 10.0, 99.0],
        })]);
        assert_eq!(
            image.pixel(25, 15),
            Some(RED),
            "a dash, 20 to 30, inside the polygon"
        );
        assert_eq!(
            image.pixel(4, 15),
            Some(WHITE),
            "a dash, 0 to 10, outside it"
        );
        assert_eq!(image.pixel(25, 18), Some(WHITE), "beside the line");
        assert_eq!(
            image.pixel(12, 31),
            Some(WHITE),
            "a gap, 30 to 40 from the start"
        );
        assert_eq!(image.pixel(22, 31), Some(RED), "a dash, 40 to 50");
        assert_eq!(image.pixel(32, 31), Some(WHITE), "the hole");
        assert_eq!(image.pixel(50, 31), Some(WHITE), "a gap, 70 to 80");
    }

    fn hatched(rings: Vec<Vec<Point>>, lines: Vec<[Point; 2]>, pattern: Vec<f64>) -> RgbaImage {
        render(&[DrawOp::Hatch(Hatch {
            rings,
            lines,
            color: RED,
            width: 3.0,
            pattern,
        })])
    }

    fn rectangle(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Point> {
        vec![p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1)]
    }

    #[test]
    fn where_rings_overlap_or_repeat_the_hatch_there_is_empty() {
        // Verifies: ADR-0032
        // Two rings that overlap from x = 20 to 40, and one ring given twice, under a line 3 wide
        // at y = 30.25: under the even-odd rule the part covered twice is outside the polygon,
        // on the line's edge rows 28 and 31 too.
        let line = vec![[p(0.0, 30.25), p(64.0, 30.25)]];
        let a = rectangle(0.0, 0.0, 40.0, 62.0);
        let b = rectangle(20.0, 0.0, 60.0, 62.0);
        let overlapping = hatched(vec![a.clone(), b], line.clone(), vec![]);
        let twice = hatched(vec![a.clone(), a], line, vec![]);
        for y in [28, 29, 30, 31] {
            assert_eq!(overlapping.pixel(30, y), Some(WHITE), "overlap, row {y}");
            assert_eq!(twice.pixel(10, y), Some(WHITE), "twice, row {y}");
        }
        assert_eq!(overlapping.pixel(10, 30), Some(RED));
        assert_eq!(overlapping.pixel(50, 30), Some(RED));
    }

    #[test]
    fn a_hole_stays_white_under_rotated_dashes() {
        // Verifies: ADR-0032
        // A square with a hole, its hole wound as the square is and touching nothing, under
        // dashed lines 3 wide at 30°, 4 apart: no pixel inside the hole is touched.
        let outer = rectangle(4.0, 4.0, 60.0, 60.0);
        let hole = rectangle(20.5, 18.25, 43.75, 41.5);
        let (cos, sin) = (0.866_025_403_784_438_6, 0.5);
        let lines: Vec<[Point; 2]> = (-20..20)
            .map(|k| {
                let o = 4.0 * f64::from(k);
                let (cx, cy) = (32.0 - sin * o, 32.0 + cos * o);
                [
                    p(cx - 60.0 * cos, cy - 60.0 * sin),
                    p(cx + 60.0 * cos, cy + 60.0 * sin),
                ]
            })
            .collect();
        let image = hatched(vec![outer, hole], lines, vec![7.0, 2.5, 1.0, 2.5]);
        for y in 19..41 {
            for x in 21..43 {
                assert_eq!(image.pixel(x, y), Some(WHITE), "({x}, {y}) in the hole");
            }
        }
        assert!(
            (5..59).any(|x| image.pixel(x, 10) == Some(RED)),
            "the hatch around it"
        );
    }

    /// A comb of 2000 teeth across `side` pixels, its 8003 vertices, and the lines at 45° a
    /// pixel apart across a square image of that side.
    fn comb(side: f64) -> (Vec<Point>, Vec<[Point; 2]>) {
        let tooth = side / 2000.0;
        let mut comb = vec![p(0.0, side), p(side, side)];
        for i in (0..2000).rev() {
            let x = f64::from(i) * tooth;
            comb.extend([
                p(x + tooth, 2.0),
                p(x + tooth / 2.0, 2.0),
                p(x + tooth / 2.0, side - 2.0),
                p(x, side - 2.0),
            ]);
        }
        comb.push(p(0.0, side));
        #[expect(clippy::cast_possible_truncation, reason = "a small whole number")]
        let reach = side as i32;
        let lines = (-reach..reach)
            .map(|k| {
                let o = f64::from(k);
                [p(o - 1.0, -1.0), p(o + side + 1.0, side + 1.0)]
            })
            .collect();
        (comb, lines)
    }

    #[test]
    fn a_comb_under_a_dense_hatch_is_cut_within_the_bounds_or_not_at_all() {
        // Verifies: Q-9
        // Over 4096 × 4096 the comb's teeth cross some 1.1e7 stroke bands, past the bound on
        // the crossings; cutting them takes some 21.6 GB.
        let (teeth, lines) = comb(4096.0);
        let dense = Hatch {
            rings: vec![teeth],
            lines,
            color: RED,
            width: 1.0,
            pattern: vec![],
        };
        assert!(!dense.is_bounded(4096, 4096));
        // Over 64 × 64, some 5e5 crossings, it is cut, and only inside the comb.
        let (teeth, lines) = comb(64.0);
        let image = hatched(vec![teeth], lines, vec![]);
        assert!(
            image.pixels.chunks(4).any(|px| px != [255, 255, 255, 255]),
            "the comb is cut"
        );
        assert_eq!(image.pixel(32, 0), Some(WHITE), "above the teeth");
    }

    #[test]
    fn a_stroke_covers_its_line() {
        let image = render(&[DrawOp::Stroke {
            paths: vec![vec![p(4.0, 32.5), p(60.0, 32.5)]],
            closed: false,
            color: RED,
            width: 3.0,
            cap: LineCap::Round,
            join: LineJoin::None,
            dashes: Dashes::default(),
        }]);
        assert_eq!(image.pixel(32, 32), Some(RED));
        assert_eq!(image.pixel(32, 20), Some(WHITE));
    }

    fn line(width: f64, cap: LineCap, pattern: Vec<f64>) -> DrawOp {
        dashed(width, cap, pattern, 0.0)
    }

    fn dashed(width: f64, cap: LineCap, pattern: Vec<f64>, offset: f64) -> DrawOp {
        DrawOp::Stroke {
            paths: vec![vec![p(10.0, 32.5), p(50.0, 32.5)]],
            closed: false,
            color: RED,
            width,
            cap,
            join: LineJoin::None,
            dashes: Dashes { pattern, offset },
        }
    }

    #[test]
    fn an_outline_has_butt_caps_and_miter_joins_where_a_line_has_round_caps_and_bevels() {
        // A right angle at (32, 32), 8 wide: the miter fills the outer corner, the bevel cuts
        // it; the round cap reaches past the end, the butt cap stops at it. A symbol's outline is
        // stroked with AGG's defaults, butt caps and miter joins; a line by default with round
        // caps and the bevel of a join of `none`.
        let corner = vec![p(10.0, 32.0), p(32.0, 32.0), p(32.0, 54.0)];
        let draw = |cap, join| {
            render(&[DrawOp::Stroke {
                paths: vec![corner.clone()],
                closed: false,
                color: RED,
                width: 8.0,
                cap,
                join,
                dashes: Dashes::default(),
            }])
        };
        let outline = draw(LineCap::Butt, LineJoin::Miter);
        let line = draw(LineCap::Round, LineJoin::None);
        assert_eq!(outline.pixel(35, 29), Some(RED), "the miter");
        assert_eq!(line.pixel(35, 29), Some(WHITE), "the bevel");
        assert_eq!(outline.pixel(8, 32), Some(WHITE), "the butt cap");
        assert_eq!(line.pixel(8, 32), Some(RED), "the round cap");
    }

    #[test]
    fn dash_lengths_round_half_to_even_and_to_at_least_a_pixel() {
        // 2.5 rounds to 2 and 0.2 to 1: dashes of 2 pixels, gaps of 1, from x = 10.
        let image = render(&[line(3.0, LineCap::Butt, vec![2.5, 0.2, 7.0])]);
        assert_eq!(image.pixel(10, 32), Some(RED));
        assert_eq!(image.pixel(11, 32), Some(RED));
        assert_eq!(image.pixel(12, 32), Some(WHITE));
        assert_eq!(image.pixel(13, 32), Some(RED));
    }

    #[test]
    fn a_triangle_cap_is_drawn_as_agg_s_butt_cap() {
        // applyCJC() maps no AGG cap to MapServer's triangle.
        let image = render(&[line(4.0, LineCap::Triangle, vec![])]);
        assert_eq!(image.pixel(8, 32), Some(WHITE));
        assert_eq!(image.pixel(12, 32), Some(RED));
    }

    #[test]
    fn a_line_of_a_pixel_is_dashed_too() {
        let image = render(&[line(1.0, LineCap::Round, vec![4.0, 4.0])]);
        // A 1-pixel line on row 32.5 covers rows 32 and 33 by half each.
        let ink = |x| image.pixel(x, 32).is_some_and(|c| c != WHITE);
        assert!(ink(12), "a dash from 10 to 14");
        assert!(!ink(16), "a gap from 14 to 18");
        assert!(ink(20), "a dash from 18 to 22");
    }

    #[test]
    fn an_offset_beyond_the_pattern_starts_the_first_dash_that_far_into_it() {
        // A pattern of 20 and an offset of 25: the start, -5, makes AGG begin 5 into the dash.
        let image = render(&[dashed(3.0, LineCap::Butt, vec![10.0, 10.0], 25.0)]);
        assert_eq!(
            image.pixel(12, 32),
            Some(RED),
            "the rest of the dash, to 15"
        );
        assert_eq!(image.pixel(20, 32), Some(WHITE), "the gap, to 25");
        assert_eq!(image.pixel(30, 32), Some(RED), "the next dash, to 35");
    }

    #[test]
    fn a_start_many_periods_long_draws_as_its_rest_within_one() {
        let near = render(&[dashed(3.0, LineCap::Butt, vec![10.0, 10.0], 5.0)]);
        let far = render(&[dashed(
            3.0,
            LineCap::Butt,
            vec![10.0, 10.0],
            5.0 + 20.0 * 1e6,
        )]);
        assert_eq!(near.pixel(12, 32), Some(WHITE), "5 into the gap first");
        assert_eq!(near.pixel(20, 32), Some(RED));
        // The start, 20 - (20e6 + 5), lies before the pattern; AGG walks its length, 5 into a
        // period, as it walks the reduced one.
        assert_eq!(far.pixel(12, 32), Some(RED), "5 into the dash");
        assert_eq!(far.pixel(20, 32), Some(WHITE));
    }

    #[test]
    #[expect(
        clippy::disallowed_types,
        clippy::disallowed_methods,
        reason = "the clock bounds the test's run time and reaches no output (Q-2)"
    )]
    fn hostile_coordinates_render_quickly_without_a_panic() {
        // Verifies: ADR-0015, Q-3, Q-9
        let started = std::time::Instant::now();
        // AGG walks a dash start dash by dash: a huge INITIALGAP or PATTERN costs no more.
        let _ = render(&[
            dashed(3.0, LineCap::Butt, vec![2.0, 2.0], 1e300),
            dashed(3.0, LineCap::Butt, vec![2.0, 2.0], 1e9),
            dashed(3.0, LineCap::Butt, vec![1e308, 1e308], 1.0),
            dashed(
                3.0,
                LineCap::Butt,
                vec![f64::NAN, f64::INFINITY],
                f64::INFINITY,
            ),
        ]);
        let huge = vec![p(-1e15, -1e15), p(1e15, 5.0), p(30.0, 1e15)];
        let broken = vec![p(f64::NAN, 1.0), p(f64::INFINITY, 5.0), p(30.0, 3.0)];
        let _ = render(&[
            DrawOp::Fill {
                rings: vec![huge.clone(), broken.clone()],
                color: RED,
            },
            DrawOp::Stroke {
                paths: vec![huge.clone(), broken.clone()],
                closed: false,
                color: RED,
                width: 1e9,
                cap: LineCap::Round,
                join: LineJoin::Miter,
                dashes: Dashes {
                    pattern: vec![1e-300, 1e300, 3.0],
                    offset: 1e300,
                },
            },
            DrawOp::Stroke {
                paths: vec![huge.clone(), broken.clone()],
                closed: true,
                color: RED,
                width: 1e9,
                cap: LineCap::Butt,
                join: LineJoin::Miter,
                dashes: Dashes::default(),
            },
        ]);
        // A hatch's dashes walked one by one, and its polygon cut, stay bounded: tiny or empty
        // dashes over long lines, and a ring of 40 000 vertices, past the bound on the polygon,
        // under 2 000 lines.
        let circle: Vec<Point> = (0..40_000)
            .map(|i| {
                let t = f64::from(i) / 40_000.0 * std::f64::consts::TAU;
                p(32.0 + 30.0 * t.cos(), 32.0 + 30.0 * t.sin())
            })
            .collect();
        let dense: Vec<[Point; 2]> = (0..2_000)
            .map(|i| {
                let y = f64::from(i) * 0.032;
                [p(-1e15, y), p(1e15, y)]
            })
            .collect();
        let hatch = |rings: Vec<Vec<Point>>, lines, width, pattern| {
            DrawOp::Hatch(Hatch {
                rings,
                lines,
                color: RED,
                width,
                pattern,
            })
        };
        let _ = render(&[
            hatch(vec![circle.clone()], dense.clone(), 1.0, vec![]),
            hatch(vec![circle.clone()], dense, 1e9, vec![1e-300, 1e300, 3.0]),
            hatch(
                vec![huge.clone(), broken.clone()],
                vec![
                    [p(-1e15, -1e15), p(1e15, 1e15)],
                    [p(f64::NAN, 0.0), p(5.0, 5.0)],
                ],
                3.0,
                vec![0.0, 0.0],
            ),
            hatch(
                vec![huge, broken],
                vec![[p(-1e15, 0.5), p(1e15, 0.5)]],
                f64::INFINITY,
                vec![f64::NAN, 1.0],
            ),
        ]);
        assert!(started.elapsed().as_secs() < 1, "{:?}", started.elapsed());
    }

    #[test]
    fn a_huge_square_around_the_image_still_fills_it() {
        // Verifies: ADR-0015
        for big in [1e15, 1e200] {
            let square = vec![p(-big, -big), p(big, -big), p(big, big), p(-big, big)];
            let image = render(&[DrawOp::Fill {
                rings: vec![square],
                color: RED,
            }]);
            assert!(
                image.pixels.chunks(4).all(|px| px == [200, 80, 80, 255]),
                "{big}"
            );
        }
    }

    #[test]
    fn glyphs_are_filled_under_the_non_zero_rule_with_their_curves_flattened() {
        // Verifies: ADR-0028
        use forager_render::PathCommand::{Close, LineTo, MoveTo, QuadTo};
        // Two squares of the same winding, one inside the other: the inner one stays filled,
        // which the even-odd rule of a Fill would leave empty.
        let square = |a: f64, b: f64| {
            vec![
                MoveTo(p(a, a)),
                LineTo(p(b, a)),
                LineTo(p(b, b)),
                LineTo(p(a, b)),
                Close,
            ]
        };
        let mut path = square(8.0, 56.0);
        path.extend(square(24.0, 40.0));
        // A quadratic bulge to the right of x = 60, bounded by its control point at x = 64.
        path.extend([
            MoveTo(p(58.0, 20.0)),
            QuadTo(p(70.0, 32.0), p(58.0, 44.0)),
            Close,
        ]);
        let image = render(&[DrawOp::Glyphs { path, color: RED }]);
        assert_eq!(image.pixel(12, 12), Some(RED));
        assert_eq!(
            image.pixel(32, 32),
            Some(RED),
            "the inner square stays filled"
        );
        assert_eq!(image.pixel(62, 32), Some(RED), "the curve is filled");
        assert_eq!(
            image.pixel(62, 22),
            Some(WHITE),
            "the curve, not its control polygon"
        );
    }

    #[test]
    fn a_glyph_far_larger_than_the_image_is_clipped_to_it() {
        // Verifies: ADR-0015
        // Six million pixels a side: without the rasterizer's clip box its cells would span the
        // whole square, and the render would take minutes.
        use forager_render::PathCommand::{Close, LineTo, MoveTo};
        let (a, b) = (-3e6, 3e6);
        let path = vec![
            MoveTo(p(a, a)),
            LineTo(p(b, a)),
            LineTo(p(b, b)),
            LineTo(p(a, b)),
            Close,
        ];
        let image = render(&[DrawOp::Glyphs { path, color: RED }]);
        assert!(image.pixels.chunks(4).all(|px| px == [200, 80, 80, 255]));
    }

    #[test]
    fn a_glyph_path_out_of_reach_is_not_drawn() {
        use forager_render::PathCommand::{Close, LineTo, MoveTo};
        let path = vec![
            MoveTo(p(-1e12, -1e12)),
            LineTo(p(1e12, -1e12)),
            LineTo(p(0.0, f64::NAN)),
            Close,
        ];
        let image = render(&[DrawOp::Glyphs { path, color: RED }]);
        assert!(image.pixels.chunks(4).all(|px| px == [255, 255, 255, 255]));
    }

    #[test]
    fn an_image_whose_size_overflows_is_empty() {
        let mut renderer = AggRenderer::new();
        renderer.begin(u32::MAX, u32::MAX, WHITE);
        renderer.draw(&DisplayItem {
            op: DrawOp::Fill {
                rings: vec![vec![p(0.0, 0.0), p(9.0, 0.0), p(9.0, 9.0)]],
                color: RED,
            },
            feature_id: None,
        });
        let image = renderer.finish();
        assert_eq!((image.width, image.height, image.pixels.len()), (0, 0, 0));
    }
}
