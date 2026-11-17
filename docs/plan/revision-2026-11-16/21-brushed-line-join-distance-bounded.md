# Ticket: A brushed line's piece whose join distances overflow agg-rust's `i32` is not drawn

## What to build

A 60-second run of ticket 01's render fuzz target from its seeds at 753648ee, by the review of
ticket 20, panicked:

```text
agg-rust-1.2.0/src/renderer_outline_image.rs:456:24: attempt to multiply with overflow
```

That line computes `dist_start`, and the one after it `dist_end`, in `DistanceInterpolator4::new`:
the difference of the pixel and the join point `sx` (or `ex`) in `line_mr` units, 1/16 pixel,
times the difference of that join point and the segment's end, in the same units, in `i32`. A
print in `brushed_line` (`crates/render-agg/src/brush.rs`), not committed, decoded the input: an
image of 123 × 34 pixels, a pattern 256 wide and a line width of 256 subpixels, a `reach` of
`1.517439853e9`, so the clip keeps the path whole, and the closed path
`(94.00000786351512, -7.817504489577309e-6) (61.000000000000114, 33.00000000000001)
(-8584.616083133831, -0.003023169095998576) (61.00000000000001, 56.17101458477057)
(94.00000786351512, -7.817504489577309e-6)`. Its segments run some 8650 pixels, about 138 000
`line_mr` units, whose product passes `i32::MAX`.

`within_i32` already keeps a piece from agg-rust's renderer where its start and its distance along
the segment would pass `i32`, and the piece is not drawn, since AGG's C++ is undefined there (Q-3,
ticket 16 for the glyph outline's width). It bounds sums of the longest segment, not the products
of `DistanceInterpolator4`. The bound grows to cover them: a piece whose products of
`dist_start` and `dist_end`, and of the other interpolators `renderer_outline_image` builds for
`line1`, `line2`, and `line3`, could pass `i32` is not drawn either. The session derives the
bound from agg-rust's code, including how far the join points `sx`, `sy`, `ex`, `ey` lie from the
segment's ends (`bisectrix` and `fix_degenerate_bisectrix`), and states it in `within_i32`'s doc
comment. Every piece the tests and msautotest draw today stays drawn.

The 288-byte input, for `cargo fuzz run render <file>` on the target of
`ticket/revision-2026-11-16/01-render-fuzz-fidelity-paths` at 753648ee
(`base64 -d > crash-21.bin`):

```text
eyJ0eXBlIjogIkJlYXR1cmVDb2xsZWN0aW9uIiwgdXV1dXV1dfX19fX19fX1InBlb21lK3RyeQoic3ltYu9sCgoKsgoKCgoK
7yB7Im719fX19fX19fX19XV1dXV1dXV1dXV1dXX19fX19fUrdHJ5IgpzeXkiCnN5bWLvbAoKCrIKCgoKCu8geyJu9fX19fX1
9fX19fV1dXV1dXV1dXJlizp47+/geyJu9VBlIjpzbG1lY3Rpb24lJSUlJSUlIiwDICJwZW9tZQEvoL6C+gvoAAAAAAAAQAZs
Cm1db2IKCgoKCgoKrd9FTkVJ1PqFf39/9fUxJG71UGUiQ29wb2ludP///4y6urq6urq6urq6urq6urq6b2UAAKenp6enp6en
```

Its SHA-1 is `fc1ca228d300917dd15c038de303d0d44d5a601c`.

## Acceptance criteria

- [ ] Given the path above brushed with a pattern 256 wide and a line width of 256 subpixels on an
  image of 123 × 34 pixels, when `brushed_line` runs in a test of `crates/render-agg`, then it does
  not panic. Before the change the test build panics at `renderer_outline_image.rs:456`.
- [ ] Given a brushed piece whose segments lie just within the new bound, when it is drawn, then it
  is drawn and does not panic; given one just past it, then it is not drawn. The tests name the
  bound's terms.
- [ ] Given the tests of `crates/render-agg` and `crates/engine` and the msautotest harness, when
  they run, then they pass unchanged.
- [ ] Given a copy of ticket 01's target at 753648ee, not that ticket's worktree and nothing
  committed, when the input above is run by `cargo fuzz run render`, then it does not panic.

## Read first

- Touches: `crates/render-agg/src/brush.rs` (`within_i32`, `brushed_line`) and its tests
- `agg-rust-1.2.0/src/renderer_outline_image.rs` (`DistanceInterpolator4`, `line1`–`line3`) and
  `line_aa_basics.rs` (`line_mr`, `bisectrix`, `fix_degenerate_bisectrix`); ADR-0027 (the clip's
  reach); `docs/SPECIFICATION.md` Q-3; ebc0a4c5, which added `within_i32`.

## Blocked by

- None (can start immediately)

## Mode

unattended: the tests decide it, it changes one function of one crate, and a wrong result costs
one revert. It touches no resource limit. Stop if:

- an existing test or an msautotest case has to change, or a piece they draw is no longer drawn;
- the fix needs a change to agg-rust itself;
- the bound cannot be derived from agg-rust's code without guessing a term.
