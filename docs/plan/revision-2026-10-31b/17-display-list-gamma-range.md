# Ticket: Hold a gamma the display list carries to what MapServer accepts

## What to build

Since #134, a `DrawOp::Fill` and a `Hatch` of `crates/render` carry `gamma: Option<f64>`, and
`render-agg` passes coverage through AGG's linear gamma for `Some`. The engine's one producer is
`GAMMA` in `crates/engine/src/draw.rs`, MapServer's default 0.75. Two values say "no gamma",
`None` and `Some(1.0)`, which draw alike; `Some(NaN)` draws nothing; and a gamma at or below 0, or
at or above 1, is drawn as given, where `agg2CreateImage` (`src/mapagg.cpp` at the pin) replaces it
with 0.75. No ticket carries a format's or a layer's `GAMMA` yet, so nothing feeds such a value
today; the first one that does would draw it. Questions for the maintainer: does the display list
hold a type that only admits MapServer's range, built where the value enters, or does the engine
clamp it where it reads `GAMMA`, leaving `render` a plain number; and is the missing gamma one state
or two.

## Acceptance criteria

- [ ] Given a gamma of 0, -1, 1, 2, or NaN handed to the display list by the chosen path, when
  rendered, then it draws as MapServer's 0.75 draws, a test per value.
- [ ] Given the polygons and hatches of today, when rendered, then they are byte for byte those of
  today and no reference image is re-blessed.

## Read first

- `crates/render/src/lib.rs`, `hatch.rs`; `crates/render-agg/src/lib.rs`; `GAMMA` in
  `crates/engine/src/draw.rs`; `agg2CreateImage` in MapServer's `src/mapagg.cpp` at the pin;
  [ADR-0032](../../adr/0032-hatch-cut-by-i-overlay.md),
  [ADR-0034](../../adr/0034-render-agg-blends-as-agg-2-4.md);
  #134's doubts (`gh pr view 134`).

## Blocked by

- None (can start immediately)

## Mode

with the human: it changes a public type of the display list, and whether the check stands in
`render` or in the engine is a choice of the crates' responsibilities. It is cheapest folded into
the ticket that first carries a format's or a layer's `GAMMA`, once one is filed.
