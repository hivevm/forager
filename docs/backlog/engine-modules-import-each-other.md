# Two modules of the engine import each other, and no rule decides a module's direction

Observed: ADR-0008 decides the dependency edges between crates only. `symbolize` holds its modules
to one direction, stated in its `lib.rs` and held by `crates/arch-test/tests/symbolize_modules.rs`,
and no ADR decides that rule. `engine` has none, and two of its modules import each other: `labels`
uses `label_styles`, and `label_styles` calls `crate::labels::push` and `crate::labels::make_room`
and takes the type `crate::labels::LabelAt`. The move of a label's styles into `symbolize`
(revision-2026-11-07/05) took `label_binding` out of the cycle, which needs `label_styles` no
more, and left the two. Its modules also take helpers from the crate's root (`layer_opacity`,
`points_in_view`, `quoted`, `Selected`, `draw_items`, `refused_pixmap`), which leaves no leaf
module for them.

The design revision of 2026-11-11 filed this as ticket 03, to decide with `propose-adr` whether
modules within a crate import in one direction. The maintainer chose on 2026-11-12 the smallest
variant, which changes no ADR structure: no ADR is due now, since the order of a crate's modules is
internal and cheap to reverse, and a crate can hold it by a test of its own, as `symbolize` does.
The `symbolize` rule stays as its `lib.rs` and its test state it. A session that next reshapes the
engine's labels, where the cycle lies, may break it by moving `push` and `make_room` into
`label_styles` or a leaf module; a rule for every crate waits for a decision of its own, from the
catalogue's `concepts/layered.md`.

Where: crates/engine/src/labels.rs, crates/engine/src/label_styles.rs (`push`, `make_room`),
crates/engine/src/lib.rs; crates/arch-test/tests/symbolize_modules.rs; ADR-0008.
