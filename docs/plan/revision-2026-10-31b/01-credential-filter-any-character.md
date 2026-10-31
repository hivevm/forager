# Ticket: Let the browser's credential filter read a `DATA` with any character

## What to build

`has_secret_parameter` in `crates/bindings-wasm/src/credentials.rs` finds the start of a
parameter's name with `rfind` over the characters that may not belong to it and slices from the
byte after the one it found: `.map_or(0, |i| i + 1)`. `rfind` gives the first byte of a character,
so where that character takes more than one byte, the slice falls inside it and panics. The browser
runs the filter on every layer's `DATA` (`admissible`, `crates/bindings-wasm/src/lib.rs`), and a
resource path may hold any character, so a mapfile with `DATA "ü=1.shp"` aborts the render in the
page (Q-3), where the filter should keep or drop the layer. A verbatim copy of the function panics
on `"data/ü=1.shp"`: "byte index 6 is not a char boundary". It is the class of the panic that
`text_bindings` of the mapfile frontend had until #140, found here by the survey of the revision of
2026-10-31b; no fuzz target reaches the bindings.

The filter steps past the whole character it found, and decides the same for every text that does
not panic today.

## Acceptance criteria

- [ ] Given a `DATA` of `ü=1.shp`, `data/ü=1.shp`, and `€password=x`, when `admissible` reads each,
  then it returns without a panic: the first two are kept and the last is refused, as the filter
  decides for an ASCII character in the same place.
- [ ] Given the filter's existing tests, when they run, then they pass unchanged.
- [ ] Given a test of the bindings that renders a mapfile with `DATA "ü=1.shp"`, when it runs,
  then the render returns its diagnostics instead of panicking (`Verifies: Q-3`).

## Read first

- `crates/bindings-wasm/src/credentials.rs` and its tests; Q-3 and Q-7 in
  `docs/SPECIFICATION.md`; the entry on refusals asserting their reason in `docs/CONVENTIONS.md`.

## Blocked by

- None (can start immediately)

## Mode

with the human: the credential filter of the browser bindings is never unattended
([ADR-0007](../../adr/0007-harness.md)), however small the fix.
