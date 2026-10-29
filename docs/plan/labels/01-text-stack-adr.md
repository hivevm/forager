# Ticket: Decide the text stack

## What to build

An ADR, through `propose-adr`, that decides how Forager turns a label's text into pixels: rustybuzz
for shaping, skrifa for outlines and metrics, whether and how glyphs are hinted, how the reference
renderer draws them, and how a font file reaches the engine (the resource loader as it is, or a
port of its own), with the bound on a font file's size. It weighs each crate for
`wasm32-unknown-unknown` without C code, its licence, and its maintenance, as the specification's
Assumption asks, and compares the result with what MapServer's AGG renderer does with FreeType and
HarfBuzz at the pin.

## Acceptance criteria

- [ ] Given the proposed ADR, when the maintainer reads it, then it names each crate with its
      version, licence, and wasm32 build, and the alternatives with why not.
- [ ] Given the ADR, then it states how MapServer at the pin loads, sizes, hints, and draws a
      glyph, from its source, and where Forager follows it and where it cannot.
- [ ] Given the ADR, then its Enforcement names the test or check that holds it.

## Read first

- The spec beside this ticket; the specification's Assumptions and Q-5, Q-8, Q-9.
- ADR-0009, ADR-0012, ADR-0015, ADR-0023; the catalogue of `propose-adr`.
- MapServer at the pin: its font cache, its text layout, and its AGG renderer's text path.

## Blocked by

- None (can start immediately)

## Mode

with the human: it decides an ADR, the maintainer's (ADR-0007).
