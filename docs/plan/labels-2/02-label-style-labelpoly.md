# Ticket: A label's labelpoly style is drawn under its text and counts in its collisions

## What to build

The tracer through a label's `STYLE`s: a style with `GEOMTRANSFORM labelpoly` is drawn as
`msDrawLabelCache` draws it at the pin, the polygon of the text's box grown as the style's offset
and width ask, filled and outlined in the style's colours under the text, and its bounds count in
the label's box for the collision tests of the label cache (ADR-0035). Today the engine reports a
label with a `STYLE` as not drawn. `labels`, `labels-anno` and `attrbind_test001`, whose labels
have such styles, cache their labels and hold their new outcome.

## Acceptance criteria

- [ ] Given a point label with one `labelpoly` style, when the map renders, then the box is drawn
  under the text, matching a reference image of Forager's whose geometry is worked out from
  `msDrawLabelCache` and `get_metrics` at the pin.
- [ ] Given two labels whose texts do not meet but whose `labelpoly` boxes do, then the second is
  discarded, as MapServer's cache discards it; `crates/label/tests/cache_pairwise.rs`'s reference
  learns the style bounds and still agrees.
- [ ] Given the harness, then each case this changes holds its new listed outcome and records
  comparison; the 5 marked cases stay marked.

## Read first

- `src/mapdraw.c` (`msDrawLabelCache`, the label styles) and `src/maplabel.c` at the pin;
  ADR-0028, ADR-0035; the engine's labels; `crates/label`'s cache and its pairwise test.

## Blocked by

- None (can start immediately)

## Mode

unattended: the reference images, the pairwise test and the harness decide its criteria; its scope
is the engine's labels and the label cache; a wrong result costs one revert; it touches none of the
parts ADR-0007 keeps with the maintainer and counts against its cap of three; stop: after two
attempts that leave `scripts/check-all.sh` red, or when the label cache's bound of ADR-0035 would
have to change, the session leaves its state in this file and hands off.
