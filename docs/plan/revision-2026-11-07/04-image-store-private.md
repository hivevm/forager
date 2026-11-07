# Ticket: Keep the engine's image store private, and test it through a render

## What to build

`crates/engine/src/lib.rs` declares `pub mod images`, and `images.rs` publishes `Images` and
`MAX_DECODED_IMAGE_BYTES`, for the integration tests `crates/engine/tests/images.rs` and
`image_memory.rs` alone; no crate uses them, and `fonts`, its sibling, is private. The engine's
surface grew for its tests, and a host could now build a store the engine does not bound.

Make the module private and move the two tests to the seam the conventions name: a render with
files in memory (`render_with_files`), or unit tests beside the module where a render cannot reach
the case.

## Acceptance criteria

- [ ] Given the engine's public items, when they are listed, then `images` is not among them.
- [ ] Given each case of today's two tests, when the moved tests run, then each still decides it:
  the image decoded once per path, a file past its bound refused, and the decoded bytes held to
  256 MiB, the last measured as `image_memory.rs` measures it.

## Read first

- [ADR-0040](../../adr/0040-a-decoded-pixmap-is-held-once-per-render.md); the test pattern of
  [`docs/ARCHITECTURE.md`](../../ARCHITECTURE.md); `crates/engine/tests/images.rs`,
  `crates/engine/tests/image_memory.rs`.

## Blocked by

- None (can start immediately)

## Mode

unattended: the moved tests decide it, each shown red against a mutation of the limit it holds;
its scope is the module and two test files, and a wrong result costs one revert; it touches no
public interface a host uses and none of the parts [ADR-0007](../../adr/0007-harness.md) keeps with
the maintainer, and counts against its cap of three; stop: when a case cannot be reached without a
public item, the session records which here and hands off.
