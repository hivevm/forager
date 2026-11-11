use forager_style_ir::Style;

use crate::canvas::Canvas;

#[test]
fn at_a_factor_the_canvas_clips_a_class_s_hatches_by_the_scaled_buffer() {
    // msDrawShape's clip buffer at a factor of 2: a plain style 1 × 2 + 1, and 2 more.
    let mut canvas = Canvas::new(&[], 101, 101, 2.0);
    canvas.class(&[Style::default()]);
    assert_eq!(canvas.hatching.clip_buffer, 5.0);
}
