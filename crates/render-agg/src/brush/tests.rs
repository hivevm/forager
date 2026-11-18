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
    // Verifies: ADR-0046
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
    // Verifies: ADR-0046
    assert_eq!(line_coord_sat(0.5 / 256.0), 0);
    assert_eq!(line_coord_sat(1.5 / 256.0), 2);
    assert_eq!(line_coord_sat(-2.5 / 256.0), -2);
    assert_eq!(line_coord_sat(10.0), 2560);
    assert_eq!(line_coord_sat(1e9), LINE_MAX_COORD);
    assert_eq!(line_coord_sat(-1e300), -LINE_MAX_COORD);
}

#[test]
fn the_filter_drops_a_vertex_within_384_subpixels_and_at_the_end_the_one_before_the_last() {
    // Verifies: ADR-0046
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
    // Verifies: ADR-0046
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
    // Verifies: ADR-0046
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
    // Verifies: ADR-0046
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
    // Verifies: ADR-0046
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
    // Verifies: ADR-0046
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
    // Verifies: ADR-0046, Q-3
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

/// A tile of 1 by 1 pixel: a pattern 256 subpixels wide and a line width of 256 subpixels.
fn dot() -> Rc<Tile> {
    let image = Rc::new(PremultipliedImage::new(1, 1, vec![200, 40, 40, 255]).unwrap());
    let mark = DrawOp::Pixmap {
        image,
        left: 0,
        top: 0,
    };
    Rc::new(Tile::new(1, 1, vec![mark]).unwrap())
}

/// `path` brushed with [`dot`] on a white image of `width` by `height` pixels.
fn dotted(width: u32, height: u32, path: Vec<Point>, reach: f64) -> RgbaImage {
    let mut renderer = AggRenderer::new();
    renderer.begin(width, height, Rgba::opaque(255, 255, 255));
    renderer.draw(&DisplayItem {
        op: DrawOp::BrushedLine {
            paths: vec![path],
            tile: dot(),
            reach,
        },
        feature_id: None,
    });
    renderer.finish()
}

/// How many pixels of `image` are not white.
fn drawn(image: &RgbaImage) -> usize {
    image
        .pixels
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|&&pixel| pixel != [255; 4])
        .count()
}

/// Whether [`JoinProbe`] admits `path`, fed whole, with [`dot`]'s pattern.
fn joins_admit(path: &[Point]) -> bool {
    let pattern = LineImagePattern::<BilinearRgba8>::with_source(&Source(dot().image(built_tile)));
    let mut rasterizer = RasterizerOutlineAa::new();
    let (x, y) = (line_coord_sat(path[0].x), line_coord_sat(path[0].y));
    rasterizer.move_to(x, y);
    for q in &path[1..] {
        rasterizer.line_to(line_coord_sat(q.x), line_coord_sat(q.y));
    }
    let mut probe = JoinProbe::new(&pattern);
    rasterizer.render(&mut probe, false);
    probe.within
}

#[test]
fn a_piece_whose_join_lies_far_past_its_short_segment_is_not_drawn_and_does_not_panic() {
    // Verifies: Q-3
    // Ticket 21's fuzz input: the short first segment's join with the segment of some 8650 pixels
    // after it lies thousands of pixels away, past what `DistanceInterpolator4` multiplies in
    // `i32`, so the piece, the whole path, is not drawn.
    let line = vec![
        p(94.00000786351512, -7.817504489577309e-6),
        p(61.000000000000114, 33.00000000000001),
        p(-8584.616083133831, -0.003023169095998576),
        p(61.00000000000001, 56.17101458477057),
        p(94.00000786351512, -7.817504489577309e-6),
    ];
    assert!(!joins_admit(&line));
    assert_eq!(drawn(&dotted(123, 34, line, 1.517439853e9)), 0);
}

#[test]
fn a_long_segment_after_a_short_one_at_a_gentle_angle_is_drawn() {
    // Verifies: ADR-0046, Q-3
    // A segment of 2 pixels, then one of 2100 at 10°: their join lies some 2100 × 0.17 pixels
    // from the vertex, far within the bound, and the line is drawn as agg-rust draws it unbounded.
    let line = vec![
        p(10.0, 10.0),
        p(12.0, 10.0),
        p(2080.096281325637, 374.6611731005537),
    ];
    assert!(joins_admit(&line));
    assert_eq!(drawn(&dotted(2200, 400, line, 10.0)), 4207);
}

