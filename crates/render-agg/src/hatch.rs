// How a hatch's lines are dashed, stroked, and cut to the polygon follows msHatchPolygon(),
// src/mapagg.cpp, and the dashing of the AGG it bundles, of the release pinned in
// scripts/fetch-msautotest.sh.
// Copyright (c) 1996-2005 Regents of the University of Minnesota.
// Copyright (c) 1996-2007 Regents of the University of Minnesota (src/mapagg.cpp).
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

//! The geometry of a hatch (ADR-0015): its lines cut into dashes and stroked as rectangles, and
//! the polygon cut to each rectangle. MapServer's msHatchPolygon strokes the dashed lines with
//! butt caps and intersects the stroke with the polygon by its polygon clipper, both read under
//! the even-odd rule. A stroked dash of a straight line is a rectangle; each ring is cut to it by
//! moving every point of the ring onto the rectangle's nearest point, which keeps how often the
//! ring winds around every point inside. Filled together, the cut rings give the intersection
//! where no two rings cross or overlap and no ring crosses itself: then a ring and a hole cut to
//! the same rectangle, wound opposite ways, cancel along its edges in the rasterizer's cells.
//! Where rings cross or overlap, their edges along a rectangle would add up instead, and the
//! renderer intersects the coverage of the stroke and of the polygon.

use forager_render::Point;

use crate::clip::{Rect, clip_line};

/// A dash of a line stroked with butt caps: the rectangle from `from` to `to` along the line
/// from `start` in the direction `along`, a unit vector, `half` to either side.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Piece {
    start: Point,
    along: Point,
    from: f64,
    to: f64,
    half: f64,
}

impl Piece {
    /// The point `u` along the line and `v` across it, to the left of the direction.
    fn at(&self, u: f64, v: f64) -> Point {
        Point {
            x: self.start.x + self.along.x * u - self.along.y * v,
            y: self.start.y + self.along.y * u + self.along.x * v,
        }
    }

    /// How far along the line and across it `p` lies.
    fn frame(&self, p: Point) -> (f64, f64) {
        let (dx, dy) = (p.x - self.start.x, p.y - self.start.y);
        (
            dx * self.along.x + dy * self.along.y,
            dy * self.along.x - dx * self.along.y,
        )
    }

    pub fn corners(&self) -> [Point; 4] {
        [
            self.at(self.from, self.half),
            self.at(self.to, self.half),
            self.at(self.to, -self.half),
            self.at(self.from, -self.half),
        ]
    }

    /// The point of the rectangle nearest to the point `u`, `v` of its frame.
    fn clamp(&self, u: f64, v: f64) -> Point {
        self.at(u.clamp(self.from, self.to), v.clamp(-self.half, self.half))
    }
}

/// The length of a vector, by IEEE's exact square root, as AGG's calc_distance takes it, so that
/// every target computes the same (Q-2).
fn length(dx: f64, dy: f64) -> f64 {
    (dx * dx + dy * dy).sqrt()
}

/// The dashes of the hatch's lines stroked `width` wide with butt caps, each line clipped to
/// `rect` and dashed by `dashes` from its first point on, as AGG's dasher walks the pattern: the
/// dashes of a clipped line keep the phase they had where the line began.
pub fn pieces(lines: &[[Point; 2]], rect: &Rect, dashes: &[f64], width: f64) -> Vec<Piece> {
    let period: f64 = dashes.iter().sum();
    let mut pieces = Vec::new();
    for &[a, b] in lines {
        for run in clip_line(&[a, b], rect) {
            let (start, end) = (run[0], run[run.len() - 1]);
            let run_length = length(end.x - start.x, end.y - start.y);
            if run_length.is_nan() || run_length <= 0.0 {
                continue;
            }
            let along = Point {
                x: (end.x - start.x) / run_length,
                y: (end.y - start.y) / run_length,
            };
            let mut piece = |from: f64, to: f64| {
                if to > from {
                    pieces.push(Piece {
                        start,
                        along,
                        from,
                        to,
                        half: width / 2.0,
                    });
                }
            };
            if dashes.is_empty() {
                piece(0.0, run_length);
                continue;
            }
            // AGG's calc_dash_start: the dash the phase falls in, and how far into it.
            let mut phase = length(start.x - a.x, start.y - a.y) % period;
            let mut dash = 0;
            while phase > 0.0 && phase > dashes[dash] {
                phase -= dashes[dash];
                dash = (dash + 1) % dashes.len();
            }
            let mut at = 0.0;
            let mut rest = dashes[dash] - phase;
            while at < run_length {
                let next = (at + rest).min(run_length);
                if dash % 2 == 0 {
                    piece(at, next);
                }
                at = next;
                dash = (dash + 1) % dashes.len();
                rest = dashes[dash];
            }
        }
    }
    pieces
}

