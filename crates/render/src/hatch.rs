//! The hatch of the display list and the bounds on its cost (Q-9, ADR-0032).

use std::collections::BTreeSet;

use forager_style_ir::Rgba;

use crate::Point;
use crate::clip::{CLIP_MARGIN, Rect, drawn_width, segment};

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
    /// The gamma the cut stroke's coverage passes through, as [`crate::DrawOp::Fill`]'s.
    pub gamma: Option<f64>,
}

impl Hatch {
    // The costs below are measured on a release build on one core, over a 4096 × 4096 image at
    // 45°, a pixel wide, each at its bound; the peaks include the image's 64 MB. The most costly
    // hatch they let through takes some 1 s at a peak of 530 MB.

    /// The longest the lines of a hatch may be in all over the image, in pixels (Q-9): the
    /// rasterizer holds a cell for every pixel an edge of the cut stroke crosses. A hatch a pixel
    /// apart over the whole image lays some 1.7e7 pixels and peaks at 730 MB; this allows half.
    /// At the bound, a hatch 2.01 pixels apart over such an image, some 8.36e6 pixels, takes
    /// 1.0 s at a peak of 530 MB; a hatch a pixel apart fits over 2900 × 2900.
    pub const MAX_LENGTH: f64 = 8_388_608.0;
    /// The most dashes and gaps the lines of a hatch may be cut into (Q-9). Each dash is a piece
    /// the cut crosses, so [`Self::MAX_CROSSINGS`] binds first: at its bound, the hatch 9 apart,
    /// dashed 3 on, 3 off, some 6.2e5 dashes and gaps, takes 0.5 s at a peak of 320 MB.
    pub const MAX_DASHES: f64 = 1_048_576.0;
    /// The most vertices of the polygon a hatch is cut from (Q-9): i_overlay's cost grows with
    /// their square where many long edges lie close, as in a star of 8192 spikes, which alone
    /// takes 0.6 s to cut, and one of 16 384 spikes 2.4 s. The count of the polygon's own
    /// crossings gives up on such a star first, and the hatch is not drawn.
    pub const MAX_VERTICES: usize = 16_384;
    /// The most crossings the cut may have to find, as [`Self::crossings`] estimates those of
    /// the stroke and a sixteenth of the polygon's own counts (Q-9). At the bound, a comb of 2000
    /// teeth, 8003 vertices, under a hatch 11 apart takes 0.8 s at a peak of 470 MB, and a star
    /// of 500 points, each edge crossing nearly every other, some 1.2e5 times, under one line,
    /// 0.8 s at a peak of 110 MB. Lines closer than their width cost the most for their count:
    /// 1287 pieces of lines 0.013 pixels apart under a stroke a pixel wide take 1.1 s, at some
    /// 3.5e6 by their weight of 32 past the bound; lines 0.0005 pixels apart, which i_overlay
    /// would take minutes to cut, far past it.
    pub const MAX_CROSSINGS: f64 = 2_097_152.0;

    /// The dash and gap lengths drawn: pairs of the pattern, its odd last length dropped.
    pub fn dashes(&self) -> &[f64] {
        &self.pattern[..self.pattern.len() & !1]
    }

    /// Whether the hatch can be drawn at a bounded cost on an image of `width` × `height`
    /// pixels (Q-9), its stroke at the width the renderer draws: the parts of its lines inside
    /// [`Self::line_rect`] no longer than [`Self::MAX_LENGTH`] in all; its dashes, if any, each a
    /// finite length of at least 0, adding up to more than 0 a period, and no more than
    /// [`Self::MAX_DASHES`] along those parts, as AGG's dasher walks a pattern dash by dash and
    /// never ends on a period of 0; its polygon of no more than [`Self::MAX_VERTICES`]; and no
    /// more than [`Self::MAX_CROSSINGS`] crossings of the two, with those of the polygon with
    /// itself at sixteen each.
    pub fn is_bounded(&self, width: u32, height: u32) -> bool {
        let rect = self.line_rect(width, height);
        let visible: Vec<f64> = self
            .lines
            .iter()
            .map(|&[a, b]| {
                segment(a, b, &rect).map_or(0.0, |(s, e)| {
                    // IEEE's square root, exact on every target, where `hypot` is the
                    // platform's (Q-2).
                    let (dx, dy) = (e.x - s.x, e.y - s.y);
                    (dx * dx + dy * dy).sqrt()
                })
            })
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
        if vertices > Self::MAX_VERTICES {
            return false;
        }
        let with_stroke = self.crossings(width, height, pieces, vertices as f64);
        // What is left of the bound for the polygon's own crossings, each of which costs the cut
        // some sixteen of the others.
        let left = (Self::MAX_CROSSINGS - with_stroke) / SELF_CROSSING_WEIGHT;
        left >= 0.0 && (self_crossings(&self.rings, left as usize) as f64) <= left
    }

