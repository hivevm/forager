//! The reference renderer: the display list rasterized with agg-rust (ADR-0015).

// How a stroke's width, cap, join, and dashes are handed to the stroker follows MapServer's
// agg2RenderLine() and applyCJC(), src/mapagg.cpp of the release pinned in
// scripts/fetch-msautotest.sh.
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

#![warn(clippy::disallowed_types, clippy::disallowed_methods)]

mod clip;
mod glyphs;

use agg_rust::basics::{FillingRule, VertexSource};
use agg_rust::color::Rgba8;
use agg_rust::conv_dash::ConvDash;
use agg_rust::conv_stroke::ConvStroke;
use agg_rust::math_stroke::{LineCap as AggCap, LineJoin as AggJoin};
use agg_rust::path_storage::PathStorage;
use agg_rust::pixfmt_rgba::PixfmtRgba32;
use agg_rust::rasterizer_scanline_aa::RasterizerScanlineAa;
use agg_rust::renderer_base::RendererBase;
use agg_rust::renderer_scanline::render_scanlines_aa_solid;
use agg_rust::rendering_buffer::RowAccessor;
use agg_rust::scanline_u::ScanlineU8;
use forager_render::{Dashes, DisplayItem, DrawOp, LineCap, LineJoin, Point, Renderer, RgbaImage};
use forager_style_ir::Rgba;

use crate::clip::{Rect, clip_line, clip_ring};

/// Pixels beyond the image that a clipped path may reach, besides half the stroke width.
const CLIP_MARGIN: f64 = 2.0;

/// Rasterizes into a straight-alpha RGBA buffer it owns.
#[derive(Default)]
pub struct AggRenderer {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl AggRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Runs `f` on an agg-rust renderer attached to the pixel buffer.
    #[allow(unsafe_code)]
    fn with_agg(&mut self, f: impl FnOnce(&mut RendererBase<PixfmtRgba32<'_>>)) {
        let Some(stride) = i32::try_from(self.width)
            .ok()
            .and_then(|w| w.checked_mul(4))
        else {
            return;
        };
        if self.pixels.is_empty() || self.pixels.len() != stride as usize * self.height as usize {
            return;
        }
        // SAFETY: `pixels` holds exactly `stride` × height bytes, checked just above, and the
        // accessor, the pixel format, and the renderer built on it are
        // dropped at the end of this function, while `self.pixels` is mutably borrowed and
        // cannot move or be read elsewhere.
        let mut buffer = unsafe {
            RowAccessor::new_with_buf(self.pixels.as_mut_ptr(), self.width, self.height, stride)
        };
        let mut renderer = RendererBase::new(PixfmtRgba32::new(&mut buffer));
        f(&mut renderer);
    }

    fn rasterize(&mut self, path: &mut PathStorage, color: Rgba, stroke: Option<&Stroke<'_>>) {
        let color = Rgba8::new(
            u32::from(color.r),
            u32::from(color.g),
            u32::from(color.b),
            u32::from(color.a),
        );
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

impl Renderer for AggRenderer {
    fn begin(&mut self, width: u32, height: u32, background: Rgba) {
        // An image whose size overflows is drawn as no image at all; the engine never asks for one.
        let pixels = (width as usize).checked_mul(height as usize);
        let fits = pixels.and_then(|n| n.checked_mul(4)).is_some()
            && i32::try_from(width).is_ok_and(|w| w.checked_mul(4).is_some());
        let (width, height) = if fits { (width, height) } else { (0, 0) };
        self.width = width;
        self.height = height;
        self.pixels = [background.r, background.g, background.b, background.a]
            .repeat(width as usize * height as usize);
    }

    fn draw(&mut self, item: &DisplayItem) {
        match &item.op {
            DrawOp::Fill { rings, color } => {
                let mut path = PathStorage::new();
                let rect = Rect::around_image(self.width, self.height, CLIP_MARGIN);
                for ring in rings {
                    add(&mut path, &clip_ring(ring, &rect), true);
                }
                self.rasterize(&mut path, *color, None);
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
            DrawOp::Glyphs { path, color } => self.glyphs(path, *color),
        }
    }

    fn finish(self) -> RgbaImage {
        RgbaImage {
            width: self.width,
            height: self.height,
            pixels: self.pixels,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
                paths: vec![huge, broken],
                closed: true,
                color: RED,
                width: 1e9,
                cap: LineCap::Butt,
                join: LineJoin::Miter,
                dashes: Dashes::default(),
            },
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