/// The edges of a ring a chunk holds together, from point `first` to point `last` of the ring
/// read round, and their bounds.
struct Chunk {
    first: usize,
    last: usize,
    bounds: Rect,
}

/// How many edges a chunk holds, and how many chunks a group.
const CHUNK: usize = 16;
const GROUP: usize = 16;

/// A ring without repeated consecutive points, its closing point included, its chunks, and its
/// chunks in groups, each the chunks from `first` to `last`, and their bounds.
struct Ring {
    points: Vec<Point>,
    chunks: Vec<Chunk>,
    groups: Vec<Chunk>,
    bounds: Rect,
}

impl Ring {
    fn new(points: &[Point]) -> Self {
        let mut kept: Vec<Point> = Vec::with_capacity(points.len());
        for &p in points {
            if kept.last() != Some(&p) {
                kept.push(p);
            }
        }
        while kept.len() > 1 && kept.first() == kept.last() {
            kept.pop();
        }
        let n = kept.len();
        let chunks = (0..n)
            .step_by(CHUNK)
            .map(|first| {
                let last = (first + CHUNK).min(n);
                let bounds = bounds_of((first..=last).map(|k| kept[k % n]));
                Chunk {
                    first,
                    last,
                    bounds,
                }
            })
            .collect::<Vec<Chunk>>();
        let groups = (0..chunks.len())
            .step_by(GROUP)
            .map(|first| {
                let last = (first + GROUP).min(chunks.len());
                let bounds = chunks[first..last]
                    .iter()
                    .map(|c| c.bounds)
                    .reduce(|a, b| Rect {
                        min_x: a.min_x.min(b.min_x),
                        min_y: a.min_y.min(b.min_y),
                        max_x: a.max_x.max(b.max_x),
                        max_y: a.max_y.max(b.max_y),
                    })
                    .unwrap_or(chunks[first].bounds);
                Chunk {
                    first,
                    last,
                    bounds,
                }
            })
            .collect();
        let bounds = bounds_of(kept.iter().copied());
        Self {
            points: kept,
            chunks,
            groups,
            bounds,
        }
    }

    fn point(&self, k: usize) -> Point {
        self.points[k % self.points.len()]
    }
}

/// The polygon's rings, ready to be cut to pieces: checked for crossings and wound by depth.
pub struct Cutter {
    rings: Vec<Ring>,
}

impl Cutter {
    /// The rings ready to be cut; `None` where two rings cross or overlap or one crosses
    /// itself, or where finding that out or winding them takes more than `budget` steps.
    pub fn new(rings: &[Vec<Point>], budget: usize) -> Option<Self> {
        let mut spent = 0usize;
        let rings: Vec<Ring> = rings
            .iter()
            .map(|r| Ring::new(r))
            .filter(|r| r.points.len() >= 3)
            .collect();
        if crossing(&rings, budget, &mut spent)? {
            return None;
        }
        let rings = oriented(rings, budget, &mut spent)?;
        Some(Self { rings })
    }

    /// The rings cut to each piece, all to be filled under the even-odd rule; `None` when that
    /// takes more than `budget` steps.
    pub fn cut<'a>(
        &self,
        pieces: impl Iterator<Item = &'a Piece>,
        budget: usize,
    ) -> Option<Vec<Vec<Point>>> {
        let mut spent = 0usize;
        let mut cut = Vec::new();
        for piece in pieces {
            let reach = bounds_of(piece.corners().into_iter());
            for ring in self.rings.iter().filter(|r| overlaps(&reach, &r.bounds)) {
                let inside = cut_ring(ring, piece, &mut spent);
                if spent > budget {
                    return None;
                }
                if inside.len() >= 3 {
                    cut.push(inside);
                }
            }
        }
        Some(cut)
    }
}

