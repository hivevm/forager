//! The display list and the renderer trait: what is drawn, in pixel coordinates, independent of
//! the backend that rasterizes it (ADR-0009). Only `style-ir` is known here.

#![warn(clippy::disallowed_types, clippy::disallowed_methods)]

use forager_style_ir::Rgba;
pub use forager_style_ir::{LineCap, LineJoin};

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
    /// Fills a label's glyph outlines under the non-zero rule, their curves flattened, as
    /// MapServer's AGG renderer draws text (ADR-0028).
    Glyphs { path: Vec<PathCommand>, color: Rgba },
}

/// Straight lines stroked with butt caps, each dashed from its first point, and their stroke
/// intersected with the area of the rings, MapServer's hatch fill (`msHatchPolygon`). The rings
/// and the stroke are each read under the even-odd rule, as MapServer's polygon clipper reads
/// them, so where two stroked lines overlap the stroke is empty.
#[derive(Clone, Debug, PartialEq)]
pub struct Hatch {
    pub rings: Vec<Vec<Point>>,
    /// Each line from its first point to its second.
    pub lines: Vec<[Point; 2]>,
    pub color: Rgba,
    pub width: f64,
    /// Dash and gap lengths in pixels, alternating, taken as they are; an odd last length is
    /// dropped, and fewer than two draw solid lines.
    pub pattern: Vec<f64>,
}

impl Hatch {
    /// The longest the lines of a hatch may be in all, in pixels (Q-9). A hatch a pixel apart,
    /// a pixel wide, at 45° over the largest image, 4096 × 4096, lays some 1.7e7 pixels of lines
    /// and stays below it: the reference renderer draws it in some 1.9 s at a peak of 130 MB, the
    /// image's 64 MB included (a release build on one core); the most this allows takes about
    /// twice as long.
    pub const MAX_LENGTH: f64 = 33_554_432.0;
    /// The most dashes and gaps the lines of a hatch may be cut into (Q-9). That hatch dashed 5 on,
    /// 5 off, some 3.4e6 dashes and gaps, takes some 1.9 s at a peak of 270 MB; the most this
    /// allows, some 330 MB.
    pub const MAX_DASHES: f64 = 4_194_304.0;

    /// The dash and gap lengths drawn: pairs of the pattern, its odd last length dropped.
    pub fn dashes(&self) -> &[f64] {
        &self.pattern[..self.pattern.len() & !1]
    }

    /// Whether the hatch can be drawn at a bounded cost on an image of `width` × `height`
    /// pixels (Q-9): the parts of its lines on the image, enlarged by the stroke, no longer than
    /// [`Self::MAX_LENGTH`] in all, and its dashes, if any, each a finite length of at least 0,
    /// adding up to more than 0 a period, and no more than [`Self::MAX_DASHES`] along those
    /// parts. AGG's dasher walks a pattern dash by dash and never ends on a period of 0.
    pub fn is_bounded(&self, width: u32, height: u32) -> bool {
        let margin = self.width.abs() / 2.0 + 2.0;
        let (max_x, max_y) = (f64::from(width) + margin, f64::from(height) + margin);
        let length: f64 = self
            .lines
            .iter()
            .map(|&[a, b]| visible_length(a, b, -margin, -margin, max_x, max_y))
            .sum();
        if length > Self::MAX_LENGTH {
            return false;
        }
        let dashes = self.dashes();
        if dashes.is_empty() {
            return true;
        }
        let period: f64 = dashes.iter().sum();
        dashes.iter().all(|d| d.is_finite() && *d >= 0.0)
            && period > 0.0
            && length / period * dashes.len() as f64 <= Self::MAX_DASHES
    }
}

/// The length of the part of the segment from `a` to `b` inside the rectangle, by Liang–Barsky;
/// 0 for a segment that is not finite.
fn visible_length(a: Point, b: Point, min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    if !(a.x.is_finite() && a.y.is_finite() && dx.is_finite() && dy.is_finite()) {
        return 0.0;
    }
    let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
    for (p, q) in [
        (-dx, a.x - min_x),
        (dx, max_x - a.x),
        (-dy, a.y - min_y),
        (dy, max_y - a.y),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return 0.0;
            }
        } else if p < 0.0 {
            t0 = t0.max(q / p);
        } else {
            t1 = t1.min(q / p);
        }
    }
    if t1 > t0 {
        // IEEE's square root, exact on every target, where `hypot` is the platform's (Q-2).
        (t1 - t0) * (dx * dx + dy * dy).sqrt()
    } else {
        0.0
    }
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
    fn finish(self) -> RgbaImage;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    fn hatch(lines: Vec<[Point; 2]>, pattern: Vec<f64>) -> Hatch {
        Hatch {
            rings: Vec::new(),
            lines,
            color: Rgba::opaque(0, 0, 0),
            width: 2.0,
            pattern,
        }
    }

    #[test]
    fn a_hatch_is_bounded_by_its_lines_on_the_image_and_its_dashes() {
        // Verifies: Q-9
        let across = vec![[p(-1e12, 5.0), p(1e12, 5.0)]];
        // Only the part over the image, 100 + 2 × 3 pixels, counts.
        assert!(hatch(across.clone(), vec![]).is_bounded(100, 100));
        assert!(hatch(across.clone(), vec![2.0, 1.0, 7.0]).is_bounded(100, 100));
        assert!(!hatch(across.clone(), vec![0.0, 0.0]).is_bounded(100, 100));
        assert!(!hatch(across.clone(), vec![-1.0, 3.0]).is_bounded(100, 100));
        assert!(!hatch(across.clone(), vec![f64::NAN, 3.0]).is_bounded(100, 100));
        assert!(!hatch(across, vec![1e-6, 1e-6]).is_bounded(100, 100));
        let many = vec![[p(0.0, 5.0), p(100.0, 5.0)]; 400_000];
        assert!(!hatch(many, vec![]).is_bounded(100, 100));
        assert!(hatch(vec![[p(f64::NAN, 1.0), p(5.0, 1.0)]], vec![]).is_bounded(100, 100));
    }
}
