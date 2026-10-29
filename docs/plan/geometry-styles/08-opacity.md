# Ticket: Opacity of styles and layers, and a transparent background

## What to build

The engine applies a style's `OPACITY` to its colours as MapServer does, draws a layer with its
`OPACITY` into an image of its own and composites it, and leaves the background transparent with
`IMAGEMODE RGBA`. The frontend carries a layer's `OPACITY` (and its `COMPOSITE` block's) into the
style IR, a field the style document gains.

## Acceptance criteria

- [ ] Given a style and a layer with an opacity, and a map in `IMAGEMODE RGBA` without a background
      colour, when rendered, then each matches Forager's reference image.
- [ ] Given the harness, then every case this makes pass is flipped to `passes` in the list.

## Read first

- `docs/semantics/opacity.md`; ADR-0020; MapServer at the pin: `msDrawLayer` and its compositing.

## Blocked by

- None (can start immediately)

## Mode

with the human: it adds a field to the style document (ADR-0007).
