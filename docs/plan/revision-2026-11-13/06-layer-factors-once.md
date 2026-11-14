# Ticket: Compute a layer's factors once per render

## What to build

A layer's scale and resolution factors are computed in four places:

- `crates/engine/src/lib.rs:463-465`;
- `crates/engine/src/labels.rs:328-329`;
- `crates/engine/src/label_styles.rs:369-375`, for every layer each time a `Styled` is built;
- `crates/engine/src/label_binding.rs:323`.

They are held in three: `Fixed.factor` and `Fixed.resolution` (`label_binding.rs:82,85`),
`Def.factor` (`labels.rs:91-93`), and `Styled.factors` (`label_styles.rs:350-351`).
`symbolize::Canvas::new` (`crates/symbolize/src/canvas.rs:54,60`) seeds both factors with the scale
factor, and each caller corrects the resolution with `with_resolution`. `label_styles.rs:380-381`
passes the resolution as both. `symbolize::Factors`, made one type by 7c70c74, is not used by the
engine. Compute one `Factors` per layer once in the engine, hand it to `Canvas::new`, and hold it
where the three fields are. No behaviour changes.

## Acceptance criteria

- [ ] Given the suite at the base, when the factors are computed once, then it passes with every
  expected value unchanged, its calls of `Canvas::new` given a `Factors`.
- [ ] Given `crates/symbolize/src/canvas.rs`, when the change lands, then `Canvas::new` takes a
  `Factors` and `with_resolution` is gone.

## Read first

- Touches: `crates/engine/src/lib.rs`, `crates/engine/src/labels.rs`,
  `crates/engine/src/label_styles.rs`, `crates/engine/src/label_binding.rs`,
  `crates/symbolize/src/canvas.rs`, `crates/symbolize/src/lib.rs`
- `docs/CONVENTIONS.md`, *A commit typed `refactor` changes nothing a test can see*.

## Blocked by

- None (can start immediately)

## Mode

unattended: a refactoring decided by the suite, whose calls of `Canvas::new` change with its
signature and whose expected values stay; one revert. Stop when a test's expected value moves.
