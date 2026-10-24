# Ticket: Decide the feature and data file limits of Q-9 (drafted specification change)

## What to build

Q-9 names two of its numbers, 16 MiB of mapfile text and 1000 diagnostics, and its other limits
are the implementer's: `MAX_FEATURES_PER_LAYER = 1_000_000` (`crates/engine/src/lib.rs:24`) and
the CLI's `MAX_SOURCE_BYTES = 256 MiB` per GeoJSON file (`crates/cli/src/main.rs:206`), which #5
listed as awaiting the maintainer and no change since has decided. Q-9's assumption says its
limits are set from a first measurement. Measured on `main` 9d2a9bc (release build): a GeoJSON
file of 245 MiB with 2,200,000 line features peaks at 1.1 GB resident in `forager render`, and the
layer is skipped with `layer-skipped` only after the source has delivered every feature. So the
feature limit bounds the drawing, not the memory: memory is bounded by the host's file limit in
the CLI, and in the browser by nothing but the page's own tab, since the bindings take GeoJSON
text of any size. Neither limit has the test per limit Q-9 asks for: the only limit test in
`crates/engine/tests/render.rs` is the image size (line 165), and `crates/cli/tests/render.rs` has
none.

Draft for the maintainer, in Q-9 of `docs/SPECIFICATION.md`:

> - **Q-9** — Bounded resources: `INCLUDE` depth and cycles, the total size of a mapfile with its
>   `INCLUDE`s (16 MiB of text), the number of diagnostics one load reports (1000), image size,
>   the features one layer draws (1,000,000), the size of a data file a host reads (256 MiB),
>   expression nesting, and regular-expression size are limited, and exceeding a limit is a
>   diagnostic. A test per limit feeds an input past it and expects the diagnostic.

The numbers are the ones in the code; the maintainer may set others from the measurement above.
Whether a feature source should stop at the limit while it reads, rather than the engine after
it, is the question the draft leaves open; answering yes changes the `FeatureSource` port.

Accepted ADRs whose basis it moves, for the maintainer to confirm: none derive their decision
from Q-9's numbers; [ADR-0007](../../adr/0007-harness.md) and
[ADR-0015](../../adr/0015-agg-rust-rasterizer.md) cite Q-9 for the existence of limits.

## Acceptance criteria

- [x] Given the maintainer's decision, when it lands, then Q-9 names the two limits in a pull
  request of its own, apart from code.
- [ ] Given a source that delivers one feature more than the decided feature limit, when the
  engine renders its layer, then the layer is skipped with `layer-skipped` naming the limit
  (`crates/engine/tests/render.rs`, citing Q-9).
- [ ] Given a GeoJSON file one byte past the decided file limit, when `forager render` binds it,
  then the layer is skipped with `source-unreadable` naming the limit
  (`crates/cli/tests/render.rs`, citing Q-9; a sparse file keeps the fixture small).
- [ ] Given decided numbers other than those in the code, when the change lands, then the
  constants match them.

## Read first

- Q-9 and *Assumptions* of `docs/SPECIFICATION.md`;
  [`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules),
  rule 7; the PR bodies of #5 (*Doubts*) and #9.

## Blocked by

- None (can start immediately)

## Mode

with the human: a specification change and the resource limits are the maintainer's
([`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules), rule 7; [ADR-0007](../../adr/0007-harness.md)).
