# Ticket: Carry what a layer left undrawn as one value

## What to build

The hatches and pixmaps a layer's drawing refused are gathered at four levels:

- hatches in `Canvas.hatching.refused`, then `Styled.hatches_refused`
  (`crates/engine/src/label_styles.rs:353-354,395-397`), then `Labels.hatches_refused` per layer
  (`crates/engine/src/labels.rs:233-234,723-727,761-765,1044-1046`);
- pixmaps in `Canvas.refused_pixmaps`, then `Styled::take_pixmaps_refused`, then
  `Labels.pixmaps_refused`, with `pixmaps_reported` taking out those the layer reported
  (`labels.rs:235-240,731-759,1047-1055`);
- both merged by `layer_reports` (`crates/engine/src/lib.rs:329-357`), of six parameters.

Give them one value with a `merge`, which the canvas yields and `Labels` keeps per layer, and one
function that turns it into the layer's diagnostics. No behaviour changes.

## Acceptance criteria

- [ ] Given the suite at the base, when the refusals are one value, then it passes unchanged, the
  tests of `crates/engine/tests/pixmaps.rs` and `labels.rs` that count refusals among them.

## Read first

- Touches: `crates/symbolize/src/canvas.rs`, `crates/engine/src/label_styles.rs`,
  `crates/engine/src/labels.rs`, `crates/engine/src/lib.rs`
- `docs/adr/0029-pixmaps-held-once-per-render.md`

## Blocked by

- docs/plan/revision-2026-11-13/08-label-draw-narrowed.md

## Mode

unattended, on the maintainer's instruction of 2026-11-13 to implement every open ticket, the
smallest variant chosen and listed in the pull request, which the maintainer reviews: the
refactoring as written, no value of Q-9 and no diagnostic moved. Stop when that variant cannot hold.