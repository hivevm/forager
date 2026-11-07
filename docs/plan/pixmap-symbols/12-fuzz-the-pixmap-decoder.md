# Ticket: Fuzz the pixmap decoder as the font reader is fuzzed

## What to build

`encode`'s `decode_pixmap` reads untrusted bytes: a symbol's image file, handed in by a mapfile's
author or, in the browser, by a page. Its tests state the bytes of each kind of PNG and bound its
memory under a counting allocator, but no fuzz target drives it, where the font reader has one
(`fuzz/fuzz_targets/font.rs`). Add a target that hands arbitrary bytes to `pixmap_size` and
`decode_pixmap`, with seeds of each PNG kind the decoder's tests build, so that the weekly run of
`scripts/fuzz.sh` holds it to Q-3: no panic, no hang, and no allocation past what the header's
size, held to 4096 × 4096, allows.

## Acceptance criteria

- [ ] Given `fuzz/Cargo.toml`, when `scripts/fuzz.sh` reads its targets, then a pixmap target is
  among them, with seeds under `fuzz/seeds/` of a palette, a grey, an RGBA, a 16-bit, and an
  interlaced PNG, and a truncated one.
- [ ] Given the target run for the seconds `scripts/fuzz.sh` gives each target, when it ends, then
  it reports no crash, panic, or hang.

## Read first

- Q-3 in `docs/SPECIFICATION.md`; `scripts/fuzz.sh`; `fuzz/fuzz_targets/font.rs` as the golden
  path; `decode_pixmap` in `crates/encode/src/pixmap.rs`; the conventions on readers of untrusted
  bytes.

## Blocked by

- docs/plan/pixmap-symbols/02-png-decode-and-limits.md

## Mode

unattended: the fuzz run decides its criteria; its scope is one fuzz target and its seeds, and a
wrong result costs one revert; it touches none of the parts [ADR-0007](../../adr/0007-harness.md)
keeps with the maintainer and counts against its cap of three; stop: when the run finds a crash,
the session records the input and the panic in this file and hands off, since that is a defect of
the decoder, not of the target.
