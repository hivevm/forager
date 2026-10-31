# Ticket: Move a label's styles out of `labels.rs`, each style sorted into its kind once

## What to build

`crates/engine/src/labels.rs` grew from 1,002 to 1,350 lines over the spec `labels-2`, and its
tickets conflicted in it three times; clippy's `too_many_lines` split its functions several times,
and the splits pass tuples to stay under the argument count (`hold`'s
`(styles, label_marker)`, `Styled::draw`'s `(member, placement)`). A label's styles are the larger
part of that growth and are spread over the file: `unsupported_style` and `marker_drawn` decide
which a label may carry; `label_marker` sizes the first `labelpoint` marker; `Styled`,
`draw_label_poly`, and `draw_label_marker` draw them; and three tables hold what they need, each
by another index (`Labels::label_styles` and `label_markers` by the text, `Def::label_marker` by
the definition). `Labels::markers`, the marker of the class's first style that `msAddLabelGroup`
caches beneath each label, is no label style and stays.

Two matches on a style's `GEOMTRANSFORM` are kept in step by hand: `unsupported_style` refuses what
it does not name, and `Styled::draw` draws every other style, its `_` arm, as a labelpoly. Should a
transform pass the first and not be named by the second, it is drawn as a labelpoly without a word:
the shape of the mistake #156's review caught in the frontend, where a dropped `GEOMTRANSFORM` fell
back to the label point.

Move a label's styles into a module of their own beside `labels.rs`, behaviour unchanged: one
function sorts a label's style into a kind of its own (labelpoly, labelpoint, labelcenter) or the
reason it is not drawn, and both the refusal and the drawing match on that kind with no catch-all
arm. Ticket `docs/plan/revision-2026-10-31/10-label-binding-stage.md` moves `bindLabel` out of the
same file; the two do not overlap, and whichever lands second rebases on the other.

## Acceptance criteria

- [ ] Given the suite and the conformance harness, when they run after the move, then every test
  and every case's outcome and `labels-hold` mark is as before, and no existing test changes.
- [ ] Given the drawing of a label's styles, when its match on the style's kind is read, then it
  names every kind and has no `_` arm, so a new kind does not compile until it is drawn.
- [ ] Given `crates/engine/src/labels.rs`, when the move is done, then it holds none of the
  functions named above nor the three tables of a label's styles, and no function of the new module
  takes a tuple to stay under clippy's argument count.

## Read first

- [ADR-0028](../../adr/0028-text-stack.md),
  [ADR-0035](../../adr/0035-label-cache-grid-and-bound.md);
  `docs/CONVENTIONS.md` (the `refactor` commit, a reader that fails closed); the module layout of
  `crates/engine/src/` in `docs/ARCHITECTURE.md`; `crates/engine/tests/labels.rs`, the seam.

## Blocked by

- None (can start immediately)

## Mode

unattended: the existing suite and the harness decide it; the change is a refactor inside one crate,
undone by one revert; it touches no public interface and nothing ADR-0007 keeps with the human (no
resource limit, port, or style document format), and its branch counts against ADR-0007's cap of
three open agent branches. It is noticed wrong by any test or case outcome that moves. Stop: if a
test or a case's outcome would have to change, or the move needs a change outside `crates/engine`,
record where in this file and hand off. Claim it only while no other ticket that edits `labels.rs`
is claimed.
