# Ticket: A layer's `smoothsia` passes over the parts it smooths, so that its time stays bounded

## What to build

The second 300-second run of ticket 01's render fuzz target ended on a timeout. The input runs for
110 seconds under cargo-fuzz's AddressSanitizer build and 11 seconds with `--sanitizer none`, past
Q-3's 10 seconds either way. The stack runs from `forager_engine::transformed` through
`layer_transform::transformed`, `evaluated`, and `Eval::shape` into `smooth_sia`, which
allocates.

Decoded by a print in `smooth_sia`, not committed: the fuzz target's polygon and line layers each
call it once, on a shape of three parts of 0, 0, and 1 points, at a size of 3 and `ITERATIONS`
of 89 409 258. `smooth_sia` (`crates/symbolize/src/layer_transform.rs`) charges each pass
`points × min(size, points)` visits of the whole shape, at least 1, against
`MAX_TRANSFORM_VISITS` (100 million per layer), so each layer's 89 409 257 passes stay within it.
Every pass collects a new vector of the parts, and `smooth_part` copies each part of fewer than 3
points as it is: some 60 nanoseconds a pass for a pass that changes nothing, and some 179 million
passes over the two layers.

Each pass smooths only the parts of 3 points or more. A part of fewer is kept as it is, read once,
as `smooth_part` returns it unchanged; a shape with no such part runs no pass. What a pass then
costs is the smoothing of its parts, which the charge counts already. MapServer's
msSmoothShapeSIA runs every pass, so what is drawn does not change, nor does what Q-9 counts. A
pass still allocates for each part it smooths, which the charge of at least 9 visits a part covers
within the bound. Where the session finds a cost a pass does not charge for, it stops and says so.

The 142-byte input, for `cargo fuzz run render <file>` on the target of
`ticket/revision-2026-11-16/01-render-fuzz-fidelity-paths` (its target is unchanged since
0b002cbd):

```text
b8 1c 00 54 00 00 00 00 00 00 40 00 00 00 00 00
00 00 25 00 00 00 0c 00 54 00 00 00 00 00 00 40
00 00 00 00 00 00 00 25 00 00 00 00 00 00 00 00
90 87 92 96 9e 66 65 8f d2 91 69 6e 65 3a 23 3a
20 22 00 46 65 61 61 74 75 74 75 72 65 43 6f 6c
6c 65 63 74 87 87 87 87 87 87 87 87 87 ff ff ff
ff ff ff ff 22 2d 66 87 87 87 87 87 7e 87 87 87
87 87 87 87 0c 00 54 00 00 00 00 00 00 40 00 00
00 00 00 00 00 25 00 00 00 00 00 00 00 00
```

## Acceptance criteria

- [ ] Given a shape of parts of 0, 0, and 1 points and `smoothsia` at a size of 3 with
  `ITERATIONS` of 89 409 258, when `smooth_sia` runs in a test, then it returns the shape as it
  was within a second. Before the change it runs some 89 million passes.
- [ ] Given a shape of one part of 4 points and 1 000 empty parts and `smoothsia` at a size of 3
  with `ITERATIONS` of 1 000 000, when `smooth_sia` runs in a test, then it returns within a
  second the 4-point part as a shape of it alone is smoothed, and the empty parts as they were.
  Before the change each of its passes copies the 1 000 empty parts.
- [ ] Given the `smoothsia` tests and the msautotest cases that hold it today, when they run, then
  they pass unchanged.
- [ ] Given ticket 01's branch rebased onto this change, when the input above is run by
  `cargo fuzz run render`, the AddressSanitizer build `scripts/fuzz.sh` runs, then it ends within
  10 seconds.

## Read first

- Touches: `crates/symbolize/src/layer_transform.rs`,
  `crates/symbolize/src/layer_transform/tests.rs`
- `docs/SPECIFICATION.md` Q-3 and Q-9 (the points a layer's transforms visit beyond reading each
  once); ADR-0048 (the transform tree, `smoothsia` among its steps, and one budget shared by a
  layer and its styles); `MAX_TRANSFORM_VISITS` and `Budget` in `layer_transform.rs`.

## Blocked by

- None (can start immediately)

## Mode

with the human: on the maintainer's word, since it changes how a transform that Q-9 bounds spends
its time, and ADR-0006 keeps the resource limits from unattended work.
