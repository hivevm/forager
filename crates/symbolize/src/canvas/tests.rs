use forager_style_ir::Style;

use crate::canvas::{Canvas, Refusals};
use crate::symbol::Factors;

#[test]
fn at_a_factor_the_canvas_clips_a_class_s_hatches_by_the_scaled_buffer() {
    // msDrawShape's clip buffer at a factor of 2: a plain style 1 × 2 + 1, and 2 more.
    let mut canvas = Canvas::new(&[], 101, 101, Factors::new(2.0, 2.0));
    canvas.class(&[Style::default()]);
    assert_eq!(canvas.hatching.clip_buffer, 5.0);
}

#[test]
fn refusals_merged_sum_their_hatches_and_keep_the_first_of_each_pixmap() {
    let named = |name: &str, why: &str| (name.to_owned(), why.to_owned());
    let mut refusals = Refusals {
        hatches: 1,
        pixmaps: [(1, named("a", "first"))].into(),
    };
    refusals.merge(Refusals {
        hatches: 2,
        pixmaps: [(1, named("a", "second")), (2, named("b", "why"))].into(),
    });
    let merged = Refusals {
        hatches: 3,
        pixmaps: [(1, named("a", "first")), (2, named("b", "why"))].into(),
    };
    assert_eq!(refusals, merged);
}

#[test]
fn the_canvas_yields_its_refusals_once() {
    let mut canvas = Canvas::new(&[], 101, 101, Factors::new(1.0, 1.0));
    canvas.hatching.refused = 2;
    canvas.refused_pixmaps.insert(3, "gone".to_owned());
    let refusals = canvas.take_refusals();
    assert_eq!(refusals.hatches, 2);
    assert_eq!(refusals.pixmaps[&3].1, "gone");
    assert_eq!(canvas.take_refusals(), Refusals::default());
}
