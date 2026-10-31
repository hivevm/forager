# Spec: Point labels, round 2

## Problem

Point labels are drawn and measured (G-10), but the text cases of the conformance harness that
fail do so on what the first round left out: 5 on `MAXLENGTH`, which breaks a long text into lines;
5 on a label's `STYLE`, the box behind the text or the marker at its point; 1 on a `PRIORITY`
computed by an expression; 1 also on a layer's `LABELCACHE OFF`. A user's map with any of these
loses its labels or draws them unlike MapServer, and in 11 of the 16 text cases Forager does not
even cache the labels MapServer caches, so G-10 cannot hold there.

## Solution

The labels of point features are drawn with these four as MapServer draws them at the pin: a text
longer than `MAXLENGTH` is broken at its `WRAP` character, as `textlayout.c` breaks it; a label's
`STYLE`s with `GEOMTRANSFORM labelpoly`, `labelpoint`, or `labelcenter` are drawn with it and
counted in its collisions, as `msDrawLabelCache` counts them; a `PRIORITY` may be an expression;
a layer with `LABELCACHE OFF` draws its labels at once, without the cache. Each failing text case
that only lacked these turns to passing, and its labels to holding G-10 where they match.

## Existing behaviour

- The engine draws the labels of point layers through MapServer's label cache (ADR-0035) and
  reports every label it caches in `Rendered.labels`; the harness compares them with MapServer's
  record and holds the list's `labels-hold` marks (ADR-0029, proposed). The 5 cases marked now must
  stay marked.
- The frontend reads `MAXLENGTH`, `WRAP`, a label's `STYLE`s with their `GEOMTRANSFORM`, and
  `PRIORITY` as a number or a binding into the style IR; the engine reports a label with a `STYLE`
  or a `MAXLENGTH` as not drawn, with a diagnostic.
- `LABELCACHE` is ignored with a warning; ticket labelcache-off/01, with the maintainer, plans it.
- The frontend reads a `PRIORITY` written as an expression into the style IR (`Bound::Expr`); the
  engine refuses a label whose property is an expression, so `label_priority_expression` caches
  none of its labels.

## Goals served

- G-10 and G-13 (category text), for labels of point features.
- Q-4: the share of passing cases grows.
- Q-9: a label's styles add to the cost of a label; the label cache's bound (ADR-0035) holds.

## User stories

1. As a user, I want a long label broken into lines at `MAXLENGTH`, so that it reads as MapServer
   draws it.
2. As a user, I want a label's background box (`labelpoly`) and its marker (`labelpoint`,
   `labelcenter`) drawn, and their room taken from other labels, so that labels look and collide as
   in MapServer.
3. As a user, I want a `PRIORITY` computed by an expression, so that my mapfile's priorities apply.
4. As a user, I want a layer with `LABELCACHE OFF` to draw its labels at once, so that they are
   neither dropped nor moved above later layers.
5. As the maintainer, I want each text case that now passes marked `labels-hold` where its labels
   match MapServer's, the box of a `labelpoint` style included, so that G-10 is measured with them.

## Implementation decisions

- **MAXLENGTH.** As `textlayout.c` at the pin: with a positive `MAXLENGTH`, a text of more glyphs
  is broken at the `WRAP` character or U+200B once a line holds `MAXLENGTH` glyphs; with `WRAP`
  and no `MAXLENGTH`, at every `WRAP` character, as today.
- **Label styles.** As `msDrawLabelCache`: a `labelpoly` style's polygon is the text's box grown by
  the style, drawn under the text and counted in the label's box for collisions; a `labelpoint` or
  `labelcenter` style is a marker at the label's point or its box's centre, its bounds counted too.
  The box the engine reports for a drawn label is MapServer's member box, the union of the text's
  box and the `labelpoint` styles' bounds (the maintainer's choice), so that G-10 compares like
  with like.
- **PRIORITY expression.** The engine evaluates a `PRIORITY` the style IR holds as an expression
  per feature, through `expr`, and bounds it as MapServer does; the style document is unchanged.
- **LABELCACHE OFF.** As ticket labelcache-off/01 plans it; this spec only counts it in.

## Testing decisions

- MAXLENGTH and the styles are tested at the engine with Forager's reference images and the label
  report, their expected values read from MapServer's source; the label cache's collisions with
  styles against `cache_pairwise.rs`'s pairwise reference, extended to style bounds.
- The harness holds each changed case's outcome and `labels-hold` mark against the records.

## Out of scope

- Labels along lines and in polygons, leader lines, complex scripts, bitmap fonts.
- Style geometry transforms of a label other than `labelpoly`, `labelpoint`, `labelcenter`.
- Shadows (`SHADOWCOLOR`, `SHADOWSIZE`).

## ADR impact

- Binding: ADR-0028 (text stack), ADR-0029 (the label records), ADR-0031 (the expression
  evaluator), ADR-0035 (the label cache's grid and bound).
- No new ADR expected.
