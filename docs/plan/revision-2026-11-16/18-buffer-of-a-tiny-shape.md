# Ticket: A buffer of a shape too small for i_overlay's offset keeps the shape it grows

## What to build

The third 300-second run of ticket 01's render fuzz target, after ticket 17's fix, panicked about
13 900 runs in:

```text
thread '<unnamed>' panicked at i_float-1.16.0/src/adapter.rs:120:17:
You are trying to convert a point[-inf, -inf] which is out of rect: (-6.6e-163, -6.6e-163)-(6.6e-163, 6.6e-163)
```

The stack runs from `forager_engine::transformed` through `layer_transform::transformed` and
`Eval::shape` into `forager_symbolize::geometry::buffer`, then geo's buffer, i_overlay's
`OutlineBuilder` with round joins, and `OffsetSection::new`. A print in `geometry::buffer`, not
committed, decoded the input. A layer's transforms buffer the point `(3.07847e-319, 0)` by
`6.607605628962252e-163`, a circle of 121 points some `1.3e-162` across. They then buffer that
polygon by `9.633340286202275e-281`.

A test in a scratch copy, not committed, ran that second buffer through `geometry::buffer`:

- with debug assertions, as cargo-fuzz and `cargo test` build, it panics as above;
- in a release build it panics nowhere and returns no polygon at all. A positive distance that
  grows a polygon makes it vanish from the drawing.

A likely cause, not confirmed: an offset section divides by the length of an edge some `1e-164`
long, whose square underflows to 0, so the direction comes out infinite. i_float checks the
point it converts only with debug assertions.

The buffer keeps what it grows. A shape and a distance too small for i_overlay's offset are
brought into a range it offsets, by scaling or another means the session finds, and the result is
brought back. Alternatively the shape is kept as it is where the distance is too small to move a
point of it. The result covers the shape, lies within the shape grown by the distance, and holds
only finite coordinates. MapServer's GEOS buffers such a shape in its own arithmetic and keeps
it. Where the session finds that i_overlay fails on shapes this does not reach, it stops and says
so.

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

- [ ] Given the point `(3.07847e-319, 0)` buffered by `6.607605628962252e-163`, and that result
  buffered by `9.633340286202275e-281`, when `geometry::buffer` runs in a test, then it does not
  panic, and it returns polygons that cover the first result's ring, lie within its bounds grown
  by the second distance, and hold only finite coordinates. Before the change the test build
  panics, and a release build returns no polygon.
- [ ] Given a point, a line, and a polygon of extent near 1 buffered by `1e-300`, when
  `geometry::buffer` runs, then the same holds of each result. This guards the means the fix
  takes; whether it fails before the change is not known.
- [ ] Given the buffer tests of `crates/symbolize/src/geometry/tests.rs`, GEOS's references
  among them, and the msautotest cases with `buffer`, when they run, then they pass unchanged.
- [ ] Given ticket 01's target with this change merged in a scratch worktree, nothing committed
  there, when the input above is run by `cargo fuzz run render`, then it does not panic.

## Read first

- Touches: `crates/symbolize/src/geometry.rs` (`buffer`), `crates/symbolize/src/geometry/tests.rs`
- ADR-0041 (geometry through geo), ADR-0042 (i_overlay in render-agg and geo);
  `docs/SPECIFICATION.md` Q-3; `crates/symbolize/src/geometry/geos-buffer.txt` (GEOS's
  references).

## Blocked by

- None (can start immediately)

## Mode

unattended: the tests decide it, it changes one function of one crate, and a wrong result costs
one revert. Stop if a GEOS reference, an msautotest case, or any existing test has to change, or if
the fix needs a change to geo or i_overlay themselves.
