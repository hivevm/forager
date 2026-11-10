# Ticket: Scale the styles of a layer with `SIZEUNITS` or `SYMBOLSCALEDENOM`, or say they are not

## What to build

The engine scales every style by `RESOLUTION` over `DEFRESOLUTION`, the last branch of
`msUpdateClassScaleFactor` in MapServer's `src/mapdraw.c` at the pin. A layer with `SIZEUNITS`
other than `pixels`, or with a `SYMBOLSCALEDENOM`, takes another branch there. Its labels are
refused with a diagnostic, but its styles are drawn at `RESOLUTION / DEFRESOLUTION` without one:
with `SYMBOLSCALEDENOM 25000` at 1:50000 and `RESOLUTION 144`, MapServer draws a style of `SIZE 10`
10 pixels high (25000 / 50000 × 144 / 72), and the engine 20. Either compute the scale factor of
each class, style, and label as `msUpdateClassScaleFactor` does, keeping the resolution factor of
the bounds apart from it, or leave such styles out with a diagnostic, as their labels are left out
([`docs/semantics/scale.md`](../../semantics/scale.md)). Which of the two is the human's decision:
the first may need the style IR to carry the `SIZEUNITS` of a class, a style, and a label, which it
holds only for a layer, and the second is a new diagnostic.

The revision of 2026-11-07 found the label's styles assuming the map's factor in three places, all
of which move with this ticket: `Styled::new` and `LabelStyles::bound_marker` in
`crates/engine/src/label_styles.rs`, and the comment of `draw_label_poly` there.

## Acceptance criteria

- [ ] Given a point layer with `SYMBOLSCALEDENOM 25000`, a style of `SIZE 10`, a map at 1:50000 and
  `RESOLUTION 144`, when it is drawn, then the marker is 10 pixels high as MapServer draws it, or
  the style is not drawn and a diagnostic names the layer and why.
- [ ] Given a layer with `SIZEUNITS meters`, when it is drawn, then its styles are sized as
  `msUpdateClassScaleFactor` sizes them, or are not drawn with the same diagnostic.
- [ ] Given a map whose layers have neither, when the engine's tests run, then nothing moves.

## Read first

- `msUpdateClassScaleFactor` and `msPrepareImage` in `src/mapdraw.c` at the pin;
  [`docs/semantics/scale.md`](../../semantics/scale.md); `classes::scale_factor` and the refusal of
  labels in `crates/engine/src/labels.rs`; the porting entry of
  [`docs/CONVENTIONS.md`](../../CONVENTIONS.md).

## Blocked by

- None (can start immediately)

## Mode

with the human: it either adds a diagnostic, a code users see, or widens what the frontend carries
into the style IR for the scale factor of classes, styles, and labels; which of the two is a
decision the ticket does not take.

## Maintainer's decision (2026-11-07)

Port the scale factor: compute the scale factor of each class, style, and label as
`msUpdateClassScaleFactor` does, carrying into the style IR what it needs (a change of the style
document's format, under ADR-0020 and ADR-0036), rather than leaving such styles out with a
diagnostic.

On 2026-11-07 the maintainer decided the format: the `sizeunits` fields lie on the class, the
style, and the label, as MapServer has them, and they take a major version of the style document,
since a reader that ignores them would draw at another size.
