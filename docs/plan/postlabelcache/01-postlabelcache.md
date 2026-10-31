# Ticket: A layer of POSTLABELCACHE TRUE is drawn after the label cache, its labels at once

## What to build

A layer whose mapfile sets `POSTLABELCACHE TRUE` is drawn as MapServer draws it at the pin:
`loadLayer` turns its label cache off (`src/mapfile.c:4755-4759`), `msDrawMap` sets it off again
as it draws (`src/mapdraw.c:854-856`), and the layer is drawn after the label cache rather than in
the layers' order. Today the frontend warns that `POSTLABELCACHE` is not supported, and such a
layer is drawn in its order, its labels cached.

Ask before building: which loop of `msDrawMap` draws the layers of `POSTLABELCACHE TRUE`, and
what it draws them over; cite the function and lines read at the pin on this ticket.

## Acceptance criteria

- [ ] Given the answer above, cited from the pin, when this ticket is worked, then this file says
  what MapServer does, and its criteria are corrected to it.
- [ ] Given a mapfile with a layer of `POSTLABELCACHE TRUE`, when it is loaded, then the style IR
  carries it without a warning, and the layer's `label_cache` is false.
- [ ] Given such a layer before a layer of cached labels that its features cover, when it renders,
  then it is drawn over those labels, and its own labels are drawn at once.
- [ ] Given a style document of an earlier version, then it loads as before (ADR-0020).

## Read first

- ADR-0007, ADR-0020, ADR-0036.
- `docs/semantics/label.md` (LABELCACHE); the `label_cache` of a layer in the style IR (#163).
- `msDrawMap` and `loadLayer` at the pin.

## Blocked by

- None (can start immediately)

## Mode

with the human: it adds a layer property to the style document, whose format ADR-0007 keeps with
the maintainer.