    /// The width the renderer strokes the lines at on an image of `width` × `height` pixels.
    pub fn drawn_width(&self, width: u32, height: u32) -> f64 {
        drawn_width(self.width.abs(), width, height)
    }

    /// The rectangle the renderer clips the lines to before it cuts them: the image enlarged by
    /// the margin and half the width drawn.
    pub fn line_rect(&self, width: u32, height: u32) -> Rect {
        Rect::around_image(
            width,
            height,
            CLIP_MARGIN + self.drawn_width(width, height) / 2.0,
        )
    }

    /// An estimate of the crossings the cut has to find on an image of `width` × `height`
    /// pixels, of the polygon's edges with the lines stroked at the width the renderer draws and
    /// of the stroke with itself: where the lines run parallel, the stroke bands each
    /// edge reaches across, found among the lines sorted by their distance from the origin, twice
    /// each; four for every piece, and 32 for every piece and every other line whose band
    /// overlaps its own, at most, where the lines lie closer than their width and the ends of
    /// their pieces, along the edge of the image, run over each other, which costs i_overlay
    /// some eight times a crossing; and two for every piece and every vertex besides. Where they
    /// do not run parallel, four for every piece and edge and for every two pieces. Lines and
    /// points that are not finite count nothing; the renderer draws none.
    pub fn crossings(&self, width: u32, height: u32, pieces: f64, vertices: f64) -> f64 {
        let half = self.drawn_width(width, height) / 2.0;
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
        2.0 * bands + 4.0 * pieces + 32.0 * pieces * others + 2.0 * pieces + 2.0 * vertices
    }
}

/// What a crossing of the polygon with itself costs the cut, in crossings of the polygon with the
/// stroke: a star of 500 points, each edge crossing nearly every other, some 1.2e5 crossings,
/// takes 0.8 s to cut under a single line.
const SELF_CROSSING_WEIGHT: f64 = 16.0;

/// How many cells of its grid an edge passes through, and pairs of edges a cell compares, the
/// count of a polygon's own crossings may take before it gives up (Q-9).
const SELF_CROSSING_WORK: usize = 16_777_216;

