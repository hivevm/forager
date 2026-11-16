use std::rc::Rc;

use agg_rust::pattern_filters_rgba::PatternFilterBilinearRgba;
use forager_render::{DisplayItem, Draw, DrawOp, Renderer, RgbaImage};
use forager_style_ir::Rgba;

use super::*;

fn rgba8(r: u8, g: u8, b: u8, a: u8) -> Rgba8 {
    Rgba8 { r, g, b, a }
}

#[test]
fn the_filter_rounds_as_agg_2_4_s_bilinear_filter_rounds() {
    // Verifies: ADR-draft-symbol-line-brushed-by-its-tile
    let buf = vec![
        vec![rgba8(1, 10, 0, 255), rgba8(2, 20, 0, 255)],
        vec![rgba8(2, 0, 0, 255), rgba8(2, 0, 0, 255)],
    ];
    let mut p = Rgba8::default();
    // The middle of the four pixels, each weighted 128 × 128 = 16384, and half of 65536 added
    // before the shift: red (32768 + 16384 × (1 + 2 + 2 + 2)) >> 16 = 147456 >> 16 = 2, of 2.25;
    // green (32768 + 16384 × 30) >> 16 = 524288 >> 16 = 8, of 7.5 rounded up; the alpha stays 255,
    // (32768 + 65536 × 255) >> 16.
    BilinearRgba8::pixel_high_res(&buf, &mut p, 128, 128);
    assert_eq!(p, rgba8(2, 8, 0, 255));
    // A quarter along the top row: weights 192 × 256 = 49152 and 64 × 256 = 16384; green
    // (32768 + 49152 × 10 + 16384 × 20) >> 16 = 851968 >> 16 = 13, of 12.5.
    BilinearRgba8::pixel_high_res(&buf, &mut p, 64, 0);
    assert_eq!(p.g, 13);
    // agg-rust's own filter truncates the same sums without the half: 1, 7, and 12.
    let mut truncated = Rgba8::default();
    PatternFilterBilinearRgba::pixel_high_res(&buf, &mut truncated, 128, 128);
    assert_eq!((truncated.r, truncated.g), (1, 7));
    PatternFilterBilinearRgba::pixel_high_res(&buf, &mut truncated, 64, 0);
    assert_eq!(truncated.g, 12);
}

#[test]
fn a_coordinate_is_saturated_then_rounded_half_to_even() {
    // Verifies: ADR-draft-symbol-line-brushed-by-its-tile
    assert_eq!(line_coord_sat(0.5 / 256.0), 0);
    assert_eq!(line_coord_sat(1.5 / 256.0), 2);
    assert_eq!(line_coord_sat(-2.5 / 256.0), -2);
    assert_eq!(line_coord_sat(10.0), 2560);
    assert_eq!(line_coord_sat(1e9), LINE_MAX_COORD);
    assert_eq!(line_coord_sat(-1e300), -LINE_MAX_COORD);
}

#[test]
fn the_filter_drops_a_vertex_within_384_subpixels_and_at_the_end_the_one_before_the_last() {
    // Verifies: ADR-draft-symbol-line-brushed-by-its-tile
    // B lies 256 from A: AGG keeps A, C, and D, and sums 2560 for A to C, not 256 + 2573.
    let line = [(0, 0), (0, 256), (2560, 0), (2560, 2560)];
    assert_eq!(filtered(&line), [(0, 2560), (2, 2560), (3, 0)]);
    let (at, left) = starts(&line, 100);
    // B, dropped, takes A's start plus its distance from A.
    assert_eq!(at, [100, 356, 2660, 5220]);
    assert_eq!(left, 5220);
    // The last lies 40 from the one before it: `close` keeps the last in its place.
    assert_eq!(
        filtered(&[(0, 0), (2560, 0), (2600, 0)]),
        [(0, 2600), (2, 0)]
    );
    // Nothing to draw of a path that ends within 384 of where it starts.
    assert_eq!(filtered(&[(0, 0), (300, 0)]), []);
    assert_eq!(starts(&[(0, 0), (300, 0)], 7).1, 7);
}

