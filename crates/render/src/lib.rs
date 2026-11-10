//! The display list and the renderer trait: what is drawn, in pixel coordinates, independent of
//! the backend that rasterizes it (ADR-0008). Only `style-ir` is known here.

#![warn(clippy::disallowed_types, clippy::disallowed_methods)]

use std::rc::Rc;

use forager_style_ir::Rgba;
pub use forager_style_ir::{LineCap, LineJoin};
pub use hatch::Hatch;
pub use tile::Tile;

pub mod clip;
mod hatch;
mod tile;

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
    /// their winding. With a `gamma`, each pixel's coverage passes through AGG's
    /// `gamma_linear(0, gamma)` first, as MapServer's AGG renderer fills a polygon shape through
    /// the gamma of the output format; without one, as it fills a symbol, the coverage is taken
    /// as it is. The renderer takes the gamma as given; the engine chooses it.
    Fill {
        rings: Vec<Vec<Point>>,
        color: Rgba,
        gamma: Option<f64>,
    },
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
    /// Draws a label's glyph outlines as MapServer's AGG renderer draws text (ADR-0031), their
    /// curves flattened: first its outline, where it has one, then its fill under the non-zero
    /// rule, where it has a colour.
    Glyphs {
        path: Vec<PathCommand>,
        color: Option<Rgba>,
        outline: Option<GlyphOutline>,
    },
    /// Blends a pixmap symbol's image unturned and unscaled, its top-left pixel at the whole
    /// pixel (`left`, `top`), every pixel at full cover, as AGG's `blend_from` lays one
    /// premultiplied image over another; what lies outside the image drawn on is left out. The
    /// image is the one the render decoded, shared by every item that draws it, never a copy
    /// (ADR-0029).
    Pixmap {
        image: Rc<PremultipliedImage>,
        left: i32,
        top: i32,
    },
    /// Resamples a pixmap symbol's image into the square of half-side `half` about `at`, as
    /// `agg2RenderPixmapSymbol` draws an image turned or scaled: the square filled under the
    /// non-zero rule without a gamma, each pixel's colour the image sampled at the position
    /// `inverse` takes the pixel's centre to, bilinearly, transparent beyond the image
    /// (ADR-0026). The square is clipped to the image as every path is (ADR-0027). The image is
    /// the one the render decoded, never a copy (ADR-0029).
    ResampledPixmap {
        image: Rc<PremultipliedImage>,
        at: Point,
        half: i32,
        inverse: Affine,
    },
    /// Fills the area the rings enclose under the even-odd rule without a gamma, each pixel's
    /// colour the pixel of `tile` at its position modulo the tile's size, as
    /// `agg2RenderPolygonTiled` repeats a tile from the image's origin. The rings are clipped to
    /// the image as every path is (ADR-0027). The tile is the one the render holds, shared by
    /// every item it fills (ADR-0029).
    TiledFill {
        rings: Vec<Vec<Point>>,
        tile: Rc<Tile>,
    },
}

/// An affine transformation of positions in pixels, AGG's `trans_affine`: it takes (x, y) to
/// (x × `sx` + y × `shx` + `tx`, x × `shy` + y × `sy` + `ty`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine {
    pub sx: f64,
    pub shy: f64,
    pub shx: f64,
    pub sy: f64,
    pub tx: f64,
    pub ty: f64,
}

/// The outline of a label's glyphs: the contour of their path at `width` pixels, filled in
/// `color`, `agg2RenderGlyphsPath`'s `conv_contour` of width `ow + 1`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlyphOutline {
    pub color: Rgba,
    pub width: f64,
}

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

/// An image as rows of premultiplied RGBA bytes, top row first: each colour already multiplied
/// by its alpha, as MapServer's `readPNG` premultiplies a pixmap (ADR-0029). A type apart from
/// [`RgbaImage`], whose alpha is straight, so that the one is never read as the other.
#[derive(Debug, PartialEq, Eq)]
pub struct PremultipliedImage {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl PremultipliedImage {
    /// The image of `width` by `height` pixels whose premultiplied RGBA bytes are `pixels`, or
    /// `None` where they are not four bytes for each pixel.
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Option<Self> {
        let len = (width as usize)
            .checked_mul(height as usize)?
            .checked_mul(4)?;
        (pixels.len() == len).then_some(Self {
            width,
            height,
            pixels,
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// The premultiplied RGBA bytes, row by row from the top.
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }
}

/// Draws display items one by one, onto an image or into a layer of it.
pub trait Draw {
    fn draw(&mut self, item: &DisplayItem);
}

impl<D: Draw + ?Sized> Draw for &mut D {
    fn draw(&mut self, item: &DisplayItem) {
        (**self).draw(item);
    }
}

/// Rasterizes a display list.
pub trait Renderer: Draw {
    /// A layer begun by [`Renderer::layer`]; it borrows the renderer until it is dropped.
    type Layer<'a>: Draw
    where
        Self: 'a;

    fn begin(&mut self, width: u32, height: u32, background: Rgba);
    /// Begins a layer: what is drawn into it goes into a transparent image of its own, which is
    /// laid over the image, its alpha scaled by `opacity`, 0 to 255, when the layer is dropped.
    /// The layer borrows the renderer, so no layer begins within another, and it draws an item
    /// as it is handed, holding none.
    fn layer(&mut self, opacity: u8) -> Self::Layer<'_>;
    fn finish(self) -> RgbaImage;
}