/// The ring cut to the piece: every point moved onto the piece's nearest point. A group or a
/// chunk wholly beyond one side of the piece moves onto that side's line, where its edges add up
/// to the edge from its first point to its last; any other edge is moved piece by piece between
/// the lines of the piece's sides it crosses.
fn cut_ring(ring: &Ring, piece: &Piece, spent: &mut usize) -> Vec<Point> {
    let mut out = Vec::new();
    for group in &ring.groups {
        *spent += 1;
        let first = ring.chunks[group.first].first;
        if beyond(piece, &group.bounds) {
            let (u, v) = piece.frame(ring.point(first));
            out.push(piece.clamp(u, v));
            continue;
        }
        for chunk in &ring.chunks[group.first..group.last] {
            *spent += 1;
            let first = piece.frame(ring.point(chunk.first));
            if beyond(piece, &chunk.bounds) {
                out.push(piece.clamp(first.0, first.1));
                continue;
            }
            *spent += chunk.last - chunk.first;
            let mut a = first;
            for k in chunk.first..chunk.last {
                let b = piece.frame(ring.point(k + 1));
                cut_edge(&mut out, piece, a, b);
                a = b;
            }
        }
    }
    out
}

/// Whether the bounds lie wholly beyond one side of the piece, on or past the line of that side.
fn beyond(piece: &Piece, b: &Rect) -> bool {
    let (mut u_min, mut u_max, mut v_min, mut v_max) = (
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
    );
    for (x, y) in [
        (b.min_x, b.min_y),
        (b.max_x, b.min_y),
        (b.min_x, b.max_y),
        (b.max_x, b.max_y),
    ] {
        let (u, v) = piece.frame(Point { x, y });
        u_min = u_min.min(u);
        u_max = u_max.max(u);
        v_min = v_min.min(v);
        v_max = v_max.max(v);
    }
    u_max <= piece.from || u_min >= piece.to || v_max <= -piece.half || v_min >= piece.half
}

/// The edge from `a` to `b`, in the piece's frame, moved onto the piece: its first point, and
/// where it crosses the line of a side, without its last point.
fn cut_edge(out: &mut Vec<Point>, piece: &Piece, (ua, va): (f64, f64), (ub, vb): (f64, f64)) {
    out.push(piece.clamp(ua, va));
    let mut crossings: Vec<f64> = Vec::with_capacity(4);
    for (c, a, b) in [
        (piece.from, ua, ub),
        (piece.to, ua, ub),
        (-piece.half, va, vb),
        (piece.half, va, vb),
    ] {
        if (a - c) * (b - c) < 0.0 {
            crossings.push((c - a) / (b - a));
        }
    }
    crossings.sort_by(f64::total_cmp);
    for t in crossings {
        out.push(piece.clamp(ua + (ub - ua) * t, va + (vb - va) * t));
    }
}

/// Whether two edges of the rings cross or touch, other than an edge and the next of its own
/// ring; `None` when finding out takes more than `budget` steps. Chunks are swept in the order of
/// their left edges, and only the edges of chunks whose bounds overlap are compared.
fn crossing(rings: &[Ring], budget: usize, spent: &mut usize) -> Option<bool> {
    let mut chunks: Vec<(usize, &Chunk)> = rings
        .iter()
        .enumerate()
        .flat_map(|(i, r)| r.chunks.iter().map(move |c| (i, c)))
        .collect();
    chunks.sort_by(|a, b| a.1.bounds.min_x.total_cmp(&b.1.bounds.min_x));
    for (n, &(i, c)) in chunks.iter().enumerate() {
        for &(j, d) in &chunks[n..] {
            *spent += 1;
            if *spent > budget {
                return None;
            }
            if d.bounds.min_x > c.bounds.max_x {
                break;
            }
            if !overlaps(&c.bounds, &d.bounds) {
                continue;
            }
            *spent += (c.last - c.first) * (d.last - d.first);
            if *spent > budget {
                return None;
            }
            if chunks_cross(&rings[i], c, &rings[j], d, i == j) {
                return Some(true);
            }
        }
    }
    Some(false)
}

