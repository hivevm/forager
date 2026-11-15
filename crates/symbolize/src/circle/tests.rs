use forager_render::{DrawOp, Point};
use forager_style_ir::{Bound, Coord, Extent, Geometry, Rgba, Style};

use super::*;
use crate::canvas::Canvas;
use crate::extent::View;
use crate::symbol::Factors;

fn c(x: f64, y: f64) -> Coord {
    Coord { x, y }
}

/// A view of the map 0 0 10 10 in an image of 11 × 11: one map unit a pixel, y down from 10.
fn unit_view() -> View {
    let extent = Extent {
        min_x: 0.0,
        min_y: 0.0,
        max_x: 10.0,
        max_y: 10.0,
    };
    View::for_image(extent, 11, 11).unwrap()
}

fn black() -> Style {
    Style {
        color: Some(Bound::Value(Rgba::opaque(0, 0, 0))),
        ..Style::default()
    }
}

/// The ring of the one fill of `ops`.
fn filled(ops: &[DrawOp]) -> &[Point] {
    match ops {
        [DrawOp::Fill { rings, .. }] => &rings[0],
        other => panic!("not one fill: {other:?}"),
    }
}

#[test]
fn a_shape_of_one_part_of_two_points_is_a_circle_on_their_midpoint() {
    let circle_of = |geometry: Geometry| circle(&geometry);
    let two = vec![c(10.0, 10.0), c(50.0, 50.0)];
    let expected = Some(Circle {
        centre: c(30.0, 30.0),
        radius: 20.0,
    });
    assert_eq!(circle_of(Geometry::MultiPoint(two.clone())), expected);
    assert_eq!(circle_of(Geometry::LineString(two.clone())), expected);
    assert_eq!(
        circle_of(Geometry::MultiLineString(vec![two.clone()])),
        expected
    );
    assert_eq!(circle_of(Geometry::Polygon(vec![two.clone()])), expected);
    assert_eq!(circle_of(Geometry::MultiPolygon(vec![vec![two]])), expected);
    // The radius is half the distance in x, whatever the distance in y.
    let wide = circle_of(Geometry::LineString(vec![c(0.0, 0.0), c(4.0, 100.0)]));
    assert_eq!(wide.map(|c| c.radius), Some(2.0));
    // Where the two share their x, it is half the distance in y.
    let tall = circle_of(Geometry::LineString(vec![c(1.0, 0.0), c(1.0, -6.0)]));
    assert_eq!(
        tall,
        Some(Circle {
            centre: c(1.0, -3.0),
            radius: 3.0
        })
    );
}

#[test]
fn any_other_shape_and_a_radius_of_0_are_no_circle() {
    let two = vec![c(0.0, 0.0), c(2.0, 2.0)];
    for geometry in [
        Geometry::Point(c(1.0, 1.0)),
        Geometry::MultiPoint(vec![c(0.0, 0.0)]),
        Geometry::LineString(vec![c(0.0, 0.0), c(2.0, 2.0), c(4.0, 4.0)]),
        Geometry::MultiLineString(vec![two.clone(), two.clone()]),
        Geometry::Polygon(vec![two.clone(), two.clone()]),
        Geometry::MultiPolygon(vec![vec![two.clone()], vec![two.clone()]]),
        Geometry::Collection(vec![Geometry::LineString(two)]),
        Geometry::LineString(vec![c(3.0, 3.0), c(3.0, 3.0)]),
        Geometry::LineString(vec![c(f64::NAN, 0.0), c(2.0, 2.0)]),
        Geometry::LineString(vec![c(0.0, 0.0), c(f64::INFINITY, 2.0)]),
    ] {
        assert_eq!(circle(&geometry), None, "{geometry:?}");
    }
}

#[test]
fn the_centre_is_the_nearest_pixel_half_to_even_and_the_radius_is_in_pixels() {
    // MS_MAP2IMAGE_X and MS_MAP2IMAGE_Y round by lrint, half to even: 2.5 to 2, 7.5 to 8.
    let view = unit_view();
    let mut canvas = Canvas::new(&[], 11, 11, Factors::new(1.0, 1.0));
    let one = Circle {
        centre: c(2.5, 2.5),
        radius: 2.0,
    };
    let ops = circle_ops(one, &black(), &view, &mut canvas);
    let ring = filled(&ops);
    // The first vertex lies at the angle 0, the radius to the right of the centre (2, 8).
    assert_eq!(ring[0], Point { x: 4.0, y: 8.0 });
    assert_eq!(ring[ring.len() - 1], ring[0]);
    for p in ring {
        let d = ((p.x - 2.0).powi(2) + (p.y - 8.0).powi(2)).sqrt();
        assert!((d - 2.0).abs() < 1e-12, "{p:?}");
    }
}

#[test]
fn the_ring_takes_agg_s_steps_and_closes() {
    // AGG's arc at an approximation scale of 1 steps by 2·acos(r / (r + 0.125)): for a radius of
    // 4, about 0.4986, so 13 steps from 0 reach 2π less a quarter step, and the end is the 14th
    // vertex, the closing one the 15th.
    let ring = rasterize_arc(Point { x: 0.0, y: 0.0 }, 4.0);
    let da = libm::acos(4.0 / 4.125) * 2.0;
    let steps = (0..)
        .take_while(|&k| f64::from(k) * da < 2.0 * PI - da / 4.0)
        .count();
    assert_eq!(steps, 13);
    assert_eq!(ring.len(), steps + 2);
    assert_eq!(
        ring[1],
        Point {
            x: libm::cos(da) * 4.0,
            y: libm::sin(da) * 4.0
        }
    );
    assert_eq!(ring[ring.len() - 1], ring[0]);
}

#[test]
fn a_ring_takes_at_most_its_bound_of_steps() {
    let ring = rasterize_arc(Point { x: 0.0, y: 0.0 }, 1e12);
    assert!(
        ring.len() <= MAX_ELLIPSE_STEPS as usize + 2,
        "{}",
        ring.len()
    );
    assert!(ring.len() >= MAX_ELLIPSE_STEPS as usize, "{}", ring.len());
}

#[test]
fn a_radius_past_any_double_in_pixels_draws_nothing() {
    let view = unit_view();
    let mut canvas = Canvas::new(&[], 11, 11, Factors::new(1.0, 1.0));
    let huge = Circle {
        centre: c(5.0, 5.0),
        radius: f64::MAX,
    };
    let tiny = View {
        cellsize: 1e-10,
        ..view
    };
    assert!(circle_ops(huge, &black(), &tiny, &mut canvas).is_empty());
}
