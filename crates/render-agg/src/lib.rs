//! The reference renderer: the display list rasterized with agg-rust (ADR-0015).

mod clip;

use agg_rust::basics::FillingRule;
use agg_rust::color::Rgba8;
use agg_rust::conv_stroke::ConvStroke;
use agg_rust::math_stroke::{LineCap, LineJoin};
use agg_rust::path_storage::PathStorage;
use agg_rust::pixfmt_rgba::PixfmtRgba32;
use agg_rust::rasterizer_scanline_aa::RasterizerScanlineAa;
use agg_rust::renderer_base::RendererBase;
use agg_rust::renderer_scanline::render_scanlines_aa_solid;
use agg_rust::rendering_buffer::RowAccessor;
use agg_rust::scanline_u::ScanlineU8;
use forager_render::{DisplayItem, DrawOp, Point, Renderer, RgbaImage};
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

    fn rasterize(&mut self, path: &mut PathStorage, color: Rgba, stroke: Option<f64>) {
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
                Some(width) => {
                    let mut outline = ConvStroke::new(path);
                    outline.set_width(width);
                    // MapServer: round caps and bevel joins by default, and for lines no wider
                    // than a pixel bevel joins with AGG.s butt caps (mapagg.cpp, applyCJC).
                    if width > 1.0 {
                        outline.set_line_cap(LineCap::Round);
                    }
                    outline.set_line_join(LineJoin::Bevel);
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
        let mut path = PathStorage::new();
        match &item.op {
            DrawOp::Fill { rings, color } => {
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
            } => {
                if !(width.is_finite() && *width > 0.0) {
                    return;
                }
                // A wider stroke covers the image all the same, and the margin stays small
                // enough for agg-rust's fixed-point coordinates.
                let width = width.min(2.0 * f64::from(self.width + self.height));
                let margin = CLIP_MARGIN + width / 2.0;
                let rect = Rect::around_image(self.width, self.height, margin);
                for points in paths {
                    if *closed {
                        add(&mut path, &clip_ring(points, &rect), true);
                    } else {
                        for run in clip_line(points, &rect) {
                            add(&mut path, &run, false);
                        }
                    }
                }
                self.rasterize(&mut path, *color, Some(width));
            }
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
    use std::time::Instant;

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
        }]);
        assert_eq!(image.pixel(32, 32), Some(RED));
        assert_eq!(image.pixel(32, 20), Some(WHITE));
    }

    #[test]
    fn hostile_coordinates_render_quickly_without_a_panic() {
        // Verifies: ADR-0015, Q-3
        let started = Instant::now();
        let huge = vec![p(-1e15, -1e15), p(1e15, 5.0), p(30.0, 1e15)];
        let broken = vec![p(f64::NAN, 1.0), p(f64::INFINITY, 5.0), p(30.0, 3.0)];
        let _ = render(&[
            DrawOp::Fill {
                rings: vec![huge.clone(), broken.clone()],
                color: RED,
            },
            DrawOp::Stroke {
                paths: vec![huge, broken],
                closed: false,
                color: RED,
                width: 1e9,
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
