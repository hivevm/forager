# Ticket: A polygon's simplify with a coordinate that is no finite number fails, not panics

## What to build

A 45-second run of ticket 01's render fuzz target from its seeds, by the review of that ticket's
rework at 753648ee, panicked:

```text
thread '<unnamed>' panicked at geo-0.33.1/src/utils.rs:96:10:
called `Option::unwrap()` on a `None` value
```

The stack runs through a class style's transform tree into `layer_transform::simplify` and
`forager_symbolize::geometry::simplify_areas` (`crates/symbolize/src/geometry.rs`), whose repair
calls geo's `unary_union`. That reaches `winding_order`, `least_index`, and `lex_cmp`, which
unwraps `partial_cmp` of two coordinates and so panics on a NaN. A print in `simplify_areas`, not
committed, decoded the input: the tolerance `NaN`, and one polygon of the ring
`(122.00000000000001, -6.450106543526558e30) (NaN, NaN) (122.00000000000001, NaN)
(122.00000000000001, -6.450106543526558e30)`. A tolerance that is no number keeps every point, as
`docs/semantics/style-symbol.md` says, geo's `Validation` finds the ring invalid, and the repair
panics.

`layer_transform::buffer` already fails a shape with a coordinate that is no finite number, since
geo's buffer panics on it: `Undrawn::Failed("calls buffer on a shape with a coordinate that is no
finite number")`, tested in `crates/symbolize/src/layer_transform/tests.rs`. `simplify` of a
polygon does the same, before `simplify_areas`, with the message "calls simplify on a polygon with
a coordinate that is no finite number". A point shape and a line keep their simplify as today: it
runs through the port of Douglas-Peucker, not through geo's predicates, and no fuzz run has found it
panic. `docs/semantics/style-symbol.md` names the failure where it describes `simplify`, as it
does for `buffer`. Whether GEOS returns a geometry for such a polygon is not confirmed.

Where the session finds another path of the transforms that hands geo's `Validation`,
`unary_union`, or another geo predicate a coordinate that is no number, it lists it in the hand-off
and does not widen this change.

The 381-byte input, for `cargo fuzz run render <file>` on the target of
`ticket/revision-2026-11-16/01-render-fuzz-fidelity-paths` at 753648ee
(`base64 -d > crash-20.bin`):

```text
eyJ0eaiLOnjv7yB7Im71UGUiOlQhAFVVVVUFOw0AAAD19e9FAAAAAAAAALPv9f//AQGB9U5FSQXv7+/v9f//AQGBAAAAra2t
30VORUnU+oUAAACtra3fRU5FSdT6hX+mf/X19fX//wEBgQAgIkZlYXR1AAEbm6Sn////9///////////////BwcHBwcHBwcH
BwcHBwcHBwcHBwcHBwcHBwcHBwcHB/////////8AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAA
```

Its SHA-1 is `1a02ad44b6e756638c28dda3540d9111699c4724`.

## Acceptance criteria

- [ ] Given a polygon with the ring above, simplified by `NaN` and by `1.0`, and a polygon with a
  coordinate `f64::INFINITY`, when the layer transform's `simplify` runs in a test of
  `crates/symbolize/src/layer_transform/tests.rs`, then it fails with "coordinate that is no finite
  number" and does not panic. Before the change the first panics.
- [ ] Given a line and a point shape with a coordinate that is no number, when `simplify` runs, then
  they are simplified as before the change.
- [ ] Given the tests of `crates/symbolize` and `crates/engine` and the msautotest harness, when
  they run, then they pass unchanged, and `scripts/sensor-diagnostics-untested.sh` reports nothing.
- [ ] Given a copy of ticket 01's target at 753648ee, not that ticket's worktree and nothing
  committed, when the input above is run by `cargo fuzz run render`, then it does not panic.
- [ ] Given `docs/semantics/style-symbol.md`, when it describes `simplify`, then it names this
  failure.

## Read first

- Touches: `crates/symbolize/src/layer_transform.rs` (`simplify`),
  `crates/symbolize/src/layer_transform/tests.rs`, `docs/semantics/style-symbol.md`
- `layer_transform::buffer` and its test of a coordinate that is no finite number (the precedent);
  `geometry::simplify_areas`; ADR-0041 (geometry through geo); `docs/SPECIFICATION.md` Q-3.

## Blocked by

- None (can start immediately)

## Mode

unattended: the tests decide it, it changes one function of one crate and a sentence of the
semantics, and a wrong result costs one revert. It touches no resource limit. Stop if:

- an existing test, an msautotest case, or a diagnostic's code has to change;
- the fix needs a change to geo, or to `geometry.rs`'s simplify itself;
- a line's or a point's simplify is found to panic on such a coordinate too.
