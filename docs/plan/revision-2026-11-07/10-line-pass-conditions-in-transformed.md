# Ticket: Check whether a transformed style honours its pass's condition, as `lineLayerDrawShape` does

## What to build

`Pass` in `crates/symbolize` carries both which style a pass draws, the style or its outline, and
when it is skipped: `PlainIfColored` and `OutlineIfOutlined` draw only a style with a colour or an
outline. `line_symbol` honours the condition. `transformed`, which draws a style's `GEOMTRANSFORM`,
treats them as plain `Plain` and `Outline`, and the plan of a line layer
(`crates/engine/src/plan.rs`) does hand it those passes. It may be a defect: a line layer's
style with a `GEOMTRANSFORM` and no colour, or no outline, may draw where MapServer skips it.

Read `lineLayerDrawShape` and `msDrawShape`'s handling of a transformed style at the pin, state in
this file what MapServer does for each pass, and either fix `transformed` with a regression test,
or, if the condition does not apply there, say so in the code with the lines read. If the fix
changes behaviour, it is a commit typed `fix`, apart from any refactor.

## Acceptance criteria

- [ ] Given a line layer of two styles, the second with `GEOMTRANSFORM vertices`, a symbol, and no
  `COLOR`, when it renders, then its markers are drawn or skipped as `lineLayerDrawShape` decides,
  with a test that states the pixels or the items.
- [ ] Given the same with an `OUTLINECOLOR` and no `OUTLINEWIDTH`, when it renders, then its
  outline pass is drawn or skipped as MapServer decides, with a test.

## Read first

- `lineLayerDrawShape`, `msDrawShape`, and `msDrawTransformedShape` in `src/mapdraw.c` and
  `src/maprendering.c` at the pin; `crates/symbolize/src/draw.rs` (`Pass`, `line_symbol`,
  `transformed`); `crates/engine/src/plan.rs`; the porting entry of
  [`docs/CONVENTIONS.md`](../../CONVENTIONS.md).

## Blocked by

- None (can start immediately)

## Mode

unattended: the two tests decide it, each red before a fix; its scope is one function of
`symbolize`, and a wrong result costs one revert; it touches none of the parts
[ADR-0007](../../adr/0007-harness.md) keeps with the maintainer and counts against its cap of
three; stop: when the C does not settle what a pass does, or a conformance case moves, the session
records it here and hands off.
