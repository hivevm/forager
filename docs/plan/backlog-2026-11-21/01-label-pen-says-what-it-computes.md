# Ticket: A label's pen is a plain product, and its comment says what the code computes

## What to build

`Layout::line` computes the pen of a line as `rows.saturating_mul(self.line_height)`. Its comment
says the product saturates where C's `int` overflows. The saturation came in with c39b8450, for a
panic the weekly fuzz runs of 2026-11-01 and 2026-11-08 found, when the line height was an `f64`
cast to `i64`. Since ef639d7a, `Layout::new` makes the line height `i64::from(int_cast(..))`, so
its magnitude is at most 2^31. The `i64` product overflows only past some 4.3e9 lines, which Q-9's
bounds on a label's text exclude. The saturation is dead code, and the comment describes nothing
the code does.

Make the product a plain `i64` multiplication. Rewrite the comment to say that where C's `int`
product overflows, which C leaves undefined, the pen holds the product's exact value. Name the
bound that keeps the `i64` product from overflowing. The rendered output does not change. The
maintainer chose this over modelling the low 32 bits of C's `int` (backlog entry
`label-pen-saturates-no-more`, triaged 2026-11-21).

## Acceptance criteria

- [ ] Given the tree, when `Layout::line` computes a pen, then it multiplies without
      `saturating_mul`, and its comment states the exact value and the bound that keeps it in
      `i64`.
- [ ] Given the seed `label-lines-past-an-int` of the render target, when its test runs, then it
      passes unchanged.
- [ ] Given the render references, when `scripts/check-all.sh` runs, then it passes with no
      reference changed.
- [ ] Given the doc of the path `label-lines-past-an-int` in `fuzz/tests/render_seeds.rs`, when it
      describes the pen, then it agrees with the new comment.

## Read first

- Touches: `crates/label/src/layout.rs`, `fuzz/tests/render_seeds.rs`
- `Layout::new` (the line height) and `Layout::line` (the pen) in `crates/label/src/layout.rs`.
- Q-9 of `docs/SPECIFICATION.md`, the bounds on a label's text.
- IR-c-ports-in-style-ir, which decides the casts of a port from C, not the overflow of a product.

## Blocked by

- None (can start immediately)

## Mode

unattended:

- A test decides it: the seed `label-lines-past-an-int` and the render references.
- Its scope is one function and its comment, and a wrong result costs one revert.
- It touches no ADR and nothing OPS-parallel-sessions-capped-at-three keeps with the human.
- A changed pen shows as a changed reference or a failed seed; the revert undoes it.
- Stop: if any reference or seed changes, hand off with the diff instead of accepting it.
