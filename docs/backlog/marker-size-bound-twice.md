# A labelpoint marker's size binds a style to a feature a second way

Observed: `symbolize::marker_size_with` takes a style with its `SIZE` and symbol given apart, in
place of the style's own. Only `label_style_marker` in `crates/symbolize/src/label.rs` calls it, for
the label cache, which asks `LabelStyles::bound_marker` (`crates/engine/src/label_styles.rs`) for
the marker of a label whose styles vary by feature each time it tests the label. Every other style
reaches `symbolize` bound, as the `Style` that `draw_ops` and `marker_size_of` take, so `symbolize`
has two ways of binding a style to a feature.

Ticket revision-2026-11-13/28 bound the style once per test, as `LabelStyles::style` binds it to
draw it, and dropped `marker_size_with`. It stopped at its Mode's stop: on release builds, 100,000
labels with a `labelpoint` style whose `SIZE` is bound to a property rendered in 42.7–45.5 ms before
and 50.8–55.3 ms after, about a fifth slower, from a clone of the style each time the cache tests
the label (about 80 ns a test). The case of 100,000 labels without a `labelpoint` style was not
slower (16.2–17.0 ms before, 14.3–15.4 ms after). The maintainer chose on 2026-11-13 to discard the
change and keep `marker_size_with`. The branch was deleted; its measuring test is not kept.

A later change could bind the two values without the clone, a borrowed view of the style with its
`SIZE` and symbol replaced, if one reads as plainly as the bound `Style`.

Where: crates/symbolize/src/symbol.rs (`marker_size_with`, `marker_size_of`),
crates/symbolize/src/label.rs (`label_style_marker`), crates/engine/src/label_styles.rs
(`bound_marker`, `LabelStyles::style`).
