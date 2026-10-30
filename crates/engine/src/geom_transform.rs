// The geometry transforms of a style follow MapServer's msDrawTransformedShape, calcOrientation,
// and calcMidAngle, src/mapgeomtransform.c, the shape msDrawShape hands them, src/mapdraw.c,
// transformed to pixels by msTransformShapeSimplify, src/mapprimitive.cpp (the transform mode of
// the AGG renderer, src/mapagg.cpp), and msGetPolygonCentroid, src/mapprimitive.cpp, of the
// release pinned in scripts/fetch-msautotest.sh.
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

//! The points at which a style with a `GEOMTRANSFORM` draws its symbol on a line or a polygon,
//! with the angle `ANGLE AUTO` turns it to. Its trigonometry is libm's, the same code on every
//! target (Q-2).

use forager_ports::{Coord, Geometry};
use forager_render::Point;
use forager_style_ir::GeomTransform;

use crate::extent::View;

/// MapServer's `MS_RAD_TO_DEG`, `src/mapserver.h`, as short as it is written there.
const RAD_TO_DEG: f64 = 57.295_779_51;

/// MapServer's `MS_PI2`, `src/mapserver.h`: 1.57079632679489661923, which rounds to the same
/// double.
const PI_2: f64 = std::f64::consts::FRAC_PI_2;

/// A point a transform draws at, and the angle in degrees `ANGLE AUTO` gives the symbol there;
/// `None` where the transform sets none.
pub(crate) type Placed = (Point, Option<f64>);

/// The points at which a style transformed by `transform` draws its symbol on `geometry`, a
/// shape of a line or a polygon layer, as msDrawTransformedShape places them on the unclipped
/// shape in pixels; `image` is the image's width and height. `labelpoint` and `labelpoly` draw
/// nothing there, as msDrawTransformedShape draws nothing for them.
pub(crate) fn placed(
    transform: GeomTransform,
    geometry: &Geometry,
    view: &View,
    image: (f64, f64),
) -> Vec<Placed> {
    let lines = shape_in_pixels(geometry, view);
    let inside = |p: &Point| !(p.x < 0.0 || p.x > image.0 || p.y < 0.0 || p.y > image.1);
    let mut placed = Vec::new();
    match transform {
        GeomTransform::Start => {
            for line in &lines {
                let p = line[0];
                if inside(&p) {
                    placed.push((p, line.get(1).map(|next| orientation(p, *next))));
                }
            }
        }
        GeomTransform::End => {
            for line in &lines {
                let p = line[line.len() - 1];
                if inside(&p) {
                    let before = line.len().checked_sub(2).map(|i| line[i]);
                    placed.push((p, before.map(|before| orientation(before, p))));
                }
            }
        }
        GeomTransform::Vertices => {
            for line in &lines {
                for i in 1..line.len().saturating_sub(1) {
                    let p = line[i];
                    if inside(&p) {
                        placed.push((p, Some(mid_angle(line[i - 1], p, line[i + 1]))));
                    }
                }
            }
        }
        GeomTransform::Centroid => {
            if let Some(c) = centroid(&lines) {
                placed.push((c, None));
            }
        }
        GeomTransform::LabelPoint | GeomTransform::LabelPoly => {}
    }
    placed
}

/// The parts of a line or a polygon shape in pixels, as msTransformShapeSimplify gives them, the
/// parts it leaves without points dropped: a polygon's rings when the geometry is a polygon, its
/// lines otherwise.
fn shape_in_pixels(geometry: &Geometry, view: &View) -> Vec<Vec<Point>> {
    let parts: Vec<&Vec<Coord>> = match geometry {
        Geometry::LineString(line) => vec![line],
        Geometry::MultiLineString(lines) | Geometry::Polygon(lines) => lines.iter().collect(),
        Geometry::MultiPolygon(polygons) => polygons.iter().flatten().collect(),
        Geometry::Point(_) | Geometry::MultiPoint(_) | Geometry::Collection(_) => Vec::new(),
    };
    let polygon = matches!(geometry, Geometry::Polygon(_) | Geometry::MultiPolygon(_));
    parts
        .into_iter()
        .map(|part| {
            if polygon {
                simplified_ring(part, view)
            } else {
                simplified_line(part, view)
            }
        })
        .filter(|part| !part.is_empty())
        .collect()
}

/// Whether msTransformShapeSimplify keeps `p` after `kept`: when it lies more than a pixel away.
fn far(kept: Point, p: Point) -> bool {
    let (dx, dy) = (p.x - kept.x, p.y - kept.y);
    dx * dx + dy * dy > 1.0
}

