use forager_style_ir::Bound;

use super::*;

const BLACK: Rgba = Rgba::opaque(0, 0, 0);
const ORIGIN: Point = Point { x: 0.0, y: 0.0 };
const IMAGE: (f64, f64) = (100.0, 100.0);

fn p(x: f64, y: f64) -> Point {
    Point { x, y }
}

/// The square from (10, 10) to (50, 50) in pixels.
fn square() -> Vec<Vec<Point>> {
    vec![vec![
        p(10.0, 10.0),
        p(50.0, 10.0),
        p(50.0, 50.0),
        p(10.0, 50.0),
    ]]
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

fn lines(spacing: f64, width: f64, angle: f64, reference: Point) -> Vec<[Point; 2]> {
    hatch_polygon(&square(), spacing, width, angle, IMAGE, reference).unwrap()
}

#[test]
fn horizontal_lines_stand_a_spacing_apart_from_the_reference_point() {
    // msHatchPolygon on the square, WIDTH 1, SIZE 10, ANGLE 0: the box grows by 0.7072 on each
    // side, so it starts at 9.2928 and is (int)(40 + 1.4144) + 1 = 42 pixels a side. createHatch
    // starts at y0 = 10 - fmod(9.2928, 10) = 0.7072 in the box and steps by 10 while below 42:
    // five lines at y = 10, 20, 30, 40, 50 on the image, across the box from x = 9.2928 to 51.2928.
    let drawn = lines(10.0, 1.0, 0.0, ORIGIN);
    assert_eq!(drawn.len(), 5);
    for (line, y) in drawn.iter().zip([10.0, 20.0, 30.0, 40.0, 50.0]) {
        assert!(
            close(line[0].x, 9.2928) && close(line[1].x, 51.2928),
            "{line:?}"
        );
        assert!(close(line[0].y, y) && close(line[1].y, y), "{line:?}");
    }
}

#[test]
fn the_reference_point_moves_the_lines() {
    // ANGLE 90 with the map's origin at pixel (3, 0): vertical lines at x = 13, 23, 33, 43.
    // x0 = 10 - fmod(9.2928 - 3, 10) = 3.7072, then every 10 below 42.
    let drawn = lines(10.0, 1.0, 90.0, p(3.0, 0.0));
    let xs: Vec<f64> = drawn.iter().map(|l| l[0].x).collect();
    assert_eq!(xs.len(), 4);
    for (x, expected) in xs.iter().zip([13.0, 23.0, 33.0, 43.0]) {
        assert!(close(*x, expected), "{xs:?}");
    }
    assert!(close(drawn[0][0].y, 9.2928) && close(drawn[0][1].y, 51.2928));
}

#[test]
fn diagonal_lines_run_from_side_to_side_of_the_box() {
    // ANGLE 45: θ = 45°, every line x cos θ + y sin θ = r with r a multiple of 10 from the
    // reference point, so x + y = 10√2 k on the image. The box's corner lies at
    // r = 9.2928 √2 = 13.142, so the first line, r0 = 10 - fmod(13.142, 10) = 6.858 into the
    // box, is k = 2: it enters on the left side at y = 9.2928 + 9.698 and leaves on the top at
    // x = 9.2928 + 9.698. The box's x + y spans 18.59 to 102.59: k from 2 to 7, six lines.
    let drawn = lines(10.0, 1.0, 45.0, ORIGIN);
    assert_eq!(drawn.len(), 6);
    let first = drawn[0];
    let reach = 20.0 * std::f64::consts::SQRT_2 - 2.0 * 9.2928;
    assert!(
        close(first[0].x, 9.2928) && close(first[0].y, 9.2928 + reach),
        "{first:?}"
    );
    assert!(
        close(first[1].x, 9.2928 + reach) && close(first[1].y, 9.2928),
        "{first:?}"
    );
    for (line, k) in drawn.iter().zip(2..) {
        for end in line {
            let along = (end.x + end.y) / std::f64::consts::SQRT_2;
            assert!(close(along, f64::from(k) * 10.0), "{line:?}");
        }
    }
    // ANGLE 135 (and -45, 315): x - y = 10√2 k, which the box's span of -42 to 42 holds for k
    // from -2 to 2, each line ending on the box's sides.
    for angle in [135.0, -45.0, 315.0] {
        let drawn = lines(10.0, 1.0, angle, ORIGIN);
        assert_eq!(drawn.len(), 5, "{angle}");
        for (line, k) in drawn.iter().zip(-2..) {
            for end in line {
                let across = (end.x - end.y) / std::f64::consts::SQRT_2;
                assert!(close(across, f64::from(k) * 10.0), "{angle}: {line:?}");
                let on_side = [end.x, end.y]
                    .iter()
                    .any(|v| close(*v, 9.2928) || close(*v, 51.2928));
                assert!(on_side, "{angle}: {line:?}");
            }
        }
    }
    // ANGLE 180 draws the lines of ANGLE 0.
    assert_eq!(
        lines(10.0, 1.0, 180.0, ORIGIN),
        lines(10.0, 1.0, 0.0, ORIGIN)
    );
}

#[test]
fn only_the_lines_that_cross_the_image_are_laid() {
    // A square 1e9 pixels a side under a hatch a pixel apart would have some 1.4e9 lines; those
    // over the image of 100 × 100, grown by the stroke, are MapServer's lines all the same.
    let huge = vec![vec![p(-1e9, -1e9), p(1e9, -1e9), p(1e9, 1e9), p(-1e9, 1e9)]];
    for angle in [0.0, 30.0, 90.0, 120.0] {
        let drawn = hatch_polygon(&huge, 1.0, 1.0, angle, IMAGE, ORIGIN).unwrap();
        assert!(
            (100..400).contains(&drawn.len()),
            "{angle}: {}",
            drawn.len()
        );
    }
    let drawn = hatch_polygon(&huge, 10.0, 1.0, 0.0, IMAGE, ORIGIN).unwrap();
    let ys: Vec<f64> = drawn.iter().map(|l| l[0].y).collect();
    assert!(
        ys.iter()
            .all(|y| (y / 10.0 - (y / 10.0).round()).abs() < 1e-6),
        "{ys:?}"
    );
    assert!(ys.contains(&0.0) && ys.contains(&100.0), "{ys:?}");
}

fn style(size: Option<f64>, width: Option<f64>) -> Style {
    Style {
        size: size.map(Bound::Value),
        width: width.map(Bound::Value),
        ..Style::default()
    }
}

fn drawn(style: &Style) -> Hatch {
    drawn_at(style, 1.0)
}

fn drawn_at(style: &Style, factor: f64) -> Hatch {
    match hatch(square(), style, BLACK, IMAGE, ORIGIN, factor) {
        Hatched::Drawn(hatch) => hatch,
        other => panic!("{other:?}"),
    }
}

#[test]
fn at_a_factor_the_spacing_width_and_pattern_are_scaled_and_bounded_by_the_resolution_factor() {
    // msDrawShadeSymbol, maprendering.c:808-817, at a scale and a resolution factor of 2:
    // without SIZE and WIDTH both are the scale factor; with them, they are scaled; the bounds
    // are scaled by the resolution factor; the pattern follows the width over the WIDTH written.
    let plain = drawn_at(&style(None, None), 2.0);
    assert_eq!(plain.width, 2.0);
    assert!(close(plain.lines[1][0].y - plain.lines[0][0].y, 2.0));
    let bounded = Style {
        max_size: Some(5.0),
        min_width: Some(3.0),
        ..style(Some(10.0), Some(2.0))
    };
    let hatch = drawn_at(&bounded, 2.0);
    assert_eq!(hatch.width, 6.0, "MINWIDTH 3 is 6 over a WIDTH of 4");
    assert!(
        close(hatch.lines[1][0].y - hatch.lines[0][0].y, 10.0),
        "MAXSIZE 5 is 10 under a SIZE of 20"
    );
    let dashed = Style {
        pattern: vec![10.0, 4.0],
        ..style(Some(10.0), Some(4.0))
    };
    assert_eq!(drawn_at(&dashed, 2.0).pattern, vec![20.0, 8.0]);
    let capped = Style {
        max_width: Some(2.0),
        ..dashed
    };
    assert_eq!(
        drawn_at(&capped, 2.0).pattern,
        vec![10.0, 4.0],
        "MAXWIDTH 2 is 4, the WIDTH written"
    );
}

#[test]
fn the_spacing_and_the_width_default_to_1_and_are_bounded() {
    let plain = drawn(&style(None, None));
    assert_eq!(plain.width, 1.0);
    assert_eq!(
        plain.lines.len(),
        42,
        "a line every pixel of the 42-pixel box"
    );
    let bounded = Style {
        max_size: Some(5.0),
        min_width: Some(3.0),
        ..style(Some(10.0), Some(2.0))
    };
    let hatch = drawn(&bounded);
    assert_eq!(hatch.width, 3.0);
    assert!(close(hatch.lines[1][0].y - hatch.lines[0][0].y, 5.0));
}

#[test]
fn the_pattern_scales_with_the_bound_on_the_width() {
    // WIDTH 4 bounded to MAXWIDTH 2 halves the pattern; the odd last length stays in the
    // display list, which drops it.
    let halved = Style {
        max_width: Some(2.0),
        pattern: vec![10.0, 4.0, 7.0],
        ..style(Some(10.0), Some(4.0))
    };
    assert_eq!(drawn(&halved).pattern, vec![5.0, 2.0, 3.5]);
    // WIDTH 0: MapServer divides by it, and a pattern of infinite lengths draws solid lines.
    let zero = Style {
        pattern: vec![10.0, 4.0],
        ..style(Some(10.0), Some(0.0))
    };
    assert_eq!(drawn(&zero).pattern, Vec::<f64>::new());
}

#[test]
fn a_hatch_that_would_not_end_or_cost_past_its_bound_is_refused() {
    // Verifies: Q-9
    let refused =
        |style: &Style| hatch(square(), style, BLACK, IMAGE, ORIGIN, 1.0) == Hatched::Refused;
    // MAXSIZE 0 bounds the spacing to 0, where MapServer's loop never ends.
    assert!(refused(&Style {
        max_size: Some(0.0),
        ..style(Some(10.0), None)
    }));
    // A pattern of nothing but zeros never ends AGG's dasher.
    assert!(refused(&Style {
        pattern: vec![0.0, 0.0],
        ..style(Some(10.0), None)
    }));
    // A spacing of a millionth of a pixel lays some 1.4e8 lines over the image.
    let tiny = Style {
        min_size: Some(1e-6),
        ..style(Some(1e-6), None)
    };
    assert!(refused(&tiny));
    // Dashes of a millionth of a pixel cut the lines into some 1e9 pieces.
    assert!(refused(&Style {
        pattern: vec![1e-6, 1e-6],
        ..style(Some(10.0), None)
    }));
}

#[test]
fn a_width_or_angle_that_is_no_number_draws_nothing() {
    let nothing =
        |style: &Style| hatch(square(), style, BLACK, IMAGE, ORIGIN, 1.0) == Hatched::Nothing;
    assert!(nothing(&Style {
        angle: Some(Bound::Value(f64::NAN)),
        ..style(Some(10.0), None)
    }));
    assert!(nothing(&Style {
        max_width: Some(f64::NAN),
        ..style(Some(10.0), None)
    }));
    let empty = hatch(
        Vec::new(),
        &style(Some(10.0), None),
        BLACK,
        IMAGE,
        ORIGIN,
        1.0,
    );
    assert_eq!(empty, Hatched::Nothing);
}

#[test]
fn a_hatch_whose_steps_no_longer_move_is_refused() {
    // Verifies: Q-9
    // A box reaching 2.3e101 pixels left of the image, at an angle a hair off 0: the lines over
    // the image lie some 1e85 from the box's corner, where adding a spacing of 1 changes nothing,
    // and MapServer's loop would never end.
    let far = vec![
        vec![p(-2.3e101, 0.0), p(0.0, 0.0), p(0.0, 2.0)],
        vec![p(1.0, 1.0)],
    ];
    assert_eq!(hatch_polygon(&far, 1.0, 1.0, 5.3e-315, IMAGE, ORIGIN), None);
    let style = Style {
        angle: Some(Bound::Value(5.3e-315)),
        ..style(None, None)
    };
    assert_eq!(
        hatch(far, &style, BLACK, IMAGE, ORIGIN, 1.0),
        Hatched::Refused
    );
}

#[test]
fn the_clip_buffer_is_msdrawshape_s() {
    // A plain style: max(1, max(-1, 1)) + 1 = 2. A hatch of SIZE 10 and WIDTH 3 offset by
    // (4, 5): max(1, max(10, 3)) + 5 + 1 = 16. The class: 16, and 2 more for a polygon.
    let plain = style(None, None);
    let hatched = Style {
        offset: Some([Bound::Value(4.0), Bound::Value(5.0)]),
        ..style(Some(10.0), Some(3.0))
    };
    let hatching = Hatching::for_class(&[plain, hatched], &[], 1.0);
    assert_eq!(hatching.clip_buffer, 18.0);
    assert!(!hatching.clip_in_pixels);
    // MINSIZE above the rest counts as it is: 12.5 + 1 rounds half to even, to 14, and 16 with
    // the 2.
    let small = Style {
        min_size: Some(12.5),
        ..style(None, None)
    };
    assert_eq!(Hatching::for_class(&[small], &[], 1.0).clip_buffer, 16.0);
}

#[test]
fn a_polygon_is_clipped_as_msclippolygonrect_clips_it() {
    // A square from -10 to 10 cut to the rectangle from 0 to 20: msClipPolygonRect's walk adds,
    // edge by edge, the corner the bottom edge enters by, (0, 0), where the right edge enters,
    // (10, 0), and the ends inside, and closes the ring.
    let square = vec![
        p(-10.0, -10.0),
        p(10.0, -10.0),
        p(10.0, 10.0),
        p(-10.0, 10.0),
        p(-10.0, -10.0),
    ];
    let clipped = clip_polygon_rect(&[square], p(0.0, 0.0), p(20.0, 20.0));
    assert_eq!(
        clipped,
        vec![vec![
            p(0.0, 0.0),
            p(10.0, 0.0),
            p(10.0, 10.0),
            p(0.0, 10.0),
            p(0.0, 0.0),
        ]]
    );
    // Rings wholly inside are kept as they are, and a ring wholly outside is dropped.
    let inside = vec![p(1.0, 1.0), p(5.0, 1.0), p(5.0, 5.0)];
    assert_eq!(
        clip_polygon_rect(std::slice::from_ref(&inside), p(0.0, 0.0), p(20.0, 20.0)),
        vec![inside.clone()]
    );
    let outside = vec![p(30.0, 30.0), p(40.0, 30.0), p(40.0, 40.0)];
    let kept = clip_polygon_rect(&[inside, outside], p(0.0, 0.0), p(20.0, 20.0));
    assert_eq!(kept.len(), 1);
}

#[test]
fn at_a_factor_the_clip_buffer_is_scaled() {
    // msDrawShape, mapdraw.c:2489-2499, at a scale and a resolution factor of 2: the largest
    // size, grown by the offset, times the scale factor; MINSIZE and MINWIDTH times the
    // resolution factor. A plain style: 1 × 2 + 1 = 3; the hatch: (10 + 5) × 2 + 1 = 31; the
    // class 31 + 2.
    let plain = style(None, None);
    let hatched = Style {
        offset: Some([Bound::Value(4.0), Bound::Value(5.0)]),
        ..style(Some(10.0), Some(3.0))
    };
    let hatching = Hatching::for_class(&[plain.clone(), hatched], &[], 2.0);
    assert_eq!(hatching.clip_buffer, 33.0);
    assert_eq!(Hatching::for_class(&[plain], &[], 2.0).clip_buffer, 5.0);
    // MINSIZE 6.25 is 12.5: + 1 rounds half to even, to 14, and 16 with the 2.
    let small = Style {
        min_size: Some(6.25),
        ..style(None, None)
    };
    assert_eq!(Hatching::for_class(&[small], &[], 2.0).clip_buffer, 16.0);
}

#[test]
fn a_hatch_bounds_its_width_and_spacing_as_ms_draw_shade_symbol_does() {
    // msDrawShadeSymbol, maprendering.c:808-813, at a factor of 2: a WIDTH and a SIZE bound to a
    // property that reads as `nan`, which atof() reads as NaN, are no number at or below 0 and
    // are scaled; MS_MIN with the upper bound times the resolution factor, then MS_MAX with the
    // lower, make them MAXWIDTH and MAXSIZE; a MINWIDTH or MINSIZE above the upper bound wins.
    let nan = forager_expr::atof(b"nan");
    let bounded = |value: f64, min: f64, max: f64| Style {
        min_size: Some(min),
        max_size: Some(max),
        min_width: Some(min),
        max_width: Some(max),
        ..style(Some(value), Some(value))
    };
    let spacing = |hatch: &Hatch| hatch.lines[1][0].y - hatch.lines[0][0].y;
    let hatch = drawn_at(&bounded(nan, 2.0, 5.0), 2.0);
    assert_eq!(hatch.width, 10.0, "MAXWIDTH 5 is 10");
    assert!(close(spacing(&hatch), 10.0), "MAXSIZE 5 is 10");
    let hatch = drawn_at(&bounded(4.0, 6.0, 3.0), 2.0);
    assert_eq!(hatch.width, 12.0, "MINWIDTH 6 is 12");
    assert!(close(spacing(&hatch), 12.0), "MINSIZE 6 is 12");
}
