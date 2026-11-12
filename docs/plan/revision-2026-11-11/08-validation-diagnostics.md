# Ticket: Say what a load does with `WEB` and with a `VALIDATION` pair it cannot read

## What to build

Since PR #177 the frontend reads the `VALIDATION` of `WEB` for its defaults, yet `WEB` is still in
`DISCARDED` (`crates/mapfile/src/translate/mod.rs:59`), so each map with a `WEB` block warns
`discarded-keyword` that the block is out of scope and discarded. A pair of a `VALIDATION` that is
not two strings, or an odd last value, gives no default and no diagnostic (`Defaults::of`,
`translate/defaults.rs:33-51`); `loadHashTable` fails the load there, and Forager applies the
block's other pairs. Warn only of what is discarded inside `WEB`, and warn of the pair it skips.

## Acceptance criteria

- [ ] Given a map with `WEB VALIDATION "default_x" "1" END END` and a layer `FILTER ([v] = %x%)`,
  when it loads, then no diagnostic says `WEB` is discarded as a whole, and each keyword of `WEB`
  that is not carried over is still reported.
- [ ] Given `VALIDATION "default_a" "1" "default_b" END`, when it loads, then one warning names the
  pair it skips, and `%a%` still takes its default.

## Read first

- Touches: `crates/mapfile/src/translate/mod.rs`, `crates/mapfile/src/translate/defaults.rs`,
  `docs/semantics/expression.md`
- `docs/CONVENTIONS.md` (*A reader that skips what it cannot read fails closed*, *A test that
  expects a refusal asserts its reason*).
- Overlap: `docs/plan/revision-2026-11-11/07-one-load-budget.md` edits the same two files;
  whichever lands second takes the other's shape.

## Blocked by

- None (can start immediately)

## Mode

unattended: the tests above decide it; the change stays in the frontend's diagnostics and costs one
revert; it changes no bound and no format, nothing ADR-0006 keeps with the human. Stop if the
maintainer's choice of a warning over MapServer's failed load is questioned by what the C does,
and hand off.
