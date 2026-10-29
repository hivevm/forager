//! The geometry of a hatch (ADR-0015): its lines cut into dashes and stroked as rectangles, and
//! the rectangles intersected with the polygon. MapServer's msHatchPolygon strokes the dashed
//! lines with butt caps and intersects the stroke with the polygon by its polygon clipper; a
//! stroked dash of a straight line is a rectangle, and the polygon cut to a convex rectangle ring
//! by ring keeps its even-odd area inside it, so the pieces read under the even-odd rule are that
//! intersection.

use forager_render::Point;

use crate::clip::{Rect, clip_line};

/// The rectangles of the hatch's lines stroked `width` wide with butt caps, each line clipped to
/// `rect` and dashed by `dashes` from its first point on, as AGG's dasher walks the pattern: the
/// dashes of a clipped line keep the phase they had where the line began.
pub fn pieces(lines: &[[Point; 2]], rect: &Rect, dashes: &[f64], width: f64) -> Vec<[Point; 4]> {
    let period: f64 = dashes.iter().sum();
    let mut pieces = Vec::new();
    for &[a, b] in lines {
        for run in clip_line(&[a, b], rect) {
            let (start, end) = (run[0], run[run.len() - 1]);
            let length = (end.x - start.x).hypot(end.y - start.y);
            if length.is_nan() || length <= 0.0 {
                continue;
            }
            let along = Point {
                x: (end.x - start.x) / length,
                y: (end.y - start.y) / length,
            };
            let mut piece = |from: f64, to: f64| {
                if to > from {
                    pieces.push(rectangle(start, along, from, to, width));
                }
            };
            if dashes.is_empty() {
                piece(0.0, length);
                continue;
            }
            // AGG's calc_dash_start: the dash the phase falls in, and how far into it.
            let mut phase = (start.x - a.x).hypot(start.y - a.y) % period;
            let mut dash = 0;
            while phase > 0.0 && phase > dashes[dash] {
                phase -= dashes[dash];
                dash = (dash + 1) % dashes.len();
            }
            let mut at = 0.0;
            let mut rest = dashes[dash] - phase;
            while at < length {
                let next = (at + rest).min(length);
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

/// The rectangle of the stretch from `from` to `to` along the line from `start` in the direction
/// `along`, `width` wide.
fn rectangle(start: Point, along: Point, from: f64, to: f64, width: f64) -> [Point; 4] {
    let half = width / 2.0;
    let (nx, ny) = (-along.y * half, along.x * half);
    let at = |t: f64| Point {
        x: start.x + along.x * t,
        y: start.y + along.y * t,
    };
    let (p, q) = (at(from), at(to));
    [
        Point {
            x: p.x + nx,
            y: p.y + ny,
        },
        Point {
            x: q.x + nx,
            y: q.y + ny,
        },
        Point {
            x: q.x - nx,
            y: q.y - ny,
        },
        Point {
            x: p.x - nx,
            y: p.y - ny,
        },
    ]
}

/// The rings cut to each piece, all to be filled under the even-odd rule; `None` when that takes
/// more than `budget` vertex steps, a polygon of very many vertices under very many pieces.
pub fn intersect(
    rings: &[Vec<Point>],
    pieces: &[[Point; 4]],
    budget: usize,
) -> Option<Vec<Vec<Point>>> {
    let mut spent = 0usize;
    let rings = oriented(rings, budget, &mut spent)?;
    let bounds: Vec<Rect> = rings.iter().map(|r| bounds(r)).collect();
    let mut cut = Vec::new();
    for piece in pieces {
        let reach = bounds_of(piece);
        for (ring, ring_bounds) in rings.iter().zip(&bounds) {
            if !overlaps(&reach, ring_bounds) {
                continue;
            }
            spent += ring.len() * 4;
            if spent > budget {
                return None;
            }
            let inside = clip_to_quad(ring, piece);
            if inside.len() >= 3 {
                cut.push(inside);
            }
        }
    }
    Some(cut)
}

/// The rings wound by how deep each lies among the others, those at an even depth one way and
/// those at an odd depth the other, which leaves the even-odd area as it is. A ring and the
/// hole it holds then cut to the same piece have coincident edges that run opposite ways and
/// cancel in the rasterizer's cells; run the same way, they would add up to a cell half covered.
fn oriented(rings: &[Vec<Point>], budget: usize, spent: &mut usize) -> Option<Vec<Vec<Point>>> {
    let total: usize = rings.iter().map(Vec::len).sum();
    let mut oriented = Vec::with_capacity(rings.len());
    for (i, ring) in rings.iter().enumerate() {
        *spent += total;
        if *spent > budget {
            return None;
        }
        let depth = rings
            .iter()
            .enumerate()
            .filter(|&(j, other)| j != i && contains(other, ring[0]))
            .count();
        let mut ring = ring.clone();
        if (signed_area(&ring) < 0.0) != (depth % 2 == 1) {
            ring.reverse();
        }
        oriented.push(ring);
    }
    Some(oriented)
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

fn bounds_of(points: &[Point]) -> Rect {
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

fn bounds(ring: &[Point]) -> Rect {
    bounds_of(ring)
}

fn overlaps(a: &Rect, b: &Rect) -> bool {
    a.min_x <= b.max_x && b.min_x <= a.max_x && a.min_y <= b.max_y && b.min_y <= a.max_y
}

/// Sutherland–Hodgman: the ring cut to the convex quadrilateral, edge by edge. Where the ring
/// leaves and re-enters, the cut runs along the quadrilateral's edge, and the edges that come to
/// lie on each other cancel when filled.
fn clip_to_quad(ring: &[Point], quad: &[Point; 4]) -> Vec<Point> {
    // The quadrilateral's turn, so that "inside" is on the same side of every edge.
    let turn = (quad[1].x - quad[0].x) * (quad[2].y - quad[1].y)
        - (quad[1].y - quad[0].y) * (quad[2].x - quad[1].x);
    let sign = if turn < 0.0 { -1.0 } else { 1.0 };
    let mut points = ring.to_vec();
    for i in 0..4 {
        let (e0, e1) = (quad[i], quad[(i + 1) % 4]);
        let side = |p: Point| sign * ((e1.x - e0.x) * (p.y - e0.y) - (e1.y - e0.y) * (p.x - e0.x));
        let mut kept = Vec::with_capacity(points.len() + 4);
        for (j, &current) in points.iter().enumerate() {
            let previous = points[(j + points.len() - 1) % points.len()];
            let (sc, sp) = (side(current), side(previous));
            if (sc >= 0.0) != (sp >= 0.0) {
                let t = sp / (sp - sc);
                kept.push(Point {
                    x: previous.x + (current.x - previous.x) * t,
                    y: previous.y + (current.y - previous.y) * t,
                });
            }
            if sc >= 0.0 {
                kept.push(current);
            }
        }
        points = kept;
        if points.is_empty() {
            break;
        }
    }
    points
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    fn area(ring: &[Point]) -> f64 {
        let n = ring.len();
        (0..n)
            .map(|i| {
                let (a, b) = (ring[i], ring[(i + 1) % n]);
                a.x * b.y - b.x * a.y
            })
            .sum::<f64>()
            .abs()
            / 2.0
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
        let spans: Vec<(f64, f64)> = pieces.iter().map(|q| (q[0].x, q[1].x)).collect();
        assert_eq!(spans, vec![(0.0, 5.0), (15.0, 25.0), (35.0, 40.0)]);
        assert_eq!((pieces[0][0].y, pieces[0][3].y), (1.0, -1.0));
    }

    #[test]
    fn a_ring_cut_to_a_piece_is_their_intersection() {
        // A 10 × 10 square cut to a band 2 high across it, and a square with a hole: the cut
        // ring and hole, filled even-odd, leave the band without the hole.
        let square = vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0), p(0.0, 10.0)];
        let band = [p(-5.0, 4.0), p(15.0, 4.0), p(15.0, 6.0), p(-5.0, 6.0)];
        assert_eq!(area(&clip_to_quad(&square, &band)), 20.0);
        let hole = vec![p(3.0, 3.0), p(5.0, 3.0), p(5.0, 7.0), p(3.0, 7.0)];
        let cut = intersect(&[square, hole], &[band], usize::MAX).unwrap();
        assert_eq!(cut.len(), 2);
        assert_eq!(area(&cut[0]) - area(&cut[1]), 16.0);
        // The hole, wound as the square is, is cut wound the other way.
        assert!(signed_area(&cut[0]) * signed_area(&cut[1]) < 0.0);
    }

    #[test]
    fn a_ring_far_from_a_piece_costs_nothing_and_a_spent_budget_gives_none() {
        let far = vec![p(100.0, 100.0), p(110.0, 100.0), p(110.0, 110.0)];
        let band = [p(0.0, 0.0), p(10.0, 0.0), p(10.0, 2.0), p(0.0, 2.0)];
        // Winding the ring costs its 3 vertices, cutting it to the far piece nothing.
        assert_eq!(intersect(&[far], &[band], 3), Some(Vec::new()));
        let near = vec![p(0.0, 0.0), p(5.0, 0.0), p(5.0, 5.0)];
        assert_eq!(intersect(std::slice::from_ref(&near), &[band], 6), None);
        assert!(intersect(&[near], &[band], 15).is_some());
    }
}
