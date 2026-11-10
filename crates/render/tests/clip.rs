//! The one clip to the image (ADR-0027): segments, lines, and rings cut by one module of
//! `render`, the image grown by its margin written once, and the hatch's bound measuring what the
//! clip leaves.

use forager_render::clip::{self, MAX_GLYPH_REACH, Rect, within_glyph_reach};
use forager_render::{Hatch, PathCommand, Point};
use forager_style_ir::Rgba;

fn p(x: f64, y: f64) -> Point {
    Point { x, y }
}

fn length((a, b): (Point, Point)) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    (dx * dx + dy * dy).sqrt()
}

#[test]
fn the_image_is_grown_by_the_margin_and_the_reach_past_it() {
    // Verifies: ADR-0027
    assert_eq!(
        Rect::around_image(100, 50, 0.0).bounds(),
        [-2.0, -2.0, 102.0, 52.0]
    );
    assert_eq!(
        Rect::around_image(100, 50, 3.0).bounds(),
        [-5.0, -5.0, 105.0, 55.0]
    );
}

#[test]
fn segments_lines_and_rings_with_both_ends_far_out_are_cut_to_the_image() {
    // Verifies: ADR-0027
    let r = Rect::around_image(100, 100, 0.0);
    let (a, b) = (p(-1e150, 5.0), p(1e150, 5.0));
    assert_eq!(clip::segment(a, b, &r), Some((p(-2.0, 5.0), p(102.0, 5.0))));
    assert_eq!(
        clip::line(&[a, b], &r),
        vec![vec![p(-2.0, 5.0), p(102.0, 5.0)]]
    );
    let big = 1e150;
    let ring = clip::ring(
        &[p(-big, -big), p(big, -big), p(big, big), p(-big, big)],
        &r,
    );
    assert_eq!(ring.len(), 4, "{ring:?}");
    for corner in [
        p(-2.0, -2.0),
        p(102.0, -2.0),
        p(102.0, 102.0),
        p(-2.0, 102.0),
    ] {
        assert!(ring.contains(&corner), "{corner:?}: {ring:?}");
    }
}

#[test]
fn the_hatch_s_measure_is_the_length_of_what_the_clip_leaves() {
    // Verifies: ADR-0027, Q-9
    // Lines from -1e150 to 1e150, two pixels apart under a stroke one pixel wide, so that no two
    // bands overlap: the bound lets through as many as the clip's part of each fits into
    // `Hatch::MAX_LENGTH`, and not one more.
    let (width, height) = (4096, 4096);
    let hatch = |count: u32| Hatch {
        rings: Vec::new(),
        lines: (0..count)
            .map(|k| {
                let y = 2.0 * f64::from(k);
                [p(-1e150, y), p(1e150, y)]
            })
            .collect(),
        color: Rgba::opaque(0, 0, 0),
        width: 1.0,
        pattern: Vec::new(),
        gamma: None,
    };
    let rect = hatch(0).line_rect(width, height);
    let each = length(clip::segment(p(-1e150, 0.0), p(1e150, 0.0), &rect).unwrap());
    // The image, the margin of 2 on either side, and half the stroke beyond it.
    assert_eq!(each, 4096.0 + 2.0 * 2.5);
    #[expect(clippy::cast_possible_truncation, reason = "some two thousand")]
    let fits = (Hatch::MAX_LENGTH / each).floor() as u32;
    assert!(hatch(fits).is_bounded(width, height));
    assert!(!hatch(fits + 1).is_bounded(width, height));
}

#[test]
fn the_reach_of_a_glyph_stands_beside_the_clip() {
    // Verifies: ADR-0027
    let image = [0.0, 0.0, 100.0, 100.0];
    let at = |x: f64| [PathCommand::MoveTo(p(x, 50.0)), PathCommand::Close];
    assert!(within_glyph_reach(image, &at(100.0 + MAX_GLYPH_REACH)));
    assert!(!within_glyph_reach(image, &at(101.0 + MAX_GLYPH_REACH)));
}
