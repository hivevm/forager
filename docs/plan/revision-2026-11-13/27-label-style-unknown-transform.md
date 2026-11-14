# Ticket: Draw a label's style of an unknown transform name as MapServer does

## What to build

From `docs/backlog/label-style-unknown-transform-name.md`. The frontend leaves out a label whose
style has a `GEOMTRANSFORM` that names no transform, such as `"foo"`, with `label-not-drawn`, and
its class's other labels with it (`crates/mapfile/src/translate/label.rs`, `label_style`).
`docs/semantics/label.md` says `msDrawLabelCache` fails the map's draw on such a name. At the pin,
`msStyleSetGeomTransform` (`src/mapgeomtransform.c`, 58-64) sets an unknown name's type to
`MS_GEOMTRANSFORM_NONE` and only records an error, `loadStyle` (`src/mapfile.c`, 2831-2843) does
not check it, and `loadLabel` (`src/mapfile.c`, 2058-2061) gives a style of
`MS_GEOMTRANSFORM_NONE` its default, `MS_GEOMTRANSFORM_LABELPOINT`. Confirm that reading at the pin,
and whether `map2img` turns the recorded error into a failure; then draw such a style at the label
point with a warning, and correct `docs/semantics/label.md`.

## Acceptance criteria

- [ ] Given a label's style of `GEOMTRANSFORM "foo"`, when the mapfile loads and renders, then the
  style is drawn at the label point and the load warns of the name, by a test citing the C's lines.
- [ ] Given `docs/semantics/label.md`, when the change lands, then it says what the C does.

## Read first

- Touches: `crates/mapfile/src/translate/label.rs`, `crates/mapfile/src/translate/style.rs`,
  `docs/semantics/label.md`
- `docs/CONVENTIONS.md`, *Behaviour ported from MapServer or PROJ is read in their source*.

## Blocked by

- docs/plan/revision-2026-11-13/12-style-transform-warned-once.md

## Mode

unattended: the tests decide it; one revert. Stop when `map2img` fails on the name: then the label
stays out, the semantics page is corrected alone, and the session hands off.