/// A square path of 100 by 50 pixels in subpixels, and AGG's start at each of its vertices.
fn square() -> ([(i32, i32); 4], Vec<i64>) {
    let vertices = [(0, 0), (25600, 0), (25600, 12800), (0, 12800)];
    let (at, _) = starts(&vertices, 0);
    assert_eq!(at, [0, 25600, 38400, 64000]);
    (vertices, at)
}

#[test]
fn a_piece_starts_at_agg_s_start_of_its_first_whole_segment_less_its_fragment() {
    // Verifies: ADR-draft-symbol-line-brushed-by-its-tile
    let (vertices, at) = square();
    // Cut 6400 into the second segment: 38400 - 6400, so that the fragment brings V2 to 38400.
    let piece = [(25600, 6400), (25600, 12800), (0, 12800)];
    assert_eq!(piece_start(&piece, 1, false, &vertices, &at), Some(32000));
    // A fragment of 200, which the filter drops V2 after: the piece's first segment runs to V3,
    // 25601 long, so the piece starts at 64000 - 25601 and reaches V3 at AGG's 64000.
    let piece = [(25600, 12600), (25600, 12800), (0, 12800)];
    assert_eq!(piece_start(&piece, 1, false, &vertices, &at), Some(38399));
    // One segment cut at both ends: V1's start plus the 3200 from V1 to the piece's first point.
    let piece = [(25600, 3200), (25600, 9600)];
    assert_eq!(piece_start(&piece, 1, true, &vertices, &at), Some(28800));
    // A piece too short for the filter draws nothing.
    let piece = [(25600, 3200), (25600, 3300)];
    assert_eq!(piece_start(&piece, 1, true, &vertices, &at), None);
}

#[test]
fn a_line_within_the_reach_is_left_whole() {
    // Verifies: ADR-draft-symbol-line-brushed-by-its-tile
    let rect = Rect::around_image(40, 20, 5.0);
    let line = vec![p(-6.0, 3.0), p(20.0, 10.0), p(46.0, -6.5)];
    let (vertices, at) = {
        let v: Vec<(i32, i32)> = line
            .iter()
            .map(|q| (line_coord_sat(q.x), line_coord_sat(q.y)))
            .collect();
        let (at, _) = starts(&v, 0);
        (v, at)
    };
    let cut = pieces(&line, &rect);
    assert_eq!(
        cut,
        [Piece {
            points: line,
            first_segment: 0,
            cut_end: false,
        }]
    );
    assert_eq!(piece_start(&vertices, 0, false, &vertices, &at), Some(0));
}

fn p(x: f64, y: f64) -> Point {
    Point { x, y }
}

/// A tile of 7 by 5 pixels, each column a colour of its own, so that the pattern's place along
/// the line shows in every pixel.
fn striped() -> Rc<Tile> {
    let mut pixels = Vec::new();
    for _ in 0..5 {
        for x in 0..7u8 {
            pixels.extend([x * 36, 255 - x * 36, 40, 255]);
        }
    }
    let image = Rc::new(PremultipliedImage::new(7, 5, pixels).unwrap());
    let mark = DrawOp::Pixmap {
        image,
        left: 0,
        top: 0,
    };
    Rc::new(Tile::new(7, 5, vec![mark]).unwrap())
}

fn brushed(width: u32, height: u32, paths: Vec<Vec<Point>>, reach: f64) -> RgbaImage {
    let mut renderer = AggRenderer::new();
    renderer.begin(width, height, Rgba::opaque(255, 255, 255));
    renderer.draw(&DisplayItem {
        op: DrawOp::BrushedLine {
            paths,
            tile: striped(),
            reach,
        },
        feature_id: None,
    });
    renderer.finish()
}

