// The drawing of glyphs follows agg2RenderGlyphsPath of MapServer's src/mapagg.cpp, at the release
// pinned in scripts/fetch-msautotest.sh (ADR-0028).
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

use agg_rust::basics::{FillingRule, PATH_FLAGS_NONE, VertexSource};
use agg_rust::conv_contour::ConvContour;
use agg_rust::conv_curve::ConvCurve;
use agg_rust::path_storage::PathStorage;
use agg_rust::rasterizer_scanline_aa::RasterizerScanlineAa;
use agg_rust::renderer_scanline::render_scanlines_aa_solid;
use agg_rust::scanline_u::ScanlineU8;
use forager_render::{GlyphOutline, MAX_GLYPH_REACH, PathCommand};
use forager_style_ir::Rgba;

use crate::{AggRenderer, CLIP_MARGIN};

impl AggRenderer {
    /// Draws `path` as `agg2RenderGlyphsPath` draws a text path, its curves flattened by
    /// `conv_curve`, under the non-zero rule, with no gamma: first its outline, the
    /// `conv_contour` of the curves at the outline's width, then its fill in `color`. A path with
    /// a point past [`MAX_GLYPH_REACH`], or one that is no number, is not drawn: the engine
    /// leaves out and reports such a glyph before it reaches the display list, so this guards
    /// the renderer against any other display list.
    pub(crate) fn glyphs(
        &mut self,
        path: &[PathCommand],
        color: Option<Rgba>,
        outline: Option<GlyphOutline>,
    ) {
        let reach = f64::from(self.width.max(self.height)) + MAX_GLYPH_REACH;
        let mut storage = PathStorage::new();
        for command in path {
            let points = match *command {
                PathCommand::MoveTo(p) => {
                    storage.move_to(p.x, p.y);
                    vec![p]
                }
                PathCommand::LineTo(p) => {
                    storage.line_to(p.x, p.y);
                    vec![p]
                }
                PathCommand::QuadTo(c, p) => {
                    storage.curve3(c.x, c.y, p.x, p.y);
                    vec![c, p]
                }
                PathCommand::CubicTo(c0, c1, p) => {
                    storage.curve4(c0.x, c0.y, c1.x, c1.y, p.x, p.y);
                    vec![c0, c1, p]
                }
                PathCommand::Close => {
                    storage.close_polygon(PATH_FLAGS_NONE);
                    vec![]
                }
            };
            if !points
                .iter()
                .all(|p| p.x.abs() <= reach && p.y.abs() <= reach)
            {
                return;
            }
        }
        if let Some(outline) = outline
            && outline.width.is_finite()
        {
            let curves = ConvCurve::new(&mut storage);
            let mut contour = ConvContour::new(curves);
            contour.set_width(outline.width);
            self.fill_glyphs(&mut contour, outline.color);
        }
        if let Some(color) = color {
            let mut curves = ConvCurve::new(&mut storage);
            self.fill_glyphs(&mut curves, color);
        }
    }

    /// Fills `source` in `color` under the non-zero rule.
    fn fill_glyphs<VS: VertexSource>(&mut self, source: &mut VS, color: Rgba) {
        let color = crate::rgba8(color);
        let (width, height) = (f64::from(self.width), f64::from(self.height));
        self.with_agg(|renderer| {
            let mut rasterizer = RasterizerScanlineAa::new();
            let mut scanline = ScanlineU8::new();
            rasterizer.filling_rule(FillingRule::NonZero);
            // A glyph's curves cannot be clipped as a polygon is, so the rasterizer clips its
            // flattened lines to the image enlarged by the margin, as ADR-0015 clips every path;
            // the cells of a glyph far larger than the image then stay within it.
            rasterizer.clip_box(
                -CLIP_MARGIN,
                -CLIP_MARGIN,
                width + CLIP_MARGIN,
                height + CLIP_MARGIN,
            );
            rasterizer.add_path(source, 0);
            render_scanlines_aa_solid(&mut rasterizer, &mut scanline, renderer, &color);
        });
    }
}
