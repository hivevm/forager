# Ticket: Port msComputeBounds once

## What to build

A shape's bounds, MapServer's `msComputeBounds`, are computed three ways, beside its cousin
`fastComputeBounds`:

- `crates/engine/src/inline.rs:98-124` (`bounds`);
- `crates/symbolize/src/geom_transform.rs:111-123` (`bbox`);
- `crates/symbolize/src/hatch.rs:368-381`, which filters non-finite points first;
- `crates/label/src/place.rs:169-180` (`fast_bounds`, MapServer's `fastComputeBounds`), which
  already folds with `ms_min` and `ms_max`, the reference the others are held to.

The three fold with `f64::min` and `f64::max`, which drop a NaN, while `ms_min` and `ms_max`
keep the second operand. Read `msComputeBounds` in `src/mapprimitive.c` at the pinned release first:
the survey could not, and whether it uses `MS_MIN` and `MS_MAX` is unconfirmed. Then port it once
in `style-ir::c`, add its name to `PORTS` in `crates/arch-test/tests/c_numbers.rs`, and call it from
each site. A site whose C differs keeps its own, with the reason beside it.

## Acceptance criteria

- [ ] Given the C at the pin, when the port lands, then its test cites ADR-0036 and covers a NaN
  vertex and an empty shape as the C decides them.
- [ ] Given the three sites, when the change lands, then each calls the port or names in a comment
  the C that differs.

## Read first

- Touches: `crates/style-ir/src/c.rs`, `crates/engine/src/inline.rs`,
  `crates/symbolize/src/geom_transform.rs`, `crates/symbolize/src/hatch.rs`,
  `crates/label/src/place.rs`, `crates/arch-test/tests/c_numbers.rs`
- `docs/adr/0036-c-ports-in-style-ir.md`; `docs/CONVENTIONS.md`, *A minimum or maximum over values
  that may be NaN decides the NaN first*.

## Blocked by

- None (can start immediately)

## Mode

unattended: the ports' tests and the unchanged suite decide it; one revert. Stop when the C cannot
be read at the pin, or when a site's change moves a test's expected value.
