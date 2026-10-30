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

//! The geometry of a hatch (ADR-0015, ADR-0032): its lines cut into dashes, each stroked with
//! butt caps as a rectangle, and the rectangles intersected with the polygon by i_overlay, as
//! msHatchPolygon intersects them by MapServer's Clipper: the polygon the subject, the stroke the
//! clip, both read under the even-odd rule, and every coordinate rounded to a hundredth of a
//! pixel, Clipper's scaling in `agg_conv_clipper`.

use forager_render::Point;
use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay::Overlay;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::core::solver::{Precision, Solver, Strategy};
use i_overlay::i_float::int::point::IntPoint;

use crate::clip::{Rect, clip_line};

/// agg_conv_clipper's default scaling: 10 to the power of 2.
const SCALE: f64 = 100.0;

/// How i_overlay solves: its fragment strategy, some six times faster than its default on a
/// polygon of many long edges; points snapped only within one unit, a hundredth of a pixel, as
/// near as it comes to Clipper, which snaps none; and on one thread, so that every target
/// computes the same (Q-2).
const SOLVER: Solver = Solver {
    strategy: Strategy::Frag,
    precision: Precision::ABSOLUTE,
    multithreading: None,
};

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

    pub fn corners(&self) -> [Point; 4] {
        [
            self.at(self.from, self.half),
            self.at(self.to, self.half),
            self.at(self.to, -self.half),
            self.at(self.from, -self.half),
        ]
    }
}

/// The length of a vector, by IEEE's exact square root, as AGG's calc_distance takes it, so that
/// every target computes the same (Q-2).
fn length(dx: f64, dy: f64) -> f64 {
    (dx * dx + dy * dy).sqrt()
}

/// The dashes of the hatch's lines stroked `width` wide with butt caps, each line clipped to
/// `rect` and dashed by `dashes` from its first point on, as AGG's dasher walks the pattern: the
/// dashes of a clipped line keep the phase they had where the line began. `None` once the walk
/// takes more than `steps` dashes and gaps (Q-9): a dash too short to move the walk along, so far
/// from the image, would never end it.
pub fn pieces(
    lines: &[[Point; 2]],
    rect: &Rect,
    dashes: &[f64],
    width: f64,
    steps: f64,
) -> Option<Vec<Piece>> {
    let period: f64 = dashes.iter().sum();
    let mut walked = 0.0;
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
                walked += 1.0;
                if walked > steps {
                    return None;
                }
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
    Some(pieces)
}

