use forager_render::{DrawOp, Point};
use forager_style_ir::{Bound, Rgba, Style};

use super::*;

const BLUE: Rgba = Rgba::opaque(30, 60, 200);

fn p(x: f64, y: f64) -> Point {
    Point { x, y }
}

fn colored(color: Option<Rgba>, outline: Option<Rgba>) -> Style {
    Style {
        color: color.map(Bound::Value),
        outline_color: outline.map(Bound::Value),
        ..Style::default()
    }
}

/// The half side of the stand-in square [`point_ops`] draws for `style`.
fn half_side(style: &Style) -> f64 {
    let mut ops = Vec::new();
    point_ops(&mut ops, p(0.0, 0.0), style);
    match &ops[0] {
        DrawOp::Fill { rings, .. } => rings[0][1].x,
        _ => panic!("the square is filled first"),
    }
}

#[test]
fn the_stand_in_square_bounds_its_size_by_mapserver_s_defaults_alone() {
    // Forager's stand-in, not MapServer's: its size is cut to between 0 and MS_MAXSYMBOLSIZE,
    // 500, and the style's MINSIZE and MAXSIZE do not count.
    let sized = |size: f64| Style {
        size: Some(Bound::Value(size)),
        ..colored(Some(BLUE), None)
    };
    assert_eq!(half_side(&sized(6.0)), 3.0);
    assert_eq!(half_side(&sized(600.0)), 250.0);
    assert_eq!(half_side(&sized(-4.0)), 0.0);
    let bounded = Style {
        min_size: Some(10.0),
        max_size: Some(2.0),
        ..sized(6.0)
    };
    assert_eq!(half_side(&bounded), 3.0);
}
