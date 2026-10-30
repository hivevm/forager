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
    // The costs below are measured on a release build on one core, over a 4096 × 4096 image at
    // 45°, a pixel wide; the peaks include the image's 64 MB. The most costly hatch they let
    // through took some 2 s at a peak of 430 MB.

    /// The longest the lines of a hatch may be in all over the image, in pixels (Q-9): the
    /// rasterizer holds a cell for every pixel an edge of the cut stroke crosses. A hatch a pixel
    /// apart over the whole image lays some 1.7e7 pixels and peaks at 730 MB; this allows half,
    /// a hatch 3 pixels apart over such an image (0.7 s, 380 MB) or a pixel apart over 2900 × 2900.
    pub const MAX_LENGTH: f64 = 8_388_608.0;
    /// The most dashes and gaps the lines of a hatch may be cut into (Q-9). Each dash is a piece
    /// the cut crosses, so [`Self::MAX_CROSSINGS`] binds first: the hatch 10 apart, dashed 3
    /// on, 3 off, some 5.6e5 dashes and gaps, takes 0.5 s at a peak of 300 MB.
    pub const MAX_DASHES: f64 = 1_048_576.0;
    /// The most vertices of the polygon a hatch is cut from (Q-9): i_overlay's cost grows with
    /// their square where many long edges overlap, as in a star of 8192 spikes, which alone
    /// takes 0.6 s to cut, and one of 16 384 spikes 2.4 s.
    pub const MAX_VERTICES: usize = 16_384;
    /// The most crossings of the polygon's edges with the hatch's stroke the cut may have to
    /// find, as [`Self::crossings`] estimates them (Q-9): a comb of 2000 teeth, 8003 vertices,
    /// under a hatch 12 apart, some 1.9e6 of them, takes 0.7 s at a peak of 430 MB; a star of
    /// 8000 spikes under a hatch 30 apart 1.8 s at a peak of 380 MB. Without the term for the
    /// stroke crossing itself, lines 0.0005 pixels apart under a stroke a pixel wide did not end.
    pub const MAX_CROSSINGS: f64 = 2_097_152.0;

    /// The dash and gap lengths drawn: pairs of the pattern, its odd last length dropped.
    pub fn dashes(&self) -> &[f64] {
        &self.pattern[..self.pattern.len() & !1]
    }

    /// Whether the hatch can be drawn at a bounded cost on an image of `width` × `height`
    /// pixels (Q-9): the parts of its lines on the image, enlarged by the stroke, no longer than
    /// [`Self::MAX_LENGTH`] in all; its dashes, if any, each a finite length of at least 0,
    /// adding up to more than 0 a period, and no more than [`Self::MAX_DASHES`] along those
    /// parts, as AGG's dasher walks a pattern dash by dash and never ends on a period of 0; its
    /// polygon of no more than [`Self::MAX_VERTICES`]; and no more than [`Self::MAX_CROSSINGS`]
    /// crossings of the two.
    pub fn is_bounded(&self, width: u32, height: u32) -> bool {
        let margin = self.width.abs() / 2.0 + 2.0;
        let (max_x, max_y) = (f64::from(width) + margin, f64::from(height) + margin);
        let visible: Vec<f64> = self
            .lines
            .iter()
            .map(|&[a, b]| visible_length(a, b, -margin, -margin, max_x, max_y))
            .collect();
        let length: f64 = visible.iter().sum();
        if length > Self::MAX_LENGTH {
            return false;
        }
        let dashes = self.dashes();
        let pieces = if dashes.is_empty() {
            visible.iter().filter(|l| **l > 0.0).count() as f64
        } else {
            let period: f64 = dashes.iter().sum();
            let walked = length / period * dashes.len() as f64;
            if !(dashes.iter().all(|d| d.is_finite() && *d >= 0.0) && period > 0.0)
                || walked > Self::MAX_DASHES
            {
                return false;
            }
            walked / 2.0 + visible.len() as f64
        };
        let vertices: usize = self.rings.iter().map(Vec::len).sum();
        vertices <= Self::MAX_VERTICES
            && self.crossings(pieces, vertices as f64) <= Self::MAX_CROSSINGS
    }

    /// An estimate of the crossings the cut has to find, of the polygon's edges with the stroked
    /// lines and of the stroke with itself: where the lines run parallel, the stroke bands each
    /// edge reaches across, found among the lines sorted by their distance from the origin, twice
    /// each; four for every piece, and sixteen for every piece and every other line whose band
    /// overlaps its own, at most, where the lines lie closer than their width and the ends of
    /// their pieces, along the edge of the image, run over each other, which costs i_overlay
    /// some four times a crossing; and two for every piece and every vertex besides. Where they do not run parallel, four for every
    /// piece and edge and for every two pieces. Lines and points that are not finite count
    /// nothing; the renderer draws none.
    pub fn crossings(&self, pieces: f64, vertices: f64) -> f64 {
        let unit = |[a, b]: [Point; 2]| {
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let length = (dx * dx + dy * dy).sqrt();
            (length > 0.0 && length.is_finite()).then(|| (dx / length, dy / length))
        };
        let directions: Vec<(f64, f64)> = self.lines.iter().filter_map(|l| unit(*l)).collect();
        let Some(&(ux, uy)) = directions.first() else {
            return 0.0;
        };
        // Parallel either way: createHatch() lays some lines from their right ends.
        let parallel = directions
            .iter()
            .all(|&(x, y)| (x * uy - y * ux).abs() < 1e-9);
        if !parallel {
            return 4.0 * pieces * (vertices + pieces);
        }
        // The distance across the lines, along their normal.
        let across = |p: Point| p.y * ux - p.x * uy;
        let mut offsets: Vec<f64> = self
            .lines
            .iter()
            .map(|l| across(l[0]))
            .filter(|o| o.is_finite())
            .collect();
        offsets.sort_by(f64::total_cmp);
        let half = self.width.abs() / 2.0;
        // The most lines whose bands overlap that of one line: within a width of it.
        let mut overlap = 0usize;
        let mut first = 0;
        for (last, &o) in offsets.iter().enumerate() {
            while offsets[first] < o - 2.0 * half {
                first += 1;
            }
            overlap = overlap.max(last - first + 1);
        }
        let mut bands = 0.0;
        for ring in &self.rings {
            for (k, &a) in ring.iter().enumerate() {
                let b = ring[(k + 1) % ring.len()];
                let (oa, ob) = (across(a), across(b));
                if !(oa.is_finite() && ob.is_finite()) {
                    continue;
                }
                let (low, high) = (oa.min(ob) - half, oa.max(ob) + half);
                let first = offsets.partition_point(|o| *o < low);
                let last = offsets.partition_point(|o| *o <= high);
                bands += (last - first) as f64;
            }
        }
        let others = overlap.saturating_sub(1) as f64;
        2.0 * bands + 4.0 * pieces + 16.0 * pieces * others + 2.0 * pieces + 2.0 * vertices
    }
}

