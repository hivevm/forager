# Ticket: Load fonts and pixmaps through one bounded load per render, the image file under a bound of its own name

## What to build

`Fonts::load` (`crates/engine/src/fonts.rs`) and `Images::image` with `decode`
(`crates/engine/src/images.rs`) each write the same steps: the resource path made with
`ResourcePath::new`, the load through the host's resource loader once per render and path, the file
held to a size bound, and the diagnostic that the file does not load. ADR-0040 says the image
store "has the shape the fonts already have"; the code copies it instead of sharing it. The image
store bounds its file by `MAX_FONT_BYTES`, imported from `fonts`, because Q-9 lets a symbol image's
file share the font file's limit; the name reads as a mistake.

Write the bounded load once and let fonts and images call it, each with what it does to the bytes;
name the file bound by what it bounds, Q-9's 32 MiB that a font file and a symbol image's file
share.

## Acceptance criteria

- [ ] Given a font file and a pixmap file each past 32 MiB, when a map that names them renders,
  then each is refused with today's diagnostic code.
- [ ] Given a font and a pixmap each named by two styles, when the map renders, then the loader is
  asked for each once.
- [ ] Given the engine's tests, `image_memory.rs` among them, when they run, then they pass
  unchanged.

## Read first

- [ADR-0040](../../adr/0040-a-decoded-pixmap-is-held-once-per-render.md),
  [ADR-0028](../../adr/0028-text-stack.md); Q-9 in
  [`docs/SPECIFICATION.md`](../../SPECIFICATION.md); `crates/engine/src/fonts.rs`,
  `crates/engine/src/images.rs`.

## Blocked by

- None (can start immediately)

## Mode

unattended: the tests of both limits and of loading once decide it; its scope is two modules of
`engine`, and a wrong result costs one revert; it touches none of the parts
[ADR-0007](../../adr/0007-harness.md) keeps with the maintainer and counts against its cap of
three; a wrong result shows as a limit test or a load count moving; stop: when the shared load
needs a parameter per caller beyond the bytes' treatment, the session records it here and hands
off.
