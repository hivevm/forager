# Ticket: smoothsia's window walk costs less, so three layers of one nested smoothsia end within the fuzz run's 10 s

## What to build

This ticket replaces the backlog entry `layers-of-one-smoothsia-at-the-fuzz-timeout`, which the
commit that filed it deleted. The maintainer chose this option in the design revision of 2026-11-19,
from the four the entry weighed. The other three each move a limit or the fuzz target, and they
stay open: a bound per render, a lower bound per layer, or fewer layers of one transform in the
render target.

**The finding.** After ticket 06 of backlog-deviations, an input of the render fuzz target still
sits at libFuzzer's 10 s timeout. The input is 750 bytes; the next section holds it. It decodes
into the three layers that share the target's `GEOMTRANSFORM`: the polygon, the line, and the
multipoint layer. All three are under four nested `smoothsia`, and every number is
`[map_cellsize] / NaN` except the outermost iterations,
`[map_cellsize] / 7.44748961190531e223`. The inner three take the preprocessing `all`, and the
outer one takes `angle`. Each layer is charged within `MAX_TRANSFORM_VISITS`:

| Layer | Its shape | Passes of the outer smoothsia | Visits charged |
| --- | --- | --- | --- |
| `multipoints` | a part of 5 points, 31 after the preprocessing | 168,037 of 93 | 15,627,594 |
| `polygons` | rings of 9, 0, 2, and 1 points, 141, 0, 2, and 1 after it | 168,037 of 432 | 72,592,632 |
| `lines` | the same rings | 168,037 of 432 | 72,592,632 |

The layers charge 160,812,858 visits together. A release build draws the input in about 0.87 s,
5.4 ns a visit. The fuzz build, under its sanitizer and coverage, took 6.2 to 9.8 s at loads of
17 to 44 on 24 cores, some 40 to 60 ns a visit, so it sits at 10 s; at loads of 41 to 61 and
above it took 13.5 to 16 s, up to 95 ns a visit. The charge counts the work: nothing
undercounts and nothing loops.

**What to build.** Make each visit of the window walk cheaper, in `Window::next`, `Window::point`,
and the loop of `Buffers::smooth`, so that the input ends well within 10 s on the fuzz build. The
walk's cost now is its per-neighbour `checked_sub` and `filter` options, and its indexed reads
through the window's indices. The fix keeps every limit, every charge, and the bits of every
result. It is a `fix` of the walk's cost, not a change of what it computes. Before the fix,
measure where the fuzz build's time goes, for example with `perf` on the replay. Cut from that
measurement, as the convention *A fix of a fuzz finding is cut from the cause its decoded input
shows* asks.

**Input** (750 bytes, base64; `bash scripts/fuzz.sh --replay render <file>` replays it):

```text
eyJ0eaiSOnjv7yB7Im71UGUnOnNsbWVjdAAGbAptb2IKCgoKCgoK7+/v7+9UVVVVVVUsICJmZWF0dWIKCgoKCuvr6+vr6+vr6wMA
AAAAAAAA6+vr6+vr6+vr6+vr6+vr6+vr6+vr63l5eXl5eXl5eXl5eXl5eXl5eXl5eXl5eXl5eXl5eXl5eXl5eXl5eXl5eXl5eXl5
eXl5eXl5eXl5nXl5eXl5eXl5eXl5eXl5eXl5eXl5eXl5eXl5eXl5eXl5eXl5eXl5eXl56+vr6+vr6+vrATf/////////////////
//////////////9B/wkA////////Onjv7yB7Im71UGUnOnNsbWVjdGklb24lJSUlJSUlIiy4DACs+gAA+/v7+/v7QPv7+/v7+/v7
+/v7+/v7+/sAAAAAAPv7+/v7+/sBAPv7+/v7+/v7+/v7+/v7AAAAQAD/////////////////////////////////////////////
/////////////////////////////////////wAAAAAAOmUiOiBwb2ludCBl///////////////////////3////////////////
/8dpbmUiOt4iK0ZlYXR1AAAAAAAAAAAAAAAAAAg6AAAA27//AAAAAAAAjgAAAJKcj9L/////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////wGluZSI63iIrRmVhdHVyZQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAENvbGxlY3SH
h4f4+Pj4+Pj4+Pj4+Pj4+Pj4+Pj4+Pj4+Pj4+Pj4+Pj4+Pj4+Pj4+Pj4+Pj4+Pj4+Pj4+Pj4+Pj4+Pj4+Pj4+IeHh4eHh4cgIodw
```

## Acceptance criteria

- [ ] Given the input above, when `bash scripts/fuzz.sh --replay render <file>` runs on the fuzz
  build of the base and of the fix, alternated five times each, so that both see the same
  load, then the median after the fix is at most 70 % of the median before it, and at most 7 s
  where the 1 minute load stays under the core count. The commit message states every time, the
  load beside each, and the share saved.
- [ ] Given 20,000 random parts and window sizes, when the old and the new walk both smooth them,
  then their results are bit-identical. The old walk is not kept as a second copy in the crate:
  the comparison runs once as a probe against the base commit, for example on results the base
  writes out, and the probe is committed under `crates/symbolize/` as a script or an example
  (see the convention *A measurement a later change is to repeat is kept in the tree*).
- [ ] Given the existing tests of `smoothsia`, among them those at the bound of
  `MAX_TRANSFORM_VISITS`, when they run, then they pass unchanged.
- [ ] Given the input above added as a seed of the render target, with its path in
  `fuzz/tests/render_seeds.rs`, when a 300-second run starts from `fuzz/seeds/render/`, then the
  run's first pass over the seeds reports no timeout. Any later finding of the run is filed in
  the backlog with its decoded input, not fixed here.

## Read first

- Touches: `crates/symbolize/src/layer_transform/smoothsia.rs` (after ticket 01), its tests
- `scripts/fuzz.sh` (`--replay`, the limits `-timeout=10 -rss_limit_mb=2048`)
- The merge of ticket 06 of backlog-deviations (`git log --merges --grep 'backlog-deviations/06'`):
  how its times were taken, and its review's times by load
- `docs/CONVENTIONS.md`: *A fix of a fuzz finding is cut from the cause its decoded input
  shows*; *A measurement a later change is to repeat is kept in the tree*
- ADR-0006 keeps the limits with the maintainer; ADR-0048 decides one budget per layer.

## Blocked by

- docs/plan/revision-2026-11-19/01-smoothsia-port-in-a-module-of-its-own.md

## Mode

unattended. The bit-identity test and the suite decide that nothing visible moves. The
replay's times decide the gain. A wrong result costs one revert. The ticket changes no limit, no
charge, and no bound's scope, so it touches nothing that ADR-0006 keeps with the human. Stop if
the walk cannot be made cheaper without changing a charge or a result's bits, or if the fix
leaves the replay above 7 s at a load under the core count. Hand off with the measurement: the
limit question then goes back to the maintainer.
