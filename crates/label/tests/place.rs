//! Where a label stands about its point, as a pure function of its text's box, without a font:
//! each expectation is worked by hand from MapServer's `get_metrics` and the choice of a position
//! in `msDrawLabelCache` at the pin.

use forager_label::{
    AUTO_POSITIONS, Anchor, Bounds, Offsets, PointLabel, Position, TextBox, label_poly_at,
    marker_bounds, metrics, place, placed_at,
};
use forager_render::Point;

/// A text 30 pixels wide whose glyphs rise 9 pixels above the baseline and reach 2 below it,
/// 11 high in all.
const TEXT: TextBox = TextBox {
    min_x: 0.0,
    min_y: -9.0,
    max_x: 30.0,
    max_y: 2.0,
    num_lines: 1,
};

const AT: Point = Point { x: 100.0, y: 50.0 };

fn point(x: f64, y: f64) -> Point {
    Point { x, y }
}

#[test]
fn each_of_the_nine_positions_puts_the_box_where_get_metrics_puts_it() {
    // 4 pixels across and 3 down from the point; the text is drawn from its baseline's left end,
    // its box `[min x, min y, max x, max y]`.
    let cases = [
        // The box's lower right corner 4 left of and 3 above the point.
        (Anchor::Ul, point(66.0, 45.0), [66.0, 36.0, 96.0, 47.0]),
        // Centred across, its offset across ignored, 2 more above: MARKER_SLOP.
        (Anchor::Uc, point(85.0, 43.0), [85.0, 34.0, 115.0, 45.0]),
        (Anchor::Ur, point(104.0, 45.0), [104.0, 36.0, 134.0, 47.0]),
        // At the left, 2 more away, and on one line with an offset down its baseline that offset
        // below the point, not centred.
        (Anchor::Cl, point(64.0, 51.0), [64.0, 42.0, 94.0, 53.0]),
        // Centred on the point, then moved by the offset.
        (Anchor::Cc, point(89.0, 56.5), [89.0, 47.5, 119.0, 58.5]),
        (Anchor::Cr, point(106.0, 51.0), [106.0, 42.0, 136.0, 53.0]),
        // The box's upper right corner 4 left of and 3 below the point.
        (Anchor::Ll, point(66.0, 62.0), [66.0, 53.0, 96.0, 64.0]),
        (Anchor::Lc, point(85.0, 64.0), [85.0, 55.0, 115.0, 66.0]),
        (Anchor::Lr, point(104.0, 62.0), [104.0, 53.0, 134.0, 64.0]),
        // A bound integer that names no position: no case of the switch, the box's lower left
        // corner on the point.
        (
            Anchor::Other,
            point(100.0, 48.0),
            [100.0, 39.0, 130.0, 50.0],
        ),
    ];
    for (anchor, origin, bbox) in cases {
        let (at, bounds) = metrics(AT, anchor, &TEXT, 4, 3, 0.0, 0);
        assert_eq!(
            (at, bounds),
            (origin, Bounds { poly: None, bbox }),
            "{anchor:?}"
        );
    }
}

#[test]
fn a_buffer_grows_the_box_and_leaves_the_point() {
    let (at, bounds) = metrics(AT, Anchor::Ul, &TEXT, 4, 3, 0.0, 2);
    assert_eq!(at, point(66.0, 45.0));
    assert_eq!(bounds.bbox, [64.0, 34.0, 98.0, 49.0]);
}