/// msTransformShapeSimplify of a line: its first point, each further one more than a pixel from
/// the one kept before it, and its last unless that equals the one kept before; nothing for a
/// line left with fewer than two points.
fn simplified_line(line: &[Coord], view: &View) -> Vec<Point> {
    if line.len() < 2 {
        return Vec::new();
    }
    let mut kept = vec![view.point_to_pixel(line[0])];
    for &c in &line[1..line.len() - 1] {
        let p = view.point_to_pixel(c);
        if far(kept[kept.len() - 1], p) {
            kept.push(p);
        }
    }
    let last = view.point_to_pixel(line[line.len() - 1]);
    if last != kept[kept.len() - 1] {
        kept.push(last);
    }
    if kept.len() < 2 { Vec::new() } else { kept }
}

/// msTransformShapeSimplify of a ring: its first two points, each further one but the last two
/// more than a pixel from the one kept before it, and its last two; nothing for a ring of fewer
/// than four points.
fn simplified_ring(ring: &[Coord], view: &View) -> Vec<Point> {
    if ring.len() < 4 {
        return Vec::new();
    }
    let mut kept = vec![view.point_to_pixel(ring[0]), view.point_to_pixel(ring[1])];
    let before_last = ring.len() - 2;
    for &c in &ring[2..before_last] {
        let p = view.point_to_pixel(c);
        if far(kept[kept.len() - 1], p) {
            kept.push(p);
        }
    }
    kept.push(view.point_to_pixel(ring[before_last]));
    kept.push(view.point_to_pixel(ring[before_last + 1]));
    kept
}

/// calcOrientation: the direction from `p1` to `p2` in degrees, counter-clockwise on the image.
fn orientation(p1: Point, p2: Point) -> f64 {
    let theta = libm::atan2(p2.x - p1.x, p2.y - p1.y);
    RAD_TO_DEG * (theta - PI_2)
}

/// calcMidAngle: the direction of the sum of the segments `p1`–`p2` and `p2`–`p3`, the first
/// scaled to the length of the second.
fn mid_angle(p1: Point, p2: Point, p3: Point) -> f64 {
    let (dx12, dy12) = (p2.x - p1.x, p2.y - p1.y);
    let l12 = (dx12 * dx12 + dy12 * dy12).sqrt();
    let (dx23, dy23) = (p3.x - p2.x, p3.y - p2.y);
    let l23 = (dx23 * dx23 + dy23 * dy23).sqrt();
    let p1n = if l12 > 0.0 {
        Point {
            x: p2.x - dx12 * (l23 / l12),
            y: p2.y - dy12 * (l23 / l12),
        }
    } else {
        p2
    };
    orientation(p1n, p3)
}

/// msGetPolygonCentroid: the centre of the shape's segments weighted by their lengths; `None`
/// where they have no length.
fn centroid(lines: &[Vec<Point>]) -> Option<Point> {
    let (mut x, mut y, mut total) = (0.0, 0.0, 0.0);
    for line in lines {
        for pair in line.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let len = (dx * dx + dy * dy).sqrt();
            x += len * ((a.x + b.x) / 2.0);
            y += len * ((a.y + b.y) / 2.0);
            total += len;
        }
    }
    (total != 0.0).then(|| Point {
        x: x / total,
        y: y / total,
    })
}

#[cfg(test)]
mod tests {
    use forager_style_ir::Extent;

    use super::*;

    /// A view of 0..100 on 101 × 101 pixels: a map unit is a pixel, y downwards from 100.
    fn view() -> View {
        View::for_image(
            Extent {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 100.0,
                max_y: 100.0,
            },
            101,
            101,
        )
        .unwrap()
    }

    const IMAGE: (f64, f64) = (101.0, 101.0);

    fn line(points: &[(f64, f64)]) -> Geometry {
        Geometry::LineString(points.iter().map(|&(x, y)| Coord { x, y }).collect())
    }

