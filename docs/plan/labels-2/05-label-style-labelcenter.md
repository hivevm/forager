# Ticket: A label's labelcenter style is a marker at its labelpoly's centre

## What to build

A label's `STYLE` with `GEOMTRANSFORM labelcenter` is drawn as `msDrawLabelCache` draws it at the
pin: its marker at the centre of the label's labelpoly box, the text's box grown by a pixel where
the label is placed (`mapdraw.c:3960-3970`, `4093-4127`), in the order of the label's styles,
before the text. Its bounds take no room from other labels and stay out of the member's box the
engine reports (`msTestLabelCacheCollisions`, `maplabel.c:781-868`; `mapdraw.c:3976-4030`). The
style IR gains a `labelcenter` value of `GeomTransform` that the frontend carries over from the
mapfile; today the frontend drops it with a warning and the engine leaves out the label with
`label-not-drawn`.

## Acceptance criteria

- [ ] Given a mapfile label `STYLE` with `GEOMTRANSFORM labelcenter`, when it is translated, then
  the style IR holds `geom_transform: labelcenter` without a warning, and the schema of the style
  document lists the value.
- [ ] Given a label with a `labelcenter` style of a vector symbol, when the map renders, then the
  marker is drawn at the centre of the labelpoly's box under the text, matching operations and a
  reference image worked out from the C, turned labels included (the centre of the turned
  polygon's box).
- [ ] Given two labels whose texts do not meet but whose `labelcenter` markers do, when the map
  renders, then both are drawn, and each reports its text's box alone (with any `labelpoint`
  marker's), as MapServer's cache does.
- [ ] Given the harness, then each case this changes holds its listed outcome and the marked
  cases stay marked.

## Read first

- `src/mapdraw.c` at the pin: `msDrawLabelCache` (the labelpoly, the label styles, the member's
  box) and `computeMarkerBounds`.
- ADR-0020 and ADR-0036 (the version rule and its freeze: before the first release the schema
  file is regenerated in the same change and the version stays where it is), ADR-0007 (the style
  document format stays with the maintainer), ADR-0029, ADR-0035.
- The golden path: `labelpoint` in `crates/engine/src/labels.rs` (`Styled::draw`,
  `draw_label_marker`) and `label_poly_at` in `crates/label/src/place.rs`;
  `crates/mapfile/src/translate/style.rs` (`geom_transform`) and `translate/label.rs`.

## Blocked by

- None (can start immediately)

## Mode

with the human: it adds a value to the style IR's `GeomTransform`, a change of the style document
format, which ADR-0007 keeps with the maintainer however small; before the first release
ADR-0020's freeze clause has the schema regenerated in the same change and the version stay where
it is, and ADR-0036 asks whether it changes what an existing document means (a document that held
no transform for a mapfile's labelcenter). The engine side alone would qualify as unattended, but
it cannot land without the IR value.
