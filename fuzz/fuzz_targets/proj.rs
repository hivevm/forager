//! The core's reprojection on arbitrary text: two proj4 strings, as a mapfile or a style document
//! may hold them, and the coordinates of points, a rectangle, and the lines of a shape, each read,
//! reprojected, and cut at the horizon or wrapped at the dateline — an error or a result, never a
//! panic or a hang (Q-3, GEO-proj-ported-with-libm).
//!
//! The input is UTF-8 text: its first line the source projection, its second the target, and the
//! rest numbers separated by white space, taken in pairs as points (at most 256), the first four
//! also as a rectangle. The fifth, where there is one, is the angle of a rotated map whose
//! extent is that rectangle, 400 × 300 pixels: the same points, rectangle, and lines go through
//! the reprojection into that map and its way back, and through the way that runs no formula
//! (GEO-map-rotation-in-reprojection).
// Verifies: Q-3, GEO-proj-ported-with-libm
#![no_main]

use forager_proj::{Geotransform, Projection, Reprojection};
use forager_style_ir::{Coord, Extent};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let mut lines = text.splitn(3, '\n');
    let (Some(from), Some(to)) = (lines.next(), lines.next()) else {
        return;
    };
    let (Ok(from), Ok(to)) = (Projection::parse(from), Projection::parse(to)) else {
        return;
    };
    let numbers: Vec<f64> = lines
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .filter_map(|n| n.parse().ok())
        .take(512)
        .collect();
    let points: Vec<Coord> = numbers
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&[x, y]| Coord { x, y })
        .collect();
    let way = Reprojection::new(from, to);
    exercise(&way, &numbers, &points);
    if let [min_x, min_y, max_x, max_y, angle, ..] = numbers[..] {
        let rect = Extent {
            min_x,
            min_y,
            max_x,
            max_y,
        };
        if let Some(gt) = Geotransform::rotated_map(&rect, 400, 300, angle) {
            let rotated = way.onto_rotated_map(gt);
            exercise(&rotated, &numbers, &points);
            exercise(&rotated.reversed(), &numbers, &points);
            exercise(
                &Reprojection::identity(false, false).onto_rotated_map(gt),
                &numbers,
                &points,
            );
        }
    }
});

/// Every operation of `way` on `points`, and on the rectangle of the first four `numbers`.
fn exercise(way: &Reprojection, numbers: &[f64], points: &[Coord]) {
    for &p in points {
        let _ = way.point(p);
    }
    if let [min_x, min_y, max_x, max_y, ..] = numbers[..] {
        let _ = way.extent(&Extent {
            min_x,
            min_y,
            max_x,
            max_y,
        });
    }
    let _ = way.shape(std::slice::from_ref(&points.to_vec()), false);
    let _ = way.shape(std::slice::from_ref(&points.to_vec()), true);
    let _ = way.cuts_lines_with_geos();
    let mut point_shape = points.to_vec();
    let _ = way.point_line(&mut point_shape);
}
