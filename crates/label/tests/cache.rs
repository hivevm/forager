//! Which labels the label cache draws, where, and in what order, as a pure function of the boxes
//! of their texts, without a font: each expectation is worked by hand from `msDrawLabelCache`,
//! `msTestLabelCacheCollisions`, and `msCheckLabelMinDistance` at the pin.

use forager_label::{
    Anchor, MAX_PRIORITY, Member, Members, Offsets, PointLabel, Position, TextBox, draw_label_cache,
};
use forager_render::Point;

/// A text 30 pixels wide and 11 high: at `CC` of a point it takes the box 15 to either side and
/// 5.5 above and below.
const TEXT: TextBox = TextBox {
    min_x: 0.0,
    min_y: -9.0,
    max_x: 30.0,
    max_y: 2.0,
    num_lines: 1,
};

/// The image the labels are drawn into.
const SIZE: (u32, u32) = (200, 100);

/// Labels with their texts, in the order they were added.
struct Cache(Vec<(Member, &'static str)>);

impl Members for Cache {
    fn count(&self) -> usize {
        self.0.len()
    }
    fn priority(&self, index: usize) -> u8 {
        self.0[index].0.priority
    }
    fn member(&self, index: usize) -> Member {
        self.0[index].0
    }
    fn same_text(&self, a: usize, b: usize) -> bool {
        self.0[a].1 == self.0[b].1
    }
}

/// A label at `CC` of `(x, y)`, of priority 1, neither forced nor partial, without a marker.
fn at(x: f64, y: f64) -> Member {
    Member {
        priority: 1,
        point: Point { x, y },
        label: Some(PointLabel {
            position: Position::At(Anchor::Cc),
            text: TEXT,
            offsets: Offsets::default(),
            rotation: 0.0,
            buffer: 0,
            force: false,
        }),
        partials: false,
        test_point: false,
        marker: None,
        min_distance: 0.0,
    }
}

fn with(mut member: Member, change: impl FnOnce(&mut Member, &mut PointLabel)) -> Member {
    let mut label = member.label.unwrap();
    change(&mut member, &mut label);
    member.label = Some(label);
    member
}

/// The indices of the labels drawn, in the order they are drawn, with the position each takes.
fn drawn(members: Vec<Member>) -> Vec<(usize, Anchor)> {
    drawn_texts(members.into_iter().map(|m| (m, "text")).collect())
}

fn drawn_texts(members: Vec<(Member, &'static str)>) -> Vec<(usize, Anchor)> {
    let mut out = Vec::new();
    draw_label_cache(&Cache(members), SIZE, |i, p| out.push((i, p.anchor)));
    out
}

#[test]
fn of_two_overlapping_labels_the_last_added_stays() {
    // Within a priority, the cache is walked from the last label added to the first.
    let cc = Anchor::Cc;
    assert_eq!(drawn(vec![at(50.0, 50.0), at(60.0, 52.0)]), [(1, cc)]);
    // Boxes 30 apart touch, and msRectOverlap counts that as meeting; 30.5 apart they do not.
    assert_eq!(drawn(vec![at(50.0, 50.0), at(80.0, 50.0)]), [(1, cc)]);
    assert_eq!(
        drawn(vec![at(50.0, 50.0), at(80.5, 50.0)]),
        [(1, cc), (0, cc)]
    );
}

#[test]
fn a_higher_priority_is_placed_first_whatever_the_order_added() {
    let high = with(at(50.0, 50.0), |m, _| m.priority = 7);
    let low = with(at(60.0, 52.0), |m, _| m.priority = 3);
    assert_eq!(drawn(vec![high, low]), [(0, Anchor::Cc)]);
    // The highest priority goes first, then down: label 1 at 10, label 0 at 2, label 2 at 1.
    let p = |x, priority| with(at(x, 50.0), |m, _| m.priority = priority);
    let order = drawn(vec![p(20.0, 2), p(100.0, MAX_PRIORITY), p(170.0, 1)]);
    assert_eq!(order.iter().map(|d| d.0).collect::<Vec<_>>(), [1, 0, 2]);
}

#[test]
fn a_buffer_grows_the_box_that_meets_the_others() {
    // 31 apart the boxes keep a pixel between them; a buffer of 1 on each closes it.
    let two = |buffer| {
        let b = |m: Member| with(m, |_, l| l.buffer = buffer);
        drawn(vec![b(at(50.0, 50.0)), b(at(81.0, 50.0))]).len()
    };
    assert_eq!(two(0), 2);
    assert_eq!(two(1), 1);
}

#[test]
fn force_draws_a_label_whatever_it_meets_and_partials_lets_it_leave_the_image() {
    let forced = |m: Member| with(m, |_, l| l.force = true);
    assert_eq!(
        drawn(vec![forced(at(50.0, 50.0)), forced(at(60.0, 52.0))]).len(),
        2
    );
    // A label at the edge: its box reaches 5 pixels past the left of the image.
    let edge = at(10.0, 50.0);
    assert_eq!(drawn(vec![edge]), []);
    assert_eq!(drawn(vec![with(edge, |m, _| m.partials = true)]).len(), 1);
    assert_eq!(drawn(vec![forced(edge)]).len(), 1);
    // The right and lower edges are open: a box must end before the image's width and height.
    assert_eq!(drawn(vec![at(184.5, 50.0)]).len(), 1);
    assert_eq!(drawn(vec![at(185.0, 50.0)]), []);
    assert_eq!(drawn(vec![at(100.0, 94.5)]), []);
    // A label drawn past the edge is met by the next as any other.
    let partial = with(edge, |m, _| m.partials = true);
    assert_eq!(drawn(vec![at(20.0, 50.0), partial]), [(1, Anchor::Cc)]);
}

#[test]
fn auto_takes_the_next_position_where_the_first_meets_a_label() {
    let auto = with(at(100.0, 50.0), |_, l| l.position = Position::Auto);
    // Nothing in the way: UL.
    assert_eq!(drawn(vec![auto]), [(0, Anchor::Ul)]);
    // A label up and to the left of the point, drawn first, takes UL away: LR is next.
    assert_eq!(
        drawn(vec![auto, at(80.0, 40.0)]),
        [(1, Anchor::Cc), (0, Anchor::Lr)]
    );
    // A forced label at the point meets every position: forced, AUTO takes the last, LC.
    let wall = with(at(100.0, 50.0), |_, l| l.force = true);
    let forced = with(auto, |_, l| l.force = true);
    assert_eq!(
        drawn(vec![forced, wall]),
        [(1, Anchor::Cc), (0, Anchor::Lc)]
    );
    assert_eq!(drawn(vec![auto, wall]), [(1, Anchor::Cc)]);
}

#[test]
fn a_label_meets_the_markers_of_its_priority_and_above_but_its_own() {
    // A marker 10 pixels across at each point.
    let marker = |m: Member, priority: u8| {
        let p = m.point;
        with(m, |m, _| {
            m.priority = priority;
            m.marker = Some([p.x - 5.0, p.y - 5.0, p.x + 4.0, p.y + 4.0]);
        })
    };
    // Its own marker lies under it: drawn.
    assert_eq!(drawn(vec![marker(at(100.0, 50.0), 1)]).len(), 1);
    // The marker of a label of the same priority, whose own label lies elsewhere, though that
    // label is placed after it.
    let near = marker(at(112.0, 58.0), 1);
    let far = with(near, |_, l| l.offsets.label = [0, -30]);
    assert_eq!(drawn(vec![far, at(100.0, 50.0)]), [(0, Anchor::Cc)]);
    // A marker of a higher priority counts; one of a lower does not.
    let label = with(at(100.0, 50.0), |m, _| m.priority = 5);
    let higher = with(far, |m, _| m.priority = 6);
    let lower = with(far, |m, _| m.priority = 4);
    assert!(!drawn(vec![higher, label]).iter().any(|d| d.0 == 1));
    assert!(drawn(vec![lower, label]).iter().any(|d| d.0 == 1));
}

#[test]
fn a_label_tested_by_its_point_is_left_out_where_its_point_is_covered() {
    // The point of the second lies on the box of the first; its text alone would fit below it.
    let first = with(at(100.0, 38.0), |_, l| {
        l.position = Position::At(Anchor::Uc)
    });
    let below = |test_point| {
        with(at(100.0, 36.0), |m, l| {
            m.test_point = test_point;
            l.position = Position::At(Anchor::Lc);
            l.offsets.label = [0, 0];
        })
    };
    let (uc, lc) = (Anchor::Uc, Anchor::Lc);
    assert_eq!(drawn(vec![below(false), first]), [(1, uc), (0, lc)]);
    assert_eq!(drawn(vec![below(true), first]), [(1, uc)]);
    // The point is tested against the image too: 0.1 inside its edge.
    let edge = |x| {
        with(at(x, 50.0), |m, l| {
            m.test_point = true;
            m.partials = true;
            l.position = Position::At(Anchor::Cr);
        })
    };
    assert_eq!(
        drawn(vec![edge(0.05)]).len(),
        1,
        "partials lets the point out"
    );
    let strict = |x| with(edge(x), |m, _| m.partials = false);
    assert_eq!(drawn(vec![strict(0.05)]), []);
    assert_eq!(drawn(vec![strict(0.1)]), [(0, Anchor::Cr)]);
}

#[test]
fn min_distance_leaves_out_a_label_near_one_of_the_same_text() {
    let spaced = |m: Member| with(m, |m, _| m.min_distance = 20.0);
    let a = (spaced(at(40.0, 50.0)), "Fen");
    // 45 apart: the boxes keep 15 between them, within 20.
    let b = (spaced(at(85.0, 50.0)), "Fen");
    assert_eq!(drawn_texts(vec![a, b]), [(1, Anchor::Cc)]);
    // Another text is not held apart.
    assert_eq!(drawn_texts(vec![a, (b.0, "Fern")]).len(), 2);
    // 51 apart, 21 between: both.
    let c = (spaced(at(91.0, 50.0)), "Fen");
    assert_eq!(drawn_texts(vec![a, c]).len(), 2);
    // The point of a drawn label counts as its box does: an offset label whose point lies near.
    let offset = (
        with(at(60.0, 30.0), |_, l| l.offsets.label = [100, 0]),
        "Fen",
    );
    assert_eq!(drawn_texts(vec![a, offset]), [(1, Anchor::Cc)]);
    // A forced label is not held apart.
    let forced = (with(b.0, |_, l| l.force = true), "Fen");
    assert_eq!(drawn_texts(vec![forced, a]).len(), 2);
}

#[test]
fn turned_labels_meet_by_their_corners_not_their_boxes() {
    // Turned 45 degrees, a text runs right and up. A label 13 pixels from another along its
    // text meets it; 13 pixels across it, more than its height, it does not, though the boxes
    // of the turned corners overlap either way.
    let turned = |x, y| with(at(x, y), |_, l| l.rotation = std::f64::consts::FRAC_PI_4);
    let d = 13.0 * std::f64::consts::FRAC_1_SQRT_2;
    assert_eq!(
        drawn(vec![turned(100.0, 50.0), turned(100.0 + d, 50.0 - d)]).len(),
        1
    );
    assert_eq!(
        drawn(vec![turned(100.0, 50.0), turned(100.0 + d, 50.0 + d)]).len(),
        2
    );
}

#[test]
fn a_text_of_line_breaks_only_draws_nothing_but_keeps_its_marker() {
    let blank = Member {
        label: None,
        marker: Some([110.0, 45.0, 119.0, 54.0]),
        ..at(115.0, 50.0)
    };
    assert_eq!(drawn(vec![blank, at(100.0, 50.0)]), []);
}

#[test]
fn labels_far_apart_and_far_outside_the_image_meet_as_near_ones_do() {
    // The grid that bounds the tests must not hide a pair that meets: a forced label far past
    // the image's edge meets a partial one there, and one whose x is no number meets every box
    // its y meets.
    let out = |x, force| {
        with(at(x, 50.0), |m, l| {
            m.partials = true;
            l.force = force;
        })
    };
    assert_eq!(
        drawn(vec![out(-4990.0, false), out(-5000.0, true)]),
        [(1, Anchor::Cc)]
    );
    assert_eq!(drawn(vec![out(5000.0, false), out(-5000.0, true)]).len(), 2);
    let nan = with(at(f64::NAN, 20.0), |m, l| {
        m.partials = true;
        l.force = true;
    });
    assert_eq!(drawn(vec![at(20.0, 20.0), nan]), [(1, Anchor::Cc)]);
    // A wide box covers many cells.
    let wide = with(at(100.0, 50.0), |_, l| l.text.max_x = 190.0);
    assert_eq!(drawn(vec![at(170.0, 52.0), wide]), [(1, Anchor::Cc)]);
}

#[test]
fn a_turned_label_whose_box_is_no_number_meets_a_label_in_another_cell() {
    // An ANGLE bound to `nan` turns the box into no numbers at a finite point. msRectOverlap and
    // msIntersectSegments find it to meet any box, so a label far from its point, in another
    // cell of the grid, is left out.
    let nan = with(at(20.0, 20.0), |m, l| {
        m.partials = true;
        l.force = true;
        l.rotation = f64::NAN;
    });
    assert_eq!(drawn(vec![at(150.0, 80.0), nan]), [(1, Anchor::Cc)]);
    let far = with(at(1900.0, 1900.0), |m, _| m.partials = true);
    let mut cache = Vec::new();
    draw_label_cache(
        &Cache(vec![(far, "a"), (nan, "b")]),
        (2000, 2000),
        |i, _| cache.push(i),
    );
    assert_eq!(cache, [1]);
}
