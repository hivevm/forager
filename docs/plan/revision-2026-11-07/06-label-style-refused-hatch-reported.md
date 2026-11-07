# Ticket: Count and report a hatch refused on a label's `labelpoly` style

## What to build

`symbolize::shade` returns `false` when it refuses a hatch past its cost bound
(`Hatch::is_bounded`, ADR-0032), and `draw_ops` counts the refusal, which the engine reports per
layer as `hatches_refused` (Q-9). `draw_label_poly` in `crates/engine/src/label_styles.rs` drops
the return value: a label's `labelpoly` style of a hatch past the bound is left undrawn without a
diagnostic, where the same hatch on a polygon is reported. Count it with the layer's refusals, so
that a refusal is reported the same way on every path.

## Acceptance criteria

- [ ] Given a point layer whose label has a `labelpoly` style of a hatch with a `SIZE` and `WIDTH`
  that `Hatch::is_bounded` refuses, when it renders, then the layer's report carries the refusal
  with the code a refused polygon hatch gets.
- [ ] Given the same label with a hatch within the bound, when it renders, then no refusal is
  reported and the pixels are today's.

## Read first

- [ADR-0032](../../adr/0032-hatch-cut-by-i-overlay.md); Q-9 in
  [`docs/SPECIFICATION.md`](../../SPECIFICATION.md); `crates/engine/src/label_styles.rs`
  (`draw_label_poly`), `crates/symbolize/src/draw.rs` (`shade`, `draw_ops`), the layer's reports in
  `crates/engine/src/lib.rs`.

## Blocked by

- None (can start immediately)

## Mode

unattended: the regression test decides it, red before the fix; its scope is one function and the
report it feeds, and a wrong result costs one revert; it touches none of the parts
[ADR-0007](../../adr/0007-harness.md) keeps with the maintainer and counts against its cap of
three; stop: when the label's refusal needs a diagnostic code of its own, the session records it
here and hands off.