/// The pairs of edges of the rings that meet, other than an edge and the next of its own ring,
/// counted up to one past `limit`; one past it, too, where counting would take more than
/// [`SELF_CROSSING_WORK`] steps. The edges are walked through a grid of about one cell per edge
/// over the rings' bounds, and the edges of each cell compared, each pair once.
fn self_crossings(rings: &[Vec<Point>], limit: usize) -> usize {
    let too_many = limit.saturating_add(1);
    let rings = simple_rings(rings);
    let edges: Vec<(usize, usize, Point, Point)> = rings
        .iter()
        .enumerate()
        .flat_map(|(r, ring)| {
            (0..ring.len()).map(move |k| (r, k, ring[k], ring[(k + 1) % ring.len()]))
        })
        .filter(|(_, _, a, b)| {
            a.x.is_finite() && a.y.is_finite() && b.x.is_finite() && b.y.is_finite() && a != b
        })
        .collect();
    if edges.len() < 2 {
        return 0;
    }
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );
    for (_, _, a, b) in &edges {
        for p in [a, b] {
            min_x = min_x.min(p.x);
            min_y = min_y.min(p.y);
            max_x = max_x.max(p.x);
            max_y = max_y.max(p.y);
        }
    }
    #[expect(clippy::cast_possible_truncation, reason = "at most 1024")]
    let side = ((edges.len() as f64).sqrt().ceil() as usize).clamp(1, 1024);
    let (cell_w, cell_h) = (
        ((max_x - min_x) / side as f64).max(f64::MIN_POSITIVE),
        ((max_y - min_y) / side as f64).max(f64::MIN_POSITIVE),
    );
    let column = |x: f64| (((x - min_x) / cell_w).floor().max(0.0) as usize).min(side - 1);
    let row = |y: f64| (((y - min_y) / cell_h).floor().max(0.0) as usize).min(side - 1);
    let mut work = 0usize;
    let mut cells: Vec<Vec<u32>> = vec![Vec::new(); side * side];
    for (index, (_, _, a, b)) in (0u32..).zip(&edges) {
        // The cells the edge passes through, column by column of its x-range: in each, the rows
        // between where it enters and where it leaves.
        let (c0, c1) = (column(a.x.min(b.x)), column(a.x.max(b.x)));
        for c in c0..=c1 {
            let (x0, x1) = (
                (min_x + c as f64 * cell_w).max(a.x.min(b.x)),
                (min_x + (c + 1) as f64 * cell_w).min(a.x.max(b.x)),
            );
            let y_at = |x: f64| {
                if b.x == a.x {
                    None
                } else {
                    Some(a.y + (x - a.x) / (b.x - a.x) * (b.y - a.y))
                }
            };
            let (r0, r1) = match (y_at(x0), y_at(x1)) {
                (Some(y0), Some(y1)) => (row(y0.min(y1)), row(y0.max(y1))),
                _ => (row(a.y.min(b.y)), row(a.y.max(b.y))),
            };
            for r in r0..=r1 {
                work += 1;
                if work > SELF_CROSSING_WORK {
                    return too_many;
                }
                cells[r * side + c].push(index);
            }
        }
    }
    let mut met: BTreeSet<(u32, u32)> = BTreeSet::new();
    for cell in &cells {
        for (n, &i) in cell.iter().enumerate() {
            for &j in &cell[n + 1..] {
                work += 1;
                if work > SELF_CROSSING_WORK {
                    return too_many;
                }
                let (e, f) = (edges[i as usize], edges[j as usize]);
                let next = |a: (usize, usize, Point, Point), b: (usize, usize, Point, Point)| {
                    a.0 == b.0 && (a.1 + 1) % rings[a.0].len() == b.1
                };
                if next(e, f) || next(f, e) || !segments_meet(e.2, e.3, f.2, f.3) {
                    continue;
                }
                met.insert((i.min(j), i.max(j)));
                if met.len() > limit {
                    return too_many;
                }
            }
        }
    }
    met.len()
}

/// Each ring without repeated consecutive points, nor its closing point.
fn simple_rings(rings: &[Vec<Point>]) -> Vec<Vec<Point>> {
    rings
        .iter()
        .map(|ring| {
            let mut kept: Vec<Point> = Vec::with_capacity(ring.len());
            for &p in ring {
                if kept.last() != Some(&p) {
                    kept.push(p);
                }
            }
            while kept.len() > 1 && kept.first() == kept.last() {
                kept.pop();
            }
            kept
        })
        .collect()
}