#[test]
fn a_labelpoly_is_the_text_box_grown_by_1_pixel_whatever_the_labels_buffer() {
    // msDrawLabelCache computes the labelpoly with get_metrics at the label's position and
    // offsets and a buffer of 1; the label's own BUFFER of 5 grows only the box it tests.
    let label = PointLabel {
        position: Position::At(Anchor::Ul),
        text: TEXT,
        offsets: Offsets {
            marker: [0.0, 0.0],
            label: [4, 3],
        },
        rotation: 0.0,
        buffer: 5,
        force: false,
    };
    // At UL 4 across and 3 down the text's box is [66, 36, 96, 47].
    assert_eq!(
        label_poly_at(AT, &label, Anchor::Ul),
        Bounds {
            poly: None,
            bbox: [65.0, 35.0, 97.0, 48.0]
        }
    );
    assert_eq!(
        placed_at(AT, &label, Anchor::Ul).1.bbox,
        [61.0, 31.0, 101.0, 52.0]
    );
}

#[test]
fn a_text_of_several_lines_stands_centred_beside_its_point() {
    // At CL and CR an offset down moves only a text of one line off the centre.
    let two = TextBox {
        num_lines: 2,
        ..TEXT
    };
    let (at, _) = metrics(AT, Anchor::Cl, &two, 4, 3, 0.0, 0);
    assert_eq!(at, point(64.0, 53.5));
    let (at, _) = metrics(AT, Anchor::Cr, &TEXT, 4, -3, 0.0, 0);
    assert_eq!(at, point(106.0, 53.5), "an offset up does not either");
}

