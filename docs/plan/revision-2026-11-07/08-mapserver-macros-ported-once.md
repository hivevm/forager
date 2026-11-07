# Ticket: Port MapServer's `MS_MIN`, `MS_MAX`, `MS_DEG_TO_RAD`, and C's string end once

## What to build

[ADR-0037](../../adr/0037-c-number-reading-in-style-ir.md) ports C's reading of numbers once, in
`forager_style_ir::c`, so that a port that differs from the C does so in one place. Other small
ports of `mapserver.h` and of C are written more than once:

- `MS_MIN` and `MS_MAX`, which keep their second operand where a comparison fails (NaN):
  `crates/symbolize/src/bounds.rs`, `crates/label/src/cache.rs` (`ms_min`, `ms_max`), and inline
  in `crates/engine/src/labels.rs`.
- `MS_DEG_TO_RAD`: `crates/symbolize/src/style.rs` (`DEG_TO_RAD`, which the engine imports to turn
  a label) and `crates/label/src/place.rs` (`PI / 180.0`, a different rounding of the same
  constant).
- A C string's end at its first NUL: `c_str` and `c_bytes` in `crates/engine/src/classes.rs`,
  beside the selection of classes.

Move each into one place that every crate needing it may depend on, `style-ir`'s `c` module unless
the maintainer names another, and let the sites call it. Whether ADR-0037's *Applies to* is to
grow for these, as `docs/plan/revision-2026-10-31c/03-c-int-cast-once.md` grows it for the
`(int)` cast, is the maintainer's; fold the two drafts into one revision if both are open.

## Acceptance criteria

- [ ] Given `MS_MIN` and `MS_MAX` with a NaN operand, when they are called, then each returns the
  second operand, with a test that states it, and the three sites call the one port.
- [ ] Given a label at `ANGLE 30`, when it is placed, then its box is that of today's tests with
  the one `MS_DEG_TO_RAD`, or the pull request names each value that moves and by how much.
- [ ] Given the workspace, when it is searched, then `PI / 180` and a second definition of
  `ms_min` or `ms_max` appear nowhere outside the one port.

## Read first

- [ADR-0037](../../adr/0037-c-number-reading-in-style-ir.md),
  [ADR-0009](../../adr/0009-ports-and-adapters-crates.md); `crates/style-ir/src/c.rs`;
  `src/mapserver.h` at the pin; `docs/plan/revision-2026-10-31c/03-c-int-cast-once.md`.

## Blocked by

- None (can start immediately)

## Mode

with the human: it may widen what ADR-0037 places in `style-ir`, and a value of a label's turned
box may move by the last bit of the constant.