/// A short segment of 3620 subpixels down and to the right at 45°, then one back up and to the
/// left, `run` subpixels across and 2560 fewer up: their join, `bisectrix` of the two, lies some
/// twice the long segment's length from the short segment's end, at about 45° too, and not along
/// the short segment, so `fix_degenerate_bisectrix` keeps it.
fn hairpin(run: i32) -> Vec<Point> {
    let back = |by: i32| 12.0 - f64::from(by) / 256.0;
    vec![p(2.0, 2.0), p(12.0, 12.0), p(back(run), back(run - 2560))]
}

/// The longest run of [`hairpin`] that [`JoinProbe`] admits with [`dot`]. The short segment,
/// from (512, 512) to (3072, 3072) in subpixels, ends at the join (734831, 732271) that
/// `bisectrix` gives with the long segment of 516501, so `|v_x| = 45926 - 192 = 45734` and
/// `|v_y| = 45766 - 192 = 45574` in `line_mr` units. Its pixels lie within
/// `|u| = ((3620 + 256 × (2 × 2 + 68) + 3) >> 4) + 3 = 1381`, and the larger product,
/// `(|u| + |v_y|) × |v_x| = 46955 × 45734 = 2147439970`, is within `i32::MAX`. One subpixel more
/// moves the join to (734833, 732273), and `46956 × 45735 = 2147532660` passes it.
const HAIRPIN_RUN: i32 = 366_499;

#[test]
fn a_piece_within_the_join_bound_is_drawn_and_one_past_it_is_not() {
    // Verifies: Q-3
    assert!(joins_admit(&hairpin(HAIRPIN_RUN)));
    assert!(!joins_admit(&hairpin(HAIRPIN_RUN + 1)));
    // The short segment and the start of the long one, as agg-rust draws them unbounded.
    assert_eq!(drawn(&dotted(20, 20, hairpin(HAIRPIN_RUN), 1e6)), 41);
    assert_eq!(drawn(&dotted(20, 20, hairpin(HAIRPIN_RUN + 1), 1e6)), 0);
}

/// A tile of 4096 by 1600 pixels, all one opaque colour: a pattern 1,048,576 subpixels wide with
/// a line width of 204,928, so an extent of 801 across the line.
fn broad() -> Rc<Tile> {
    let corners = [(0.0, 0.0), (4096.0, 0.0), (4096.0, 1600.0), (0.0, 1600.0)];
    let mark = DrawOp::Fill {
        rings: vec![corners.iter().map(|&(x, y)| p(x, y)).collect()],
        color: Rgba::opaque(40, 120, 200),
        gamma: None,
    };
    Rc::new(Tile::new(4096, 1600, vec![mark]).unwrap())
}

/// `path` brushed with [`broad`] on a white image of 40 by 40 pixels, its reach 1e7 pixels.
fn broadly(path: Vec<Point>) -> RgbaImage {
    let mut renderer = AggRenderer::new();
    renderer.begin(40, 40, Rgba::opaque(255, 255, 255));
    renderer.draw(&DisplayItem {
        op: DrawOp::BrushedLine {
            paths: vec![path],
            tile: broad(),
            reach: 1e7,
        },
        feature_id: None,
    });
    renderer.finish()
}

/// The half of the longest diagonal, in subpixels, whose place [`broad`]'s pattern keeps within
/// `i32`: from (-k, -k) to (k, k) the segment is 652,214,271 subpixels long, and the place is at
/// most `(801 + 3) × 1,048,576 + 2 × 652,214,271 = 2,147,483,646`. To (k + 1, k + 1) it is
/// 652,214,273, and the place passes `i32::MAX` by 3.
const DIAGONAL_HALF: f64 = 230_592_567.0;

#[test]
fn a_piece_whose_place_would_pass_i32_is_not_drawn_and_a_short_one_beside_it_is() {
    // Verifies: Q-3
    let k = DIAGONAL_HALF;
    let past = vec![
        p(-k / 256.0, -k / 256.0),
        p((k + 1.0) / 256.0, (k + 1.0) / 256.0),
    ];
    assert_eq!(drawn(&broadly(past)), 0);
    // A line of 35 pixels, the tile higher than the image: 35 columns of 40 pixels.
    assert_eq!(drawn(&broadly(vec![p(2.0, 20.0), p(37.0, 20.0)])), 1400);
}
