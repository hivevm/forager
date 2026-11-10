# Ticket: Draw or refuse what a label's styles still leave unported

## What to build

The pull requests of the spec `labels-2` left three points of a label's styles in their *Doubts*
(`gh pr view 156`):

- A text of line breaks only, with a `labelpnt` style: #156's doubt says MapServer still tests
  and draws the marker, which no one has read at the pin. Does `msDrawLabelCache`
  (`src/mapdraw.c`) test and draw a member's `labelpoint` marker when its text has no box? The
  engine caches the label, but skips a member without a box, so the marker is neither tested nor
  drawn.
- `ANGLE AUTO` on a label's style is not refused, and is drawn as some angle.
- A label's style without a transform, which the style document now means "not drawn", is
  refused as "a STYLE other than one of GEOMTRANSFORM labelpoly, labelpnt, or labelcenter"
  (`kind` in `crates/engine/src/label_styles.rs`), which does not say that the transform is
  missing. Whether a label's style may be written without a transform at all is parked with
  revision-2026-10-31c/11 on `parked/tickets-2026-11-07` until after the take-up of NUC v0.7.0;
  until then the rule is kept, and the refusal is reworded here.
- A `labelpoly` style with a `SYMBOL`, a hatch among them, is refused today ("a STYLE of
  GEOMTRANSFORM labelpoly with a SYMBOL"), so the label is not drawn; `msDrawLabelBounds` draws it
  through `msDrawShadeSymbol`, so MapServer hatches the box. Taken over from
  `docs/plan/revision-2026-11-07/06-label-style-refused-hatch-reported.md`, which found the path
  it named unreachable: once such a style is drawn, `draw_label_poly`
  (`crates/engine/src/label_styles.rs`) must count the `false` that `symbolize::shade` returns for
  a hatch past `Hatch::is_bounded`, and the engine must report it with the layer's
  `hatches_refused`, as it reports a refused polygon hatch (Q-9). It reads the count from the
  layer's canvas today, not from the label styles' canvas in `LabelStyles`.

Read each in `msDrawLabelCache` and `computeMarkerBounds` (`src/mapdraw.c`) and `bindLabel`
(`src/maputil.c`) at the pin, and `msDrawLabelBounds`; port the first two and the fourth, or
refuse them with `label-not-drawn` where porting needs more than this ticket, and word the third
refusal for what it is. The module of a label's styles this ticket waited for landed with
ec1b873 (`crates/engine/src/label_styles.rs`).

## Acceptance criteria

- [ ] Given the question of the first point, when it is answered, then the answer names the
  function and lines of `src/mapdraw.c` read at the pin.
- [ ] Given a label of text `"\n"` with a `labelpnt` style of a vector symbol, when it is drawn,
  then its marker is tested and drawn as that answer says, or the label is refused, with the lines
  of the C cited in its test.
- [ ] Given a label's style of `ANGLE AUTO`, when it is drawn, then it is drawn as the pin draws
  it, or refused with `label-not-drawn`.
- [ ] Given a label's style without a transform, when it is refused, then the diagnostic says that
  the style has no transform.
- [ ] Given a label with a `labelpoly` style of a hatch, when it is drawn, then the box is hatched
  as `msDrawShadeSymbol` hatches it, with stated values or an msautotest case, or the label is
  refused as today; and given the same hatch past `Hatch::is_bounded`, then the layer's report
  carries the refusal with the code a refused polygon hatch gets.

## Read first

- [ADR-0028](../../adr/0028-text-stack.md),
  [ADR-0035](../../adr/0035-label-cache-grid-and-bound.md);
  `docs/semantics/label.md` (Styles); `docs/CONVENTIONS.md` (behaviour ported from MapServer, a
  refusal asserts its reason); `crates/engine/src/label_styles.rs`,
  `crates/engine/tests/labels.rs`.
- revision-2026-11-07/05, parked on `parked/tickets-2026-11-07`, which may later move the same
  drawing into `crates/symbolize`.

## Blocked by

- None (can start immediately)

## Mode

unattended: engine tests whose values come from the C decide each point; the change stays in the
engine's labels and their styles, undone by one revert. It touches nothing
[ADR-0007](../../adr/0007-harness.md) keeps with the human: no resource limit, port, public
interface, or style document format, only what the engine draws or refuses of a document it already
reads. Its branch counts against ADR-0007's cap of three open agent branches. A wrong result shows
as a case outcome or a `labels-hold` mark that moves, and is undone by the revert; the tests citing
the C prove it right. Stop: if the source contradicts a point as stated here, a port needs the style
IR to change, record what the source
says here and hand off.
