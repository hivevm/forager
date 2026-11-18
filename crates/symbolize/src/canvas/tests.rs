use forager_style_ir::Style;

use crate::canvas::{Canvas, Refusals};
use crate::line_marker::MarkerBudget;
use crate::symbol::Factors;

#[test]
fn at_a_factor_the_canvas_clips_a_class_s_hatches_by_the_scaled_buffer() {
    // msDrawShape's clip buffer at a factor of 2: a plain style 1 × 2 + 1, and 2 more.
    let mut canvas = Canvas::new(&[], 101, 101, Factors::new(2.0, 2.0));
    canvas.class(&[Style::default()]);
    assert_eq!(canvas.clip.buffer, 5.0);
}

#[test]
fn refusals_merged_sum_their_hatches_and_keep_the_first_of_each_symbol() {
    let named = |name: &str, why: &str| (name.to_owned(), why.to_owned());
    let mut refusals = Refusals {
        hatches: 1,
        pixmaps: [(1, named("a", "first"))].into(),
        symbols: [(4, named("v", "first"))].into(),
        glyphs: [(6, named("t", "first"))].into(),
        ..Refusals::default()
    };
    refusals.merge(Refusals {
        hatches: 2,
        pixmaps: [(1, named("a", "second")), (2, named("b", "why"))].into(),
        symbols: [(4, named("v", "second")), (5, named("w", "why"))].into(),
        glyphs: [(6, named("t", "second"))].into(),
        ..Refusals::default()
    });
    let merged = Refusals {
        hatches: 3,
        pixmaps: [(1, named("a", "first")), (2, named("b", "why"))].into(),
        symbols: [(4, named("v", "first")), (5, named("w", "why"))].into(),
        glyphs: [(6, named("t", "first"))].into(),
        ..Refusals::default()
    };
    assert_eq!(refusals, merged);
}

#[test]
fn the_canvas_yields_its_refusals_once() {
    let mut canvas = Canvas::new(&[], 101, 101, Factors::new(1.0, 1.0));
    canvas.refused_hatches = 2;
    canvas.refused_pixmaps.insert(3, "gone".to_owned());
    canvas.refused_symbols.insert(4, "too large".to_owned());
    let refusals = canvas.take_refusals();
    assert_eq!(refusals.hatches, 2);
    assert_eq!(refusals.pixmaps[&3].1, "gone");
    assert_eq!(refusals.symbols[&4].1, "too large");
    assert_eq!(canvas.take_refusals(), Refusals::default());
}

#[test]
fn the_canvas_yields_its_markers_spent_with_its_refusals_and_holds_on_to_them() {
    // Verifies: Q-9
    let mut canvas = Canvas::new(&[], 101, 101, Factors::new(1.0, 1.0));
    assert!(!canvas.take_refusals().markers_spent);
    canvas.markers = MarkerBudget::of(1);
    assert!(!canvas.markers.take(2, 1));
    assert!(canvas.take_refusals().markers_spent);
    // The budget stays spent for the layer's later features, and so does its report.
    assert!(canvas.take_refusals().markers_spent);
}

#[test]
fn refusals_merged_keep_markers_spent_and_the_first_bound_of_the_trees() {
    // Verifies: Q-9
    let mut refusals = Refusals {
        markers_spent: true,
        trees_over_limit: Some("first".to_owned()),
        ..Refusals::default()
    };
    refusals.merge(Refusals {
        trees_over_limit: Some("second".to_owned()),
        ..Refusals::default()
    });
    assert!(refusals.markers_spent);
    assert_eq!(refusals.trees_over_limit.as_deref(), Some("first"));
    let mut none = Refusals::default();
    none.merge(refusals);
    assert!(none.markers_spent);
    assert_eq!(none.trees_over_limit.as_deref(), Some("first"));
}