/// The side of the line through `a` and `b` that `p` lies on: 1 left, -1 right, 0 on it.
fn side(a: Point, b: Point, p: Point) -> i8 {
    let cross = (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
    if cross > 0.0 {
        1
    } else if cross < 0.0 {
        -1
    } else {
        0
    }
}

/// Whether `p` lies within the bounds of the segment from `a` to `b`.
fn within(a: Point, b: Point, p: Point) -> bool {
    a.x.min(b.x) <= p.x && p.x <= a.x.max(b.x) && a.y.min(b.y) <= p.y && p.y <= a.y.max(b.y)
}

/// Whether the segments from `a` to `b` and from `p` to `q` cross, touch, or overlap.
fn segments_meet(a: Point, b: Point, p: Point, q: Point) -> bool {
    let (d1, d2) = (side(p, q, a), side(p, q, b));
    let (d3, d4) = (side(a, b, p), side(a, b, q));
    if d1 * d2 < 0 && d3 * d4 < 0 {
        return true;
    }
    (d1 == 0 && within(p, q, a))
        || (d2 == 0 && within(p, q, b))
        || (d3 == 0 && within(a, b, p))
        || (d4 == 0 && within(a, b, q))
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
            gamma: None,
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
    fn a_hatch_wider_than_the_renderer_draws_is_bounded_at_the_width_drawn() {
        // Verifies: Q-9
        // A stroke ten million pixels wide is drawn at twice the image's width and height, 400
        // pixels: the line across counts 100 + 2 × 202 pixels, not ten million.
        let wide = Hatch {
            width: 1e7,
            ..hatch(vec![[p(-1e12, 5.0), p(1e12, 5.0)]], vec![])
        };
        assert!(wide.is_bounded(100, 100));
    }

    #[test]
    fn lines_with_both_ends_far_out_count_the_part_the_renderer_draws() {
        // Verifies: Q-9
        // A line from -1e150 to 1e150 meets the margin's edges at t = 0.5 either way, rounded;
        // the renderer still draws its 106 pixels, so 100 000 of them are past the bound.
        let far = vec![[p(-1e150, 5.0), p(1e150, 5.0)]; 100_000];
        assert!(!hatch(far, vec![]).is_bounded(100, 100));
    }

    fn star(points: usize, skip: usize) -> Vec<Point> {
        (0..points)
            .map(|i| {
                let t = ((i * skip) % points) as f64 / points as f64 * std::f64::consts::TAU;
                p(50.0 + 40.0 * t.cos(), 50.0 + 40.0 * t.sin())
            })
            .collect()
    }

    #[test]
    fn the_polygon_s_own_crossings_are_counted_up_to_a_limit() {
        // A pentagram crosses itself 5 times; a closed square, its last point its first, none;
        // a square and one over it twice; a ring given twice: each edge meets its twin and the
        // twins of its two neighbours, 12 pairs.
        assert_eq!(self_crossings(&[star(5, 2)], 100), 5);
        let square = vec![
            p(0.0, 0.0),
            p(10.0, 0.0),
            p(10.0, 10.0),
            p(0.0, 10.0),
            p(0.0, 0.0),
        ];
        assert_eq!(self_crossings(std::slice::from_ref(&square), 100), 0);
        let other = vec![p(5.0, 5.0), p(15.0, 5.0), p(15.0, 15.0), p(5.0, 15.0)];
        assert_eq!(self_crossings(&[square.clone(), other], 100), 2);
        assert_eq!(self_crossings(&[square.clone(), square], 100), 12);
        // Counting stops one past the limit.
        assert_eq!(self_crossings(&[star(501, 249)], 1000), 1001);
    }

    #[test]
    fn a_polygon_crossing_itself_everywhere_is_refused() {
        // Verifies: Q-9
        // A star of 3001 points, each edge crossing nearly every other, some 4.5e6 times, under a
        // single line: i_overlay would take minutes and gigabytes to split it.
        let line = vec![[p(0.0, 50.0), p(100.0, 50.0)]];
        let crossed = Hatch {
            rings: vec![star(3001, 1499)],
            ..hatch(line.clone(), vec![])
        };
        assert!(!crossed.is_bounded(100, 100));
        let simple = Hatch {
            rings: vec![star(3001, 1)],
            ..hatch(line, vec![])
        };
        assert!(simple.is_bounded(100, 100));
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
        assert_eq!(bounded.crossings(100, 100, 10.0, 4.0), 104.0);
        assert!(bounded.is_bounded(100, 100));
        // Lines not parallel count four crossings for each piece and vertex, and each two pieces.
        let mut crossed = lines;
        crossed.push([p(0.0, 0.0), p(100.0, 100.0)]);
        let skewed = Hatch {
            rings: vec![square.clone()],
            ..hatch(crossed, vec![])
        };
        assert_eq!(skewed.crossings(100, 100, 11.0, 4.0), 660.0);
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
        assert!(crowded.crossings(100, 100, 2000.0, 3.0) > 3e7);
        assert!(!crowded.is_bounded(100, 100));
        let many = Hatch {
            rings: vec![square; Hatch::MAX_VERTICES / 4 + 1],
            ..bounded
        };
        assert!(!many.is_bounded(100, 100));
    }
}