#[test]
fn a_turned_label_turns_its_point_and_its_box_about_the_label_point() {
    // A quarter turn counter-clockwise at UR without offset: the text's lower left corner stays
    // on the point and the text runs upwards, its baseline 2 pixels left of the point.
    let quarter = std::f64::consts::FRAC_PI_2;
    let (at, bounds) = metrics(AT, Anchor::Ur, &TEXT, 0, 0, quarter, 0);
    let near = |a: Point, b: Point| (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9;
    assert!(near(at, point(98.0, 50.0)), "{at:?}");
    let poly = bounds.poly.expect("a turned label has its corners");
    for (corner, expected) in poly.iter().zip([
        point(100.0, 50.0),
        point(89.0, 50.0),
        point(89.0, 20.0),
        point(100.0, 20.0),
        point(100.0, 50.0),
    ]) {
        assert!(near(*corner, expected), "{corner:?} for {expected:?}");
    }
    let [x0, y0, x1, y1] = bounds.bbox;
    assert!(near(point(x0, y0), point(89.0, 20.0)) && near(point(x1, y1), point(100.0, 50.0)));
    // The arithmetic is get_metrics', with libm's sine and cosine.
    let (sin, cos) = (libm::sin(quarter), libm::cos(quarter));
    assert_eq!(
        at,
        point(
            100.0 + (0.0 * cos - 2.0 * sin),
            50.0 - (0.0 * sin + 2.0 * cos)
        )
    );
}

#[test]
fn a_fixed_position_takes_the_offsets_of_the_label_and_the_marker() {
    // The marker's half size and the label's offset are added as doubles and cut to whole
    // pixels: 5.5 + 1 is 6, 5.5 + 2 is 7. At CC only the label's offset counts.
    let offsets = Offsets {
        marker: [5.5, 5.5],
        label: [1, 2],
    };
    let ul = place(AT, &label(Position::At(Anchor::Ul), offsets, false), |_| {
        true
    })
    .expect("placed");
    assert_eq!(ul.origin, metrics(AT, Anchor::Ul, &TEXT, 6, 7, 0.0, 0).0);
    let cc = place(AT, &label(Position::At(Anchor::Cc), offsets, false), |_| {
        true
    })
    .expect("placed");
    assert_eq!(cc.origin, metrics(AT, Anchor::Cc, &TEXT, 1, 2, 0.0, 0).0);
    // A negative sum is cut towards 0.
    let offsets = Offsets {
        marker: [0.0, 0.0],
        label: [-3, 0],
    };
    let lr = place(AT, &label(Position::At(Anchor::Lr), offsets, false), |_| {
        true
    })
    .expect("placed");
    assert_eq!(lr.bounds.bbox[0], 97.0);
}

#[test]
fn a_fixed_position_that_does_not_fit_is_placed_only_when_forced() {
    let fixed = Position::At(Anchor::Lr);
    let none = Offsets::default();
    assert_eq!(place(AT, &label(fixed, none, false), |_| false), None);
    let forced = place(AT, &label(fixed, none, true), |_| false).expect("forced");
    assert_eq!(forced.anchor, Anchor::Lr);
}

#[test]
fn auto_takes_the_first_position_that_fits_in_msdrawlabelcaches_order() {
    assert_eq!(
        AUTO_POSITIONS,
        [
            Anchor::Ul,
            Anchor::Lr,
            Anchor::Ur,
            Anchor::Ll,
            Anchor::Cr,
            Anchor::Cl,
            Anchor::Uc,
            Anchor::Lc,
        ]
    );
    let offsets = Offsets {
        marker: [2.0, 2.0],
        label: [0, 0],
    };
    // Every position is tried, with the marker's offset, until one fits.
    let mut tried = Vec::new();
    let placed = place(AT, &label(Position::Auto, offsets, false), |b| {
        tried.push(b.bbox);
        tried.len() == 3
    })
    .expect("the third fits");
    assert_eq!(placed.anchor, Anchor::Ur);
    let (origin, bounds) = metrics(AT, Anchor::Ur, &TEXT, 2, 2, 0.0, 0);
    assert_eq!((placed.origin, placed.bounds), (origin, bounds));
    assert_eq!(
        tried[0],
        metrics(AT, Anchor::Ul, &TEXT, 2, 2, 0.0, 0).1.bbox
    );
    // None fits: the label is not placed, unless forced, then at the last.
    assert_eq!(
        place(AT, &label(Position::Auto, offsets, false), |_| false),
        None
    );
    let forced = place(AT, &label(Position::Auto, offsets, true), |_| false);
    assert_eq!(forced.map(|p| p.anchor), Some(Anchor::Lc));
}

/// A label of [`TEXT`] at `position`, unturned and without a buffer.
fn label(position: Position, offsets: Offsets, force: bool) -> PointLabel {
    PointLabel {
        position,
        text: TEXT,
        offsets,
        rotation: 0.0,
        buffer: 0,
        force,
    }
}

#[test]
fn a_labelpoint_marker_is_its_box_about_its_anchor_or_its_turned_corners() {
    // computeMarkerBounds: 8 by 4 at (10, 20), anchored at its upper left corner, offset (1, 2):
    // centred (0.5 - 0) * 8 + 10 + 1 = 15 across and (0.5 - 0) * 4 + 20 + 2 = 24 down.
    let at = Point { x: 10.0, y: 20.0 };
    assert_eq!(
        marker_bounds(at, (8.0, 4.0), [0.0, 0.0], 0.0, [1.0, 2.0]),
        Bounds {
            poly: None,
            bbox: [11.0, 22.0, 19.0, 26.0]
        }
    );
    // Turned by 90 degrees about its centre, its corners (4, 2), (4, -2), (-4, -2), (-4, 2)
    // become (2, -4), (-2, -4), (-2, 4), (2, 4), moved to the point: a box 4 wide, 8 high.
    let turned = marker_bounds(at, (8.0, 4.0), [0.5, 0.5], 90.0, [0.0, 0.0]);
    let poly = turned.poly.expect("a turned marker has its corners");
    let want = [
        (12.0, 16.0),
        (8.0, 16.0),
        (8.0, 24.0),
        (12.0, 24.0),
        (12.0, 16.0),
    ];
    for (p, (x, y)) in poly.iter().zip(want) {
        assert!(
            (p.x - x).abs() < 1e-12 && (p.y - y).abs() < 1e-12,
            "{poly:?}"
        );
    }
    for (got, want) in turned.bbox.iter().zip([8.0, 16.0, 12.0, 24.0]) {
        assert!((got - want).abs() < 1e-12, "{:?}", turned.bbox);
    }
}