fn chunks_cross(r: &Ring, c: &Chunk, s: &Ring, d: &Chunk, same: bool) -> bool {
    let n = r.points.len();
    for k in c.first..c.last {
        for l in d.first..d.last {
            if same && (k == l || (k + 1) % n == l % n || (l + 1) % n == k % n) {
                continue;
            }
            if same && l < k && d.first == c.first {
                // Each pair within one chunk once.
                continue;
            }
            let (a, b) = (r.point(k), r.point(k + 1));
            let (p, q) = (s.point(l), s.point(l + 1));
            if segments_meet(a, b, p, q) {
                return true;
            }
        }
    }
    false
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

/// The rings wound by how deep each lies among the others, those at an even depth one way and
/// those at an odd depth the other, which leaves the even-odd area as it is: a ring and the hole
/// it holds, cut to the same piece, then have coincident edges that run opposite ways and cancel
/// in the rasterizer's cells; run the same way, they would add up to a cell half covered.
fn oriented(rings: Vec<Ring>, budget: usize, spent: &mut usize) -> Option<Vec<Ring>> {
    let total: usize = rings.iter().map(|r| r.points.len()).sum();
    let mut depths = Vec::with_capacity(rings.len());
    for (i, ring) in rings.iter().enumerate() {
        *spent += total;
        if *spent > budget {
            return None;
        }
        let depth = rings
            .iter()
            .enumerate()
            .filter(|&(j, other)| j != i && contains(&other.points, ring.points[0]))
            .count();
        depths.push(depth);
    }
    Some(
        rings
            .into_iter()
            .zip(depths)
            .map(|(mut ring, depth)| {
                if (signed_area(&ring.points) < 0.0) != (depth % 2 == 1) {
                    ring.points.reverse();
                    ring = Ring::new(&ring.points);
                }
                ring
            })
            .collect(),
    )
}

/// Twice the ring's area, positive where it winds from x towards y.
fn signed_area(ring: &[Point]) -> f64 {
    let n = ring.len();
    (0..n)
        .map(|i| {
            let (a, b) = (ring[i], ring[(i + 1) % n]);
            a.x * b.y - b.x * a.y
        })
        .sum()
}

/// Whether the point lies inside the ring under the even-odd rule.
fn contains(ring: &[Point], p: Point) -> bool {
    let n = ring.len();
    let mut inside = false;
    for i in 0..n {
        let (a, b) = (ring[i], ring[(i + 1) % n]);
        if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) {
            inside = !inside;
        }
    }
    inside
}

pub fn bounds_of(points: impl Iterator<Item = Point>) -> Rect {
    let mut r = Rect {
        min_x: f64::INFINITY,
        min_y: f64::INFINITY,
        max_x: f64::NEG_INFINITY,
        max_y: f64::NEG_INFINITY,
    };
    for p in points {
        r.min_x = r.min_x.min(p.x);
        r.min_y = r.min_y.min(p.y);
        r.max_x = r.max_x.max(p.x);
        r.max_y = r.max_y.max(p.y);
    }
    r
}

