# Ticket: A label with no text source draws its labelpnt styles

## What to build

A label with no `TEXT`, no `LABELITEM` and no feature text is left out by the engine on both
paths (`collect` in `crates/engine/src/labels.rs`). MapServer keeps such a "symbol only label" in
the label cache (`msAddLabelGroup`, `src/maplabel.c:268-276`), and with `LABELCACHE OFF`
`msDrawLabel` is called with a NULL text and still draws its labelpnt and plain styles
(`src/mapdraw.c:2737-2748`). Draw them as MapServer does, on both paths; a labelpoly or
labelcenter style of such a label is refused as for an empty text (#163).

Ask before building: what the label cache tests and reports for a member without text, read at
the pin in `msAddLabelGroup` and `msDrawLabelCache`; cite the lines on this ticket.

## Acceptance criteria

- [ ] Given the answer above, cited from the pin, when this ticket is worked, then this file says
  what MapServer does, and its criteria are corrected to it.
- [ ] Given a cached label with no text source and a labelpnt style, when it renders, then its
  marker is drawn at the point, tested and reported as MapServer's cache does, with stated boxes.
- [ ] Given the same label on a layer of `LABELCACHE OFF`, then its marker is drawn at the point.
- [ ] Given the harness, then every case holds its listed outcome and mark.

## Read first

- `docs/semantics/label.md`; `crates/engine/src/labels.rs` and `label_styles.rs`; #163's handling
  of a label without text.

## Blocked by

- None (can start immediately)

## Mode

unattended: it touches nothing ADR-0007 keeps with the human (no resource limit, port, public
interface or style document format), its branch counts against ADR-0007's cap of three, and it
changes only `crates/engine` and `docs/semantics/label.md`. Stop: if the label memory test of Q-9
fails, or past three hours, leave the state here and hand off.
