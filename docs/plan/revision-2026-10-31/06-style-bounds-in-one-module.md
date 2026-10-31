# Ticket: Gather a style's bounds and MapServer's constants into one module

## What to build

`MS_MINSYMBOLWIDTH`, `MS_MAXSYMBOLWIDTH`, `MS_MINSYMBOLSIZE`, and `MS_MAXSYMBOLSIZE` are defined in
`draw.rs`, `symbol.rs`, and `hatch.rs`; `DEG_TO_RAD` in `symbol.rs`, `hatch.rs`, and `labels.rs`;
`ms_max` in `symbol.rs` and in `crates/label/src/cache.rs`. A style's width is bounded five times,
in three orders and with two kinds of comparison (`draw.rs` twice with `f64::min`/`max`, once
clamped to `(0, MAX)` without the style's `MINWIDTH`; `symbol.rs` and `hatch.rs` with `ms_min` and
`ms_max` in opposite orders). Some orders follow MapServer, but no copy names the function it
follows. Put the constants and one named function per MapServer site (`msDrawLineSymbol`,
`computeSymbolStyle`, `msDrawShadeSymbol`) into one module, each citing its function at the pin.

## Acceptance criteria

- [ ] Given a style with `MINWIDTH` and `MAXWIDTH` and a `NaN`-yielding binding, when each of the
  five paths draws it, then each follows the MapServer function it names, a test per path.
- [ ] Given the crates, when they are searched, then each constant is defined once.

## Read first

- the files named above; the porting entry of [`docs/CONVENTIONS.md`](../../CONVENTIONS.md); ticket
  01 of this revision, which moves these modules.

## Blocked by

- None (can start immediately)

## Mode

unattended: a test per bounding path, against MapServer's source, decides its criteria; its scope is
the bounds of styles in `crates/engine`, and a wrong result costs one revert; it touches none of the
parts [ADR-0007](../../adr/0007-harness.md) keeps with the maintainer and counts against its cap of
three; a wrong result shows as a moved reference image or a falling conformance case; stop: after
two attempts that leave `scripts/check-all.sh` red, or when ticket 01 of this revision has started,
which moves the same files, the session leaves its state in this file and hands off.