#[test]
fn a_line_is_brushed_with_its_tile_as_wide_as_the_tile_is_high() {
    // Verifies: ADR-draft-symbol-line-brushed-by-its-tile
    let image = brushed(40, 20, vec![vec![p(2.0, 10.0), p(37.0, 10.0)]], 3.0);
    let white = Some(Rgba::opaque(255, 255, 255));
    // Within the tile's height about the line the pixels are the tile's; beyond it, untouched.
    for x in 5..35 {
        assert_ne!(image.pixel(x, 10), white, "on the line at {x}");
        assert_eq!(image.pixel(x, 14), white, "past its width at {x}");
        assert_eq!(image.pixel(x, 6), white, "past its width at {x}");
    }
    // The columns along the line repeat every 7 pixels.
    assert_eq!(image.pixel(10, 10), image.pixel(17, 10));
    assert_ne!(image.pixel(10, 10), image.pixel(11, 10));
}

#[test]
fn a_line_that_leaves_the_image_and_comes_back_draws_the_whole_line_s_pixels_on_it() {
    // Verifies: ADR-draft-symbol-line-brushed-by-its-tile
    // Out over the top edge, along far above, and back in: the reach of 3 cuts the second and
    // the fourth segment, and the last runs along y = 30 well inside.
    let line = vec![
        p(5.0, 10.0),
        p(35.0, 10.0),
        p(35.0, -100.3),
        p(20.0, -100.3),
        p(20.0, 30.0),
        p(4.0, 30.0),
    ];
    let cut = brushed(40, 40, vec![line.clone()], 3.0);
    // The reach of 200 leaves the line whole, as MapServer draws a line it does not clip.
    let whole = brushed(40, 40, vec![line], 200.0);
    assert_ne!(cut, brushed(40, 40, Vec::new(), 3.0));
    for y in 26..35 {
        for x in 5..16 {
            assert_eq!(cut.pixel(x, y), whole.pixel(x, y), "at {x}, {y}");
        }
    }
    for y in 6..15 {
        for x in 6..30 {
            assert_eq!(cut.pixel(x, y), whole.pixel(x, y), "at {x}, {y}");
        }
    }
}

#[test]
fn the_start_carries_over_the_paths_of_a_shape() {
    // Verifies: ADR-draft-symbol-line-brushed-by-its-tile
    // The second path starts where the first, 10 pixels long, left the pattern, so its pixel at x
    // shows the pattern where the first path's does at x - 8, or a whole repeat of 7 from there.
    let image = brushed(
        60,
        30,
        vec![
            vec![p(2.0, 8.0), p(12.0, 8.0)],
            vec![p(20.0, 20.0), p(55.0, 20.0)],
        ],
        3.0,
    );
    for x in 25..40 {
        let first = 4 + (x - 8 - 4) % 7;
        assert_eq!(image.pixel(x, 20), image.pixel(first, 8), "at {x}");
    }
}

#[test]
fn a_path_whose_start_passes_i32_draws_without_a_panic() {
    // Verifies: ADR-draft-symbol-line-brushed-by-its-tile, Q-3
    // 90 segments of 100,000 pixels each: 9,000,000 pixels, past the 8,388,607 the start holds in
    // subpixels. The last segment crosses the image.
    let mut line: Vec<Point> = (0..90)
        .map(|i| p(if i % 2 == 0 { -50_000.0 } else { 50_000.0 }, 10.0))
        .collect();
    line.push(p(10.0, 10.0));
    let image = brushed(20, 20, vec![line], 1e6);
    assert_ne!(image.pixel(10, 10), Some(Rgba::opaque(255, 255, 255)));
}

#[test]
fn hostile_coordinates_brush_without_a_panic() {
    // Verifies: Q-3
    let paths = vec![
        vec![p(f64::NAN, 0.0), p(1e300, -1e300), p(5.0, 5.0)],
        vec![p(f64::INFINITY, 3.0), p(-1e12, 7.0), p(1e12, 7.5)],
        vec![p(3.0, 3.0)],
        Vec::new(),
    ];
    for reach in [0.0, 3.0, f64::NAN, 1e300] {
        brushed(20, 20, paths.clone(), reach);
    }
}
