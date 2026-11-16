# Ticket: A render's resources are loaded in one place of the engine

## What to build

What a render loads through the host's resource loader is spread over three holders:

- `Labels` hands out the loader (`files()`) and loads the TrueType markers' fonts
  (`load_marker_fonts`), though those fonts are not a label's;
- `Images` carries the marker glyphs (`glyphs_mut`) and returns a tuple of three from
  `drawing()`;
- `lib.rs` loads a pixmap with the same guard written three times,
  `if !images.decoded().contains_key(path) { images.image(path, files) }`.

ADR-0029 decides that images are held once per render. ADR-0044 decides that the engine answers
the glyph port. Neither decides that the labels carry the loader.

The render's resources become one value of the engine: the loader, the fonts, the decoded images,
the tile cache, and the marker glyphs. It has one call that ensures a path is loaded. `Labels`
borrows the fonts from it, and the drawing borrows the images and glyphs. The Q-9 bounds on files,
images, tiles, and brushes are counted where they are counted now. This is a `refactor`.

## Acceptance criteria

- [ ] Given the tree, when `labels.rs` is read, then it neither hands out the loader nor loads a
  marker's font.
- [ ] Given `lib.rs`, when it loads a pixmap, then it calls the one function, and the
  `contains_key` guard appears once.
- [ ] Given `cargo test --workspace` (the Q-9 tests of fonts, images, tiles, and brushes among
  them) and the msautotest harness, when they run, then all pass unchanged.

## Read first

- Touches: `crates/engine/src/{lib,labels,images,fonts,resource,marker_glyphs}.rs`
- ADR-0029, ADR-0031, ADR-0044; `docs/SPECIFICATION.md` Q-9;
  `docs/ARCHITECTURE.md` (the engine section, which names each of these files).

## Blocked by

- docs/plan/revision-2026-11-16/03-engine-rotation-owns-the-angle.md (both change `Drawing`)

## Mode

with the human: it moves the code that holds Q-9's bounds on files, images, and tiles
(`resource.rs`, `images.rs`), and ADR-0006 keeps the resource limits from unattended work.