fn overlaps(a: &Rect, b: &Rect) -> bool {
    a.min_x <= b.max_x && b.min_x <= a.max_x && a.min_y <= b.max_y && b.min_y <= a.max_y
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    fn area(ring: &[Point]) -> f64 {
        signed_area(ring).abs() / 2.0
    }

    /// The band from y = 4 to y = 6 across x = -5 to 15.
    fn band() -> Piece {
        Piece {
            start: p(-5.0, 5.0),
            along: p(1.0, 0.0),
            from: 0.0,
            to: 20.0,
            half: 1.0,
        }
    }

    /// The rings cut to the pieces, within `budget` steps to get ready and as many to cut.
    fn intersect(rings: &[Vec<Point>], pieces: &[Piece], budget: usize) -> Option<Vec<Vec<Point>>> {
        Cutter::new(rings, budget)?.cut(pieces.iter(), budget)
    }

    fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Point> {
        vec![p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1)]
    }

    #[test]
    fn a_dashed_line_keeps_the_phase_of_its_start_beyond_the_clip() {
        // The line starts 25 before the clip at 0: 25 into a pattern of 10 on, 10 off is 5 into
        // the second dash, so the first dash on the image runs from 0 to 5.
        let rect = Rect {
            min_x: 0.0,
            min_y: -10.0,
            max_x: 40.0,
            max_y: 10.0,
        };
        let pieces = pieces(&[[p(-25.0, 0.0), p(40.0, 0.0)]], &rect, &[10.0, 10.0], 2.0);
        let spans: Vec<(f64, f64)> = pieces
            .iter()
            .map(|q| (q.corners()[0].x, q.corners()[1].x))
            .collect();
        assert_eq!(spans, vec![(0.0, 5.0), (15.0, 25.0), (35.0, 40.0)]);
        let corners = pieces[0].corners();
        assert_eq!((corners[0].y, corners[3].y), (1.0, -1.0));
    }

    #[test]
    fn a_ring_cut_to_a_piece_is_their_intersection() {
        // A 10 × 10 square cut to a band 2 high across it, and a square with a hole: the cut
        // ring and hole, filled even-odd, leave the band without the hole.
        let outer = square(0.0, 0.0, 10.0, 10.0);
        assert_eq!(area(&cut_ring(&Ring::new(&outer), &band(), &mut 0)), 20.0);
        let hole = square(3.0, 3.0, 5.0, 7.0);
        let cut = intersect(&[outer, hole], &[band()], usize::MAX).unwrap();
        assert_eq!(cut.len(), 2);
        assert_eq!(area(&cut[0]) - area(&cut[1]), 16.0);
        // The hole, wound as the square is, is cut wound the other way.
        assert!(signed_area(&cut[0]) * signed_area(&cut[1]) < 0.0);
    }

    #[test]
    fn a_ring_of_many_vertices_is_cut_chunk_by_chunk_to_the_same_area() {
        // A circle of 4000 vertices, radius 10 about (5, 5): the band through its middle.
        let circle: Vec<Point> = (0..4000)
            .map(|i| {
                let t = f64::from(i) / 4000.0 * std::f64::consts::TAU;
                p(5.0 + 10.0 * t.cos(), 5.0 + 10.0 * t.sin())
            })
            .collect();
        let mut spent = 0;
        let cut = cut_ring(&Ring::new(&circle), &band(), &mut spent);
        // Both chords at y = 4 and 6 lie 1 from the centre: the band is near 2 × 2 × 10.
        assert!((area(&cut) - 39.93).abs() < 0.01, "{}", area(&cut));
        // The groups and chunks beyond the band each cost one step.
        assert!(spent < 600, "{spent}");
    }

    #[test]
    fn crossing_or_overlapping_rings_are_found() {
        let a = square(0.0, 0.0, 40.0, 62.0);
        let b = square(20.0, 0.0, 60.0, 62.0);
        let hole = square(10.0, 10.0, 20.0, 20.0);
        fn found(rs: &[&Vec<Point>]) -> Option<bool> {
            let rings: Vec<Ring> = rs.iter().map(|r| Ring::new(r)).collect();
            crossing(&rings, usize::MAX, &mut 0)
        }
        assert_eq!(found(&[&a, &hole]), Some(false));
        assert_eq!(found(&[&a, &b]), Some(true), "two rings that overlap");
        assert_eq!(found(&[&a, &a]), Some(true), "a ring given twice");
        let twice = [a.clone(), a.clone()].concat();
        assert_eq!(found(&[&twice]), Some(true), "a ring wound twice");
        let bow = vec![p(0.0, 0.0), p(10.0, 10.0), p(10.0, 0.0), p(0.0, 10.0)];
        assert_eq!(found(&[&bow]), Some(true), "a ring that crosses itself");
        // The closing point, repeated, is no crossing.
        let closed = [a.clone(), vec![a[0]]].concat();
        assert_eq!(found(&[&closed]), Some(false));
        assert_eq!(intersect(&[a, b], &[band()], usize::MAX), None);
    }

    #[test]
    fn a_ring_far_from_a_piece_costs_nothing_and_a_spent_budget_gives_none() {
        let far = vec![p(100.0, 100.0), p(110.0, 100.0), p(110.0, 110.0)];
        // Looking for crossings costs a step for the one chunk and 3 × 3 for its edges against
        // themselves, winding the ring its 3 vertices: 13 in all.
        assert!(Cutter::new(std::slice::from_ref(&far), 12).is_none());
        let cutter = Cutter::new(&[far], 13).unwrap();
        // Cutting the ring to a far piece costs nothing.
        assert_eq!(cutter.cut([band()].iter(), 0), Some(Vec::new()));
        // Cutting a near ring costs a step for its group, one for its chunk, and one for each of
        // its 3 edges.
        let near = Cutter::new(&[vec![p(0.0, 0.0), p(5.0, 0.0), p(5.0, 5.0)]], 13).unwrap();
        assert_eq!(near.cut([band()].iter(), 4), None);
        assert!(near.cut([band()].iter(), 5).is_some());
    }
}
