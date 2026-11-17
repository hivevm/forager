# Ticket: A buffer of a shape whose edges are too short for i_overlay's offset neither panics nor vanishes

## What to build

The third 300-second run of ticket 01's render fuzz target, after ticket 17's fix, panicked about
13 900 runs in:

```text
thread '<unnamed>' panicked at i_float-1.16.0/src/adapter.rs:120:17:
You are trying to convert a point[-inf, -inf] which is out of rect: (-6.6e-163, -6.6e-163)-(6.6e-163, 6.6e-163)
```

The stack runs from `forager_engine::transformed` through `layer_transform::transformed` and
`Eval::shape` into `forager_symbolize::geometry::buffer`. From there it enters geo's
`buffer_with_style`, i_overlay's `OutlineBuilder` with round joins, and `OffsetSection::new`. A
print in `geometry::buffer`, not committed, decoded the input. A layer's transforms buffer the
point `(3.07847e-319, 0)` by `6.607605628962252e-163`, giving a circle of 121 points some
`1.3e-162` across. They then buffer that polygon by `9.633340286202275e-281`.

The cause is the length of the shape's edges, not the distance. i_overlay normalizes each edge's
direction `b − a` (`i_overlay-4.5.2/src/mesh/outline/builder.rs:189-193`). The ring's shortest edge
is `3.46e-164` long, so its squared length underflows to 0, and the direction comes out infinite.
i_float checks the point it converts only with debug assertions, as cargo-fuzz and `cargo test`
build. In a release build the same buffer panics nowhere and returns no polygon: a positive
distance makes the polygon vanish from the drawing. The second review reproduced both. It also
showed that the same ring buffered by `1e-163`, a distance about its own size, panics the same
way, and so does the line `(0, 0) (1e-164, 1e-164) (2e-164, 0)` buffered by `1e-164`.

`geometry::buffer` scales such a shape and its distance by one exact power of two, which brings
the shape's extent near 1. It buffers the scaled shape through geo's `buffer_with_style`, as
ADR-0041 decides, and scales the result back by the inverse power; a power of two scales without
rounding. Only a shape whose extent is so small that the squares of its edges can underflow, below
some `2^-500`, is scaled. Every other shape is buffered as today, so the shapes the tests and
msautotest buffer do not move. The charges to the
layer's budget do not change, since scaling changes no point count. Whether MapServer's GEOS keeps
such a shape is not confirmed. Where the session finds that i_overlay fails on shapes this does not
reach, it stops and says so.

The 142-byte input, for `cargo fuzz run render <file>` on the target of
`ticket/revision-2026-11-16/01-render-fuzz-fidelity-paths` (its target is unchanged since
0b002cbd):

```text
b8 0c 00 54 00 00 00 00 00 00 40 00 00 00 00 00
00 00 25 00 00 00 0c 00 54 00 00 00 00 00 00 40
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
00 00 00 00 00 25 40 00 00 00 00 00 00 00 00 e1
96 87 92 9c 8f d2 c0 69 6e 65 22 3a 20 22 2b 46
65 61 74 75 72 65 43 6f 6c 6c 65 63 74 87 87 87
87 87 87 87 87 87 87 87 87 87 87 87 87 87 87 21
87 87 87 87 87 87 87 87 87 87 ff 04 00 00 00 00
00 3a 00 00 21 04 70 6f 69 6e 74 20 65 f3
```

## Acceptance criteria

Each result below is compared after scaling it and its input by the same exact power of two to an
extent near 1, since geo's predicates underflow at this scale. "Covers" means that every vertex of
the input lies inside the result, or within a millionth of the extent of its boundary, the
tolerance i_overlay's integer grid needs.

- [ ] Given the point `(3.07847e-319, 0)` buffered by `6.607605628962252e-163`, and that ring
  buffered by `9.633340286202275e-281`, when `geometry::buffer` runs in a test, then it does not
  panic. It returns polygons with only finite coordinates, which cover the ring and lie within
  its bounds grown by the second distance. Before the change the test build panics, and a release
  build returns no polygon.
- [ ] Given that ring buffered by `1e-163`, and the line `(0, 0) (1e-164, 1e-164) (2e-164, 0)`
  buffered by `1e-164`, when `geometry::buffer` runs, then neither panics. Each returns polygons
  with only finite coordinates, which cover its input and lie within its bounds grown by its
  distance. Before the change each panics in the test build.
- [ ] Given the buffer tests of `crates/symbolize/src/geometry/tests.rs`, when they run, then they
  pass unchanged. These include GEOS's references, and the charges that
  `a_buffer_charges_its_offset_curves_before_it_runs_and_its_added_points_after` records with its
  `Recorder`. The msautotest cases with `buffer` pass unchanged too.
- [ ] Given a copy of ticket 01's target with this change merged, not that ticket's worktree and
  nothing committed, when the input above is run by `cargo fuzz run render`, then it does not
  panic. This run is what proves the fix against the defect the fuzzer found.

## Read first

- Touches: `crates/symbolize/src/geometry.rs` (`buffer`), `crates/symbolize/src/geometry/tests.rs`
- ADR-0041 (geometry through geo: `buffer` is geo's `buffer_with_style`), ADR-0042 (i_overlay in
  render-agg and geo); `docs/SPECIFICATION.md` Q-3 and Q-9; the `Charge` trait of `geometry.rs`
  and the tests' `Recorder`; `crates/symbolize/src/geometry/geos-buffer.txt` (GEOS's references).

## Blocked by

- None (can start immediately)

## Mode

unattended: the tests decide it, it changes one function of one crate, and a wrong result costs
one revert. The charges of `buffer` stay as they are. Stop if:

- a charge, or an assertion on a `Recorder`, has to change;
- the fix leaves geo's `buffer_with_style` for some inputs;
- a GEOS reference, an msautotest case, or any other existing test has to change;
- the fix needs a change to geo or i_overlay themselves.
