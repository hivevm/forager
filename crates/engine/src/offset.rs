// The offset of a style's geometry follows MapServer's msOffsetPolyline() and msOffsetCurve(),
// src/maputil.c of the release pinned in scripts/fetch-msautotest.sh, and the meaning of the
// offsets -99 and -999, MS_STYLE_SINGLE_SIDED_OFFSET and MS_STYLE_DOUBLE_SIDED_OFFSET in
// src/mapserver.h.
// Copyright (c) 1996-2005 Regents of the University of Minnesota.
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

use forager_render::Point;

/// How a style moves the paths it draws, in pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Offset {
    /// A shift by `x` and `y`, `y` downwards.
    Shift { x: f64, y: f64 },
    /// A path parallel to each path at the distance, MapServer's `OFFSET d -99`.
    Parallel(f64),
    /// Two parallel paths, each at half the distance on either side, MapServer's `-999`.
    BothSides(f64),
}

/// The paths moved by `offset`, as msOffsetPolyline() moves them.
pub fn apply(paths: &[Vec<Point>], offset: Offset) -> Vec<Vec<Point>> {
    match offset {
        Offset::Shift { x, y } => paths
            .iter()
            .map(|path| {
                path.iter()
                    .map(|p| Point {
                        x: p.x + x,
                        y: p.y + y,
                    })
                    .collect()
            })
            .collect(),
        Offset::Parallel(distance) => paths.iter().map(|p| curve(p, distance)).collect(),
        Offset::BothSides(distance) => {
            let mut both: Vec<_> = paths.iter().map(|p| curve(p, distance / 2.0)).collect();
            both.extend(paths.iter().map(|p| curve(p, -distance / 2.0)));
            both
        }
    }
}

/// Below this sine of the turn at a vertex, msOffsetCurve() places the corner by the average of
/// the two offset directions instead of dividing by the sine (`CURVE_SIN_LIMIT`).
const CURVE_SIN_LIMIT: f64 = 0.3;

/// The path parallel to `path` at `distance`, as msOffsetCurve() computes it without GEOS: each
/// vertex moves along the normal `(-dy, dx)` of the segments it joins, so a positive distance
/// lies below a path drawn rightwards. A path of fewer than two points has none.
fn curve(path: &[Point], distance: f64) -> Vec<Point> {
    if path.len() < 2 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(path.len());
    let mut old = path[0];
    let mut previous: Option<(Point, Point)> = None;
    for &pt in &path[1..] {
        let dir = norm(diff(pt, old));
        let off = rotate90(dir);
        let corner = match previous {
            None => sum(old, scale(off, distance)),
            Some((old_dir, old_off)) => {
                let sin_curve = cross(dir, old_dir);
                let cos_curve = cross(old_off, dir);
                if -CURVE_SIN_LIMIT < sin_curve && sin_curve < CURVE_SIN_LIMIT {
                    sum(old, scale(sum(off, old_off), 0.5 * distance))
                } else {
                    let base_shift = -(1.0 + cos_curve) / sin_curve;
                    sum(old, scale(sum(scale(dir, base_shift), off), distance))
                }
            }
        };
        out.push(corner);
        old = pt;
        previous = Some((dir, off));
    }
    if let Some((_, off)) = previous {
        out.push(sum(old, scale(off, distance)));
    }
    out
}

fn diff(a: Point, b: Point) -> Point {
    Point {
        x: a.x - b.x,
        y: a.y - b.y,
    }
}

fn sum(a: Point, b: Point) -> Point {
    Point {
        x: a.x + b.x,
        y: a.y + b.y,
    }
}

fn scale(a: Point, by: f64) -> Point {
    Point {
        x: a.x * by,
        y: a.y * by,
    }
}

/// The vector of length 1 along `a`; a vector of length 0 gives NaN, as in MapServer, and the
/// renderer drops the point.
fn norm(a: Point) -> Point {
    scale(a, 1.0 / (a.x * a.x + a.y * a.y).sqrt())
}

fn rotate90(a: Point) -> Point {
    Point { x: -a.y, y: a.x }
}

fn cross(a: Point, b: Point) -> f64 {
    a.x * b.y - a.y * b.x
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    fn close(a: &[Point], b: &[Point]) -> bool {
        a.len() == b.len()
            && a.iter()
                .zip(b)
                .all(|(a, b)| (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9)
    }

    #[test]
    fn a_shift_moves_every_point() {
        let moved = apply(
            &[vec![p(0.0, 0.0), p(10.0, 0.0)]],
            Offset::Shift { x: 2.0, y: -3.0 },
        );
        assert!(close(&moved[0], &[p(2.0, -3.0), p(12.0, -3.0)]));
    }

    #[test]
    fn a_parallel_line_lies_on_the_normal_side_with_its_corner_mitred() {
        // Rightwards, then downwards in pixels: the normal (-dy, dx) points down, then left.
        let path = vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0)];
        let moved = apply(&[path], Offset::Parallel(2.0));
        assert!(
            close(&moved[0], &[p(0.0, 2.0), p(8.0, 2.0), p(8.0, 10.0)]),
            "{:?}",
            moved
        );
    }

    #[test]
    fn a_gentle_turn_takes_the_average_of_the_normals() {
        // A turn whose sine is below 0.3 is not mitred.
        let path = vec![p(0.0, 0.0), p(10.0, 0.0), p(20.0, 1.0)];
        let moved = apply(&[path], Offset::Parallel(2.0));
        let n = (101.0_f64).sqrt();
        let second = rotate90(p(10.0 / n, 1.0 / n));
        let corner = p(10.0 + (second.x + 0.0) * 1.0, 0.0 + (second.y + 1.0) * 1.0);
        assert!(close(&moved[0][1..2], &[corner]), "{:?}", moved);
    }

    #[test]
    fn both_sides_draw_two_lines_at_half_the_distance() {
        let moved = apply(&[vec![p(0.0, 0.0), p(10.0, 0.0)]], Offset::BothSides(6.0));
        assert_eq!(moved.len(), 2);
        assert!(close(&moved[0], &[p(0.0, 3.0), p(10.0, 3.0)]));
        assert!(close(&moved[1], &[p(0.0, -3.0), p(10.0, -3.0)]));
    }

    #[test]
    fn a_path_of_one_point_has_no_parallel() {
        let moved = apply(&[vec![p(1.0, 1.0)]], Offset::Parallel(2.0));
        assert!(moved[0].is_empty());
    }
}
