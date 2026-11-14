# Ticket: Cite what a pixmap that does not load is decided by, not G-8

## What to build

G-8 says that a layer whose source has no bound adapter or whose adapter fails is skipped with a
diagnostic naming the layer, the source kind, and the source span. Three tests of
`crates/engine/tests/labels.rs` cite it for a label's pixmap that does not load:
`a_labelcenter_pixmap_that_does_not_load_leaves_its_marker_out_and_warns` (:3398) and
`a_labelpnt_pixmap_that_does_not_load_leaves_the_label_out` (:3473), both of 122bb82, and
`the_label_over_a_pixmap_that_does_not_load_is_left_out_and_the_map_drawn` (:4329), of a26a539.
No source fails in them, and no layer is skipped. The convention *A `Verifies:` marker claims every
*shall* of its criterion* refuses such a marker. The class's pixmap tests of
`crates/engine/tests/pixmaps.rs` (:329, :676) cite nothing. Drop the three markers, or cite the
clause of an accepted ADR that decides the behaviour, such as ADR-0029's, where one does.

## Acceptance criteria

- [ ] Given `rg -n 'Verifies: G-8' crates/engine/tests/labels.rs`, when the change lands, then it
  finds none.
- [ ] Given `scripts/check-traceability.sh`, when it runs, then G-8 is still cited by a test that
  decides it.

## Read first

- Touches: `crates/engine/tests/labels.rs`
- `docs/SPECIFICATION.md`, G-8; `docs/adr/0029-pixmaps-held-once-per-render.md`;
  `docs/CONVENTIONS.md`, *A `Verifies:` marker claims every *shall* of its criterion*.

## Blocked by

- None (can start immediately)

## Mode

unattended: the traceability check decides it; one revert. Stop when no test but these three
cites G-8; then the change is to the specification, and the session hands off.
