# Ticket: A layer of POSTLABELCACHE TRUE is drawn after the label cache, its labels at once

## What to build

A layer whose mapfile sets `POSTLABELCACHE TRUE` is drawn as MapServer draws it at the pin:
`loadLayer` turns its label cache off (`src/mapfile.c:4755-4759`), and `msDrawLayer`
(`src/mapdraw.c:832`) sets it off again as it draws each layer (854-856). `msDrawMap` skips such a
layer in its loop over the layers' order (`src/mapdraw.c:456`), draws the label cache
(`msDrawLabelCache`, 593), and then draws the layers of `POSTLABELCACHE TRUE` in a second loop over
the layers' order (604-611), each through `msDrawLayer` (633). So such a layer is drawn over every
cached label of the map, after every other layer, in the order of the layers of
`POSTLABELCACHE TRUE` among themselves; and its own labels, the cache being off, are drawn at once.
Today the frontend warns that `POSTLABELCACHE` is not supported, and such a layer is drawn in its
order, its labels cached.

## Acceptance criteria

- [ ] Given a mapfile with a layer of `POSTLABELCACHE TRUE`, when it is loaded, then the style IR
  carries it without a warning, and the layer's `label_cache` is false.
- [ ] Given such a point layer listed before a point layer of cached labels that its features
  cover, when it renders, then its markers are drawn after, and over, those labels, as the second
  loop of `msDrawMap` (`src/mapdraw.c:604-611`) draws it after `msDrawLabelCache` (593), and its
  own labels are drawn at once, not entered in the cache, as `msDrawLayer` (854-856) sets
  `labelcache` off.
- [ ] Given two layers of `POSTLABELCACHE TRUE` and one without, listed postlabelcache, plain,
  postlabelcache, when it renders, then the plain layer is drawn first, the cache next, and the two
  others after it in their listed order, as the skip at `src/mapdraw.c:456` and the loop at 604
  take them.
- [ ] Given a style document of an earlier version, when it loads, then it loads as before
  (ADR-0020).

## Read first

- ADR-0007, ADR-0020, ADR-0036.
- `docs/semantics/label.md` (LABELCACHE); the `label_cache` of a layer in the style IR (#163).
- `msDrawMap` (`src/mapdraw.c`, 448-611), `msDrawLayer` (832-856), and `loadLayer`
  (`src/mapfile.c`) at the pin.

## Blocked by

- None (can start immediately)

## Mode

with the human: it adds a layer property to the style document, whose format ADR-0007 keeps with
the maintainer.