    fn p(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    /// An L from pixel (10, 20) right to (90, 20), then down to (90, 90).
    fn ell() -> Geometry {
        line(&[(10.0, 80.0), (90.0, 80.0), (90.0, 10.0)])
    }

    #[test]
    fn start_and_end_draw_at_the_ends_of_a_line_turned_with_its_end_segments() {
        // calcOrientation: atan2(dx, dy) − π/2 in degrees; rightwards is 0, downwards on the
        // image −90 (MapServer's angles turn counter-clockwise).
        let start = placed(GeomTransform::Start, &ell(), &view(), IMAGE);
        assert_eq!(start, vec![(p(10.0, 20.0), Some(0.0))]);
        let end = placed(GeomTransform::End, &ell(), &view(), IMAGE);
        assert_eq!(end.len(), 1);
        assert_eq!(end[0].0, p(90.0, 90.0));
        assert!((end[0].1.unwrap() - (-90.0)).abs() < 1e-6, "{end:?}");
    }

    #[test]
    fn vertices_draw_at_every_inner_vertex_turned_halfway() {
        // The corner of the L bisects rightwards (0) and downwards (−90).
        let drawn = placed(GeomTransform::Vertices, &ell(), &view(), IMAGE);
        assert_eq!(drawn.len(), 1);
        assert_eq!(drawn[0].0, p(90.0, 20.0));
        assert!((drawn[0].1.unwrap() - (-45.0)).abs() < 1e-6, "{drawn:?}");
    }

    #[test]
    fn a_point_outside_the_image_draws_nothing() {
        let leaving = line(&[(-20.0, 50.0), (50.0, 50.0), (150.0, 50.0)]);
        assert!(placed(GeomTransform::Start, &leaving, &view(), IMAGE).is_empty());
        assert!(placed(GeomTransform::End, &leaving, &view(), IMAGE).is_empty());
        assert_eq!(
            placed(GeomTransform::Vertices, &leaving, &view(), IMAGE),
            vec![(p(50.0, 50.0), Some(0.0))]
        );
    }

    #[test]
    fn vertices_less_than_a_pixel_apart_are_simplified_away() {
        // msTransformShapeSimplify drops (10.5, 50) beside (10, 50) and keeps (11.5, 50); a last
        // point equal to the one kept before it is dropped.
        let dense = line(&[
            (10.0, 50.0),
            (10.5, 50.0),
            (11.5, 50.0),
            (30.0, 50.0),
            (30.0, 50.0),
        ]);
        let drawn = placed(GeomTransform::Vertices, &dense, &view(), IMAGE);
        assert_eq!(drawn, vec![(p(11.5, 50.0), Some(0.0))]);
        let end = placed(GeomTransform::End, &dense, &view(), IMAGE);
        assert_eq!(end[0].0, p(30.0, 50.0));
    }

    #[test]
    fn a_line_simplified_to_one_point_draws_nothing() {
        let short = line(&[(10.0, 50.0), (10.0, 50.0)]);
        assert!(placed(GeomTransform::Start, &short, &view(), IMAGE).is_empty());
    }

    #[test]
    fn the_centroid_weighs_the_segments_by_their_length() {
        // msGetPolygonCentroid: a square ring's is its centre; a ring of a long and a short side
        // leans towards the long one.
        let square = Geometry::Polygon(vec![
            [
                (10.0, 10.0),
                (50.0, 10.0),
                (50.0, 50.0),
                (10.0, 50.0),
                (10.0, 10.0),
            ]
            .map(|(x, y)| Coord { x, y })
            .to_vec(),
        ]);
        assert_eq!(
            placed(GeomTransform::Centroid, &square, &view(), IMAGE),
            vec![(p(30.0, 70.0), None)]
        );
        let ell = placed(GeomTransform::Centroid, &ell(), &view(), IMAGE);
        // Segments of 80 and 70 with midpoints (50, 20) and (90, 55).
        let expected = p(
            (80.0 * 50.0 + 70.0 * 90.0) / 150.0,
            (80.0 * 20.0 + 70.0 * 55.0) / 150.0,
        );
        assert_eq!(ell, vec![(expected, None)]);
        // Outside the image, the centroid draws all the same.
        let far_away = line(&[(200.0, 50.0), (300.0, 50.0)]);
        assert_eq!(
            placed(GeomTransform::Centroid, &far_away, &view(), IMAGE),
            vec![(p(250.0, 50.0), None)]
        );
    }

    #[test]
    fn a_ring_keeps_its_first_two_and_last_two_points() {
        // A ring of five points: the second (10.2, 10) is kept although it lies within a pixel.
        let ring = vec![
            [
                (10.0, 10.0),
                (10.2, 10.0),
                (50.0, 10.0),
                (50.0, 50.0),
                (10.0, 10.0),
            ]
            .map(|(x, y)| Coord { x, y })
            .to_vec(),
        ];
        let drawn = placed(
            GeomTransform::Vertices,
            &Geometry::Polygon(ring),
            &view(),
            IMAGE,
        );
        let points: Vec<Point> = drawn.iter().map(|(p, _)| *p).collect();
        assert_eq!(points, vec![p(10.2, 90.0), p(50.0, 90.0), p(50.0, 50.0)]);
    }

    #[test]
    fn label_transforms_draw_nothing_on_a_shape() {
        for transform in [GeomTransform::LabelPoint, GeomTransform::LabelPoly] {
            assert!(placed(transform, &ell(), &view(), IMAGE).is_empty());
        }
    }
}