/// The length of the part of the segment from `a` to `b` inside the rectangle, by Liang–Barsky;
/// 0 for a segment that is not finite. Where the part lies near `b`, its parameters are measured
/// from `b`: from `a`, they would round to 1 for an `a` far out, and the part to nothing.
fn visible_length(a: Point, b: Point, min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    if !(a.x.is_finite() && a.y.is_finite() && dx.is_finite() && dy.is_finite()) {
        return 0.0;
    }
    // t from `a`, u = 1 - t from `b`, of the entry and of the exit.
    let (mut t0, mut t1, mut u0, mut u1) = (0.0_f64, 1.0_f64, 1.0_f64, 0.0_f64);
    for (p, from_a, from_b) in [
        (-dx, a.x - min_x, b.x - min_x),
        (dx, max_x - a.x, max_x - b.x),
        (-dy, a.y - min_y, b.y - min_y),
        (dy, max_y - a.y, max_y - b.y),
    ] {
        if p == 0.0 {
            if from_a < 0.0 {
                return 0.0;
            }
        } else if p < 0.0 {
            t0 = t0.max(from_a / p);
            u0 = u0.min(-from_b / p);
        } else {
            t1 = t1.min(from_a / p);
            u1 = u1.max(-from_b / p);
        }
    }
    // IEEE's square root, exact on every target, where `hypot` is the platform's (Q-2).
    let length = (dx * dx + dy * dy).sqrt();
    let part = if t0 + t1 <= 1.0 { t1 - t0 } else { u0 - u1 };
    if part > 0.0 { part * length } else { 0.0 }
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
        // A line from 3e125 pixels away whose last pixels cross the image: those count, and its
        // dashes of 1e-277 are too many.
        let far = vec![[p(-3.4e125, 2.1e125), p(0.0, 1.29)]];
        assert!(hatch(far.clone(), vec![]).is_bounded(100, 100));
        assert!(!hatch(far, vec![1.8e-278, 5.8e-277]).is_bounded(100, 100));
    }

    #[test]
    fn a_hatch_is_bounded_by_its_polygon_s_vertices_and_their_crossings() {
        // Verifies: Q-9
        let lines: Vec<[Point; 2]> = (0..10)
            .map(|k| {
                let y = 10.0 * f64::from(k);
                // Every other line from its right end, as createHatch() lays some.
                if k % 2 == 0 {
                    [p(0.0, y), p(100.0, y)]
                } else {
                    [p(100.0, y), p(0.0, y)]
                }
            })
            .collect();
        let square = vec![p(5.0, 5.0), p(95.0, 5.0), p(95.0, 95.0), p(5.0, 95.0)];
        let bounded = Hatch {
            rings: vec![square.clone()],
            ..hatch(lines.clone(), vec![])
        };
        // The two upright edges reach across the bands of lines 1 to 9, the others none: 2 × 18;
        // each line's band overlaps its own alone: 4 for each of the 10 pieces; and 2 for each
        // piece and each of the 4 vertices.
        assert_eq!(bounded.crossings(10.0, 4.0), 104.0);
        assert!(bounded.is_bounded(100, 100));
        // Lines not parallel count four crossings for each piece and vertex, and each two pieces.
        let mut crossed = lines;
        crossed.push([p(0.0, 0.0), p(100.0, 100.0)]);
        let skewed = Hatch {
            rings: vec![square.clone()],
            ..hatch(crossed, vec![])
        };
        assert_eq!(skewed.crossings(11.0, 4.0), 660.0);
        // Lines a thousandth of a pixel apart under a stroke a pixel wide: each band overlaps a
        // thousand others, whose ends the cut would have to cross.
        let close: Vec<[Point; 2]> = (0..2000)
            .map(|k| {
                let y = 50.0 + f64::from(k) / 1000.0;
                [p(-1.0, y), p(101.0, y)]
            })
            .collect();
        let crowded = Hatch {
            rings: vec![vec![p(5.0, 5.0), p(95.0, 5.0), p(95.0, 95.0)]],
            ..hatch(close, vec![])
        };
        assert!(crowded.crossings(2000.0, 3.0) > 3e7);
        assert!(!crowded.is_bounded(100, 100));
        let many = Hatch {
            rings: vec![square; Hatch::MAX_VERTICES / 4 + 1],
            ..bounded
        };
        assert!(!many.is_bounded(100, 100));
    }
}
