# Ticket: A label with no text source reaches the cache and the at-once path

## What to build

A label with no `TEXT`, no `LABELITEM` and no feature text is left out by the engine on both
paths, before anything of it is drawn (`collect` in `crates/engine/src/labels.rs:262-270`). At the
pin MapServer keeps it in two cases:

- In the label cache, `msAddLabelGroup` (`src/maplabel.c:268-276`) keeps a label whose
  `msShapeGetLabelAnnotation` gives no text only if one of its styles has `GEOMTRANSFORM
  labelpnt` (270), a "symbol only label"; without one it skips the label, with no message (275).
- With `LABELCACHE OFF`, `pointLayerDrawShape` calls `msDrawLabel` with the NULL text
  (`src/mapdraw.c:2094-2101`), which takes it as an empty text (`haveLabelText`, 2714-2715): it
  draws each `labelpnt` style's marker at the label's point (2733-2747), with no offset, and fails
  the layer's draw on a `labelpoly` or `labelcenter` style ("Unknown label geomtransform", 2794).
  Under `POSITION XY` the offset is scaled by the text's scale factor, which is 0 without a text
  (2709, 2817-2818); the style IR has no `XY` position, so the marker lies at the point either way.

A plain label style, without a transform, which `msDrawLabel` draws too (2740-2742), is not part of
this ticket: the style IR draws no label style without a transform
(`crates/style-ir/src/model.rs:529-532`).

How the cache then tests and draws a member without a text box, the case of `"\n"`, is the first
point of `docs/plan/revision-2026-10-31c/04-label-styles-left-unported.md`, which this ticket waits
for. What is left here is letting the label with no text source through `collect`, on the cached
path under the condition of `maplabel.c:270`, and on the at-once path as an empty text, which #163
draws (`docs/semantics/label.md`, LABELCACHE).

## Acceptance criteria

- [ ] Given a cached point label with no text source and a `labelpnt` style of a vector symbol,
  when it renders, then it enters the label cache, and its marker is tested and drawn, or the label
  refused, as ticket 04 settled for a member of text `"\n"` with the same style, with the boxes and
  operations stated in the test and `src/maplabel.c:268-276` cited.
- [ ] Given a cached point label with no text source and only a `labelpoly` style, when it renders,
  then nothing of it is drawn or cached, and no diagnostic is reported for it, as
  `src/maplabel.c:275` skips it.
- [ ] Given a point label with no text source and a `labelpnt` style on a layer of
  `LABELCACHE OFF`, when it renders, then its marker is drawn at the point, unmoved by the label's
  `OFFSET`, as `src/mapdraw.c:2714-2715` and 2733-2747 draw it.
- [ ] Given the same label on a layer of `LABELCACHE OFF` with a `labelpoly` or `labelcenter`
  style, when it renders, then it is left out with the warning an empty text gets there, as
  `msDrawLabel` fails at `src/mapdraw.c:2794`.
- [ ] Given the harness, when it runs, then every case holds its listed outcome and `labels-hold`
  mark.

## Read first

- `docs/semantics/label.md` (Styles, LABELCACHE); `crates/engine/src/labels.rs` (`collect`) and
  `label_styles.rs`; `crates/engine/tests/labels.rs` and #163's cases of an empty text.
- `msAddLabelGroup`, `src/maplabel.c:255-285`; `msDrawLabel`, `src/mapdraw.c:2703-2897`, at the
  pin.

## Blocked by

- docs/plan/revision-2026-10-31c/04-label-styles-left-unported.md

## Mode

unattended: engine tests whose values come from the C cited above, and from ticket 04's for the
cached marker, decide each criterion; the change stays in `crates/engine` and
`docs/semantics/label.md`, undone by one revert. It touches nothing
[ADR-0007](../../adr/0007-harness.md) keeps with the human: no resource limit, port, public
interface or style document format, only what the engine draws or refuses of a document it already
reads. Its branch counts against ADR-0007's cap of three open agent branches. A wrong result shows
as a case outcome or a `labels-hold` mark that moves, or a pinned box or operation of
`labels.rs` that moves, and is undone by the revert; the tests citing the C prove it right. Stop:
if the source at the pin contradicts a point as stated here, if the label memory test of Q-9
fails, or if a criterion needs the style IR to change, record what the source says here and hand
off.
