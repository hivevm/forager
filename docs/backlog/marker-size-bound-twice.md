# A labelpoint marker's size binds a style to a feature a second way

Observed: `symbolize::marker_size_with` takes a style with its `SIZE` and symbol given apart, in
place of the style's own. Only `marker_of` in `crates/engine/src/label_styles.rs` calls it: the
label cache asks `LabelStyles::bound_marker` for the marker of a label whose styles vary by feature
each time it tests the label, and `bound_marker` reads the values bound to the feature from the
values it holds without rebuilding the style. Every other style reaches `symbolize` bound, as the
`Style` that `draw_ops` and `marker_size_of` take. So `symbolize` has two ways of binding a style
to a feature: the bound `Style`, and overrides of two of its properties.

The ticket revision-2026-11-07/05, which moved a label's styles into `symbolize`, named this in its
third bullet and left it, as its criteria do not ask for it. The trade-off: binding the style once
per test, as `LabelStyles::style` binds it to draw it, costs a clone of the style each time the
cache tests the label, the many tests of a crowded map among them; holding the bound marker per
label instead costs memory for every label, which `LabelStyles` avoids for Q-9.

A change could give `marker_size_of` the bound style, built once per label when it is held, if a
measurement shows the clone is cheap against the cache's test, or hold a label's marker bounds
where its styles vary, if Q-9's budget allows it.

Where: crates/symbolize/src/symbol.rs (`marker_size_with`, `marker_size_of`),
crates/engine/src/label_styles.rs (`marker_of`, `LabelStyles::bound_marker`, `LabelStyles::style`).