/// The intersection of the rings, the subject, with the pieces, the clip, both under the
/// even-odd rule, as Clipper's `ctIntersection` with `pftEvenOdd` for both computes it in
/// agg_conv_clipper: each coordinate scaled by 100 and rounded half away from zero, as its
/// `Round` does, and the result scaled back. The contours come out with their holes wound
/// against their outer contours, for the non-zero rule MapServer fills them with.
pub fn intersect(rings: &[Vec<Point>], pieces: &[Piece]) -> Vec<Vec<Point>> {
    let scaled = |points: &mut dyn Iterator<Item = Point>| -> Vec<IntPoint<i32>> {
        let mut contour: Vec<IntPoint<i32>> = Vec::new();
        for p in points {
            // The coordinates lie within the image grown by a margin: far inside i32.
            #[expect(
                clippy::cast_possible_truncation,
                reason = "rounded, within the margin"
            )]
            let q = IntPoint::new((p.x * SCALE).round() as i32, (p.y * SCALE).round() as i32);
            if contour.last() != Some(&q) {
                contour.push(q);
            }
        }
        while contour.len() > 1 && contour.first() == contour.last() {
            contour.pop();
        }
        contour
    };
    let subject: Vec<Vec<IntPoint<i32>>> = rings
        .iter()
        .map(|r| scaled(&mut r.iter().copied()))
        .filter(|c| c.len() >= 3)
        .collect();
    let clip: Vec<Vec<IntPoint<i32>>> = pieces
        .iter()
        .map(|p| scaled(&mut p.corners().into_iter()))
        .filter(|c| c.len() >= 3)
        .collect();
    if subject.is_empty() || clip.is_empty() {
        return Vec::new();
    }
    Overlay::from_subj_and_clip_custom(&subject, &clip, Default::default(), SOLVER)
        .overlay(OverlayRule::Intersect, FillRule::EvenOdd)
        .into_iter()
        .flatten()
        .map(|contour| {
            contour
                .into_iter()
                .map(|q| Point {
                    x: f64::from(q.x) / SCALE,
                    y: f64::from(q.y) / SCALE,
                })
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    /// Twice the area of a contour, positive where it winds from x towards y.
    fn signed_area(ring: &[Point]) -> f64 {
        let n = ring.len();
        (0..n)
            .map(|i| {
                let (a, b) = (ring[i], ring[(i + 1) % n]);
                a.x * b.y - b.x * a.y
            })
            .sum()
    }

    /// The even-odd area of contours wound as i_overlay winds them.
    fn area(contours: &[Vec<Point>]) -> f64 {
        contours.iter().map(|c| signed_area(c)).sum::<f64>().abs() / 2.0
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
        let pieces = pieces(
            &[[p(-25.0, 0.0), p(40.0, 0.0)]],
            &rect,
            &[10.0, 10.0],
            2.0,
            9.0,
        )
        .unwrap();
        let spans: Vec<(f64, f64)> = pieces
            .iter()
            .map(|q| (q.corners()[0].x, q.corners()[1].x))
            .collect();
        assert_eq!(spans, vec![(0.0, 5.0), (15.0, 25.0), (35.0, 40.0)]);
        let corners = pieces[0].corners();
        assert_eq!((corners[0].y, corners[3].y), (1.0, -1.0));
    }

    #[test]
    fn a_walk_of_dashes_past_its_steps_gives_none() {
        // Verifies: Q-9
        // A line from 3e125 pixels away, dashed in lengths of 1e-277: its phase is reduced to a
        // period, but no step moves the walk along a pixel.
        let rect = Rect {
            min_x: -3.0,
            min_y: -3.0,
            max_x: 99.0,
            max_y: 6.0,
        };
        let far = [[p(-3.4e125, 2.1e125), p(0.0, 1.29)]];
        assert_eq!(pieces(&far, &rect, &[1.8e-278, 5.8e-277], 1.0, 1e6), None);
        // The line of 65 pixels above walks 7 dashes and gaps.
        let near = [[p(-25.0, 0.0), p(40.0, 0.0)]];
        let wide = Rect {
            min_x: -30.0,
            min_y: -10.0,
            max_x: 40.0,
            max_y: 10.0,
        };
        assert!(pieces(&near, &wide, &[10.0, 10.0], 2.0, 7.0).is_some());
        assert_eq!(pieces(&near, &wide, &[10.0, 10.0], 2.0, 6.0), None);
    }

    #[test]
    fn the_rings_and_the_pieces_intersect_under_the_even_odd_rule() {
        // Verifies: ADR-0032
        // A square with a hole, cut to a band 2 high across both: the band less the hole.
        let outer = square(0.0, 0.0, 10.0, 10.0);
        let hole = square(3.0, 3.0, 5.0, 7.0);
        assert_eq!(
            area(&intersect(&[outer.clone(), hole.clone()], &[band()])),
            16.0
        );
        // Two rings over each other and a ring given twice: the part covered twice is outside.
        let other = square(5.0, 0.0, 15.0, 10.0);
        assert_eq!(area(&intersect(&[outer.clone(), other], &[band()])), 20.0);
        assert_eq!(
            area(&intersect(&[outer.clone(), outer.clone()], &[band()])),
            0.0
        );
        // Two pieces over each other: there the stroke is empty.
        let half = Piece { to: 10.0, ..band() };
        assert_eq!(area(&intersect(&[outer], &[band(), half])), 10.0);
    }

    #[test]
    fn coordinates_round_to_a_hundredth_of_a_pixel_half_away_from_zero() {
        // Verifies: ADR-0032
        // A band from y = 4.004 to 5.996 and a square from x = 0.125 to 9.875, exact in binary:
        // Clipper's Round makes them 4.00 to 6.00, and 12.5 and 987.5 hundredths 0.13 to 9.88.
        let thin = Piece {
            start: p(-5.0, 5.0),
            half: 0.996,
            ..band()
        };
        let right = square(0.125, 0.0, 9.875, 10.0);
        let cut = intersect(&[right], &[thin]);
        assert!((area(&cut) - 2.0 * 9.75).abs() < 1e-9, "{}", area(&cut));
        // A square from x = -0.125: -12.5 hundredths round away from zero to -13, so it runs
        // from -0.13 to 9.88, 10.01 wide, and the cut is 2 × 10.01; rounded half up, -12 would
        // make it 10.00 wide.
        let left = square(-0.125, 0.0, 9.875, 10.0);
        let cut = intersect(&[left], &[thin]);
        assert!((area(&cut) - 2.0 * 10.01).abs() < 1e-9, "{}", area(&cut));
    }
}
