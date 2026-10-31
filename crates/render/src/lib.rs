//! The display list and the renderer trait: what is drawn, in pixel coordinates, independent of
//! the backend that rasterizes it (ADR-0009). Only `style-ir` is known here.

#![warn(clippy::disallowed_types, clippy::disallowed_methods)]

use forager_style_ir::Rgba;
pub use forager_style_ir::{LineCap, LineJoin};
pub use hatch::Hatch;

pub mod clip;
mod hatch;

/// A position in pixels, from the top-left corner of the image.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// One drawing operation.
#[derive(Clone, Debug, PartialEq)]
pub enum DrawOp {
    /// Fills the area the rings enclose under the even-odd rule, so holes stay empty whatever
    /// their winding.
    Fill { rings: Vec<Vec<Point>>, color: Rgba },
    /// Fills the area of a polygon feature as [`DrawOp::Fill`] does, as MapServer's renderer
    /// fills a polygon shape; MapServer's AGG renderer, unlike a symbol's fill, passes its
    /// coverage through the gamma of the output format.
    Polygon { rings: Vec<Vec<Point>>, color: Rgba },
    /// Strokes each path; a closed path joins its last point to its first, an open one ends
    /// in caps at both. The cap, the join, and the dashes are those of MapServer's stroke style,
    /// its `strokeStyleObj`; how they map onto the rasterizer is the renderer's.
    Stroke {
        paths: Vec<Vec<Point>>,
        closed: bool,
        color: Rgba,
        width: f64,
        cap: LineCap,
        join: LineJoin,
        dashes: Dashes,
    },
    /// Fills the part of the hatch's stroked lines that lies inside its rings.
    Hatch(Hatch),
    /// Draws a label's glyph outlines as MapServer's AGG renderer draws text (ADR-0028), their
    /// curves flattened: first its outline, where it has one, then its fill under the non-zero
    /// rule, where it has a colour.
    Glyphs {
        path: Vec<PathCommand>,
        color: Option<Rgba>,
        outline: Option<GlyphOutline>,
    },
}

/// The outline of a label's glyphs: the contour of their path at `width` pixels, filled in
/// `color`, `agg2RenderGlyphsPath`'s `conv_contour` of width `ow + 1`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlyphOutline {
    pub color: Rgba,
    pub width: f64,
}

/// How far, in pixels, a point of a glyph path may lie outside the image: agg-rust holds
/// coordinates in 24.8 fixed point, which a farther point would overflow. The text path leaves out
/// a glyph that reaches farther, and the reference renderer draws no path that does.
pub const MAX_GLYPH_REACH: f64 = 4_194_304.0;

/// A step of a glyph outline, in pixels; a quadratic curve names its control point and its end,
/// a cubic its two control points and its end.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathCommand {
    MoveTo(Point),
    LineTo(Point),
    QuadTo(Point, Point),
    CubicTo(Point, Point, Point),
    Close,
}

/// The dash pattern of a stroke, MapServer's `pattern` and `patternoffset`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Dashes {
    /// Dash and gap lengths in pixels, alternating; fewer than two draw a solid line.
    pub pattern: Vec<f64>,
    /// How far before the end of the pattern each path starts, in pixels; 0 starts it at its
    /// first dash.
    pub offset: f64,
}

/// A drawing operation and the feature it came from.
#[derive(Clone, Debug, PartialEq)]
pub struct DisplayItem {
    pub op: DrawOp,
    pub feature_id: Option<u64>,
}

/// An image as rows of straight-alpha RGBA bytes, top row first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RgbaImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl RgbaImage {
    /// The colour at a pixel, or `None` outside the image.
    pub fn pixel(&self, x: u32, y: u32) -> Option<Rgba> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let i = (y as usize * self.width as usize + x as usize) * 4;
        let p = &self.pixels[i..i + 4];
        Some(Rgba {
            r: p[0],
            g: p[1],
            b: p[2],
            a: p[3],
        })
    }
}

/// Rasterizes a display list.
pub trait Renderer {
    fn begin(&mut self, width: u32, height: u32, background: Rgba);
    fn draw(&mut self, item: &DisplayItem);
    /// Draws what follows, up to [`Renderer::end_layer`], into a transparent image of its own.
    fn begin_layer(&mut self);
    /// Lays the image begun by [`Renderer::begin_layer`] over the image beneath it, its alpha
    /// scaled by `opacity`, 0 to 255.
    fn end_layer(&mut self, opacity: u8);
    fn finish(self) -> RgbaImage;
}
