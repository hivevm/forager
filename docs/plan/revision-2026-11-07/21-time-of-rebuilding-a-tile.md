# Ticket: Decide what bounds the time of rebuilding a fill tile

## What to build

pixmap-symbols/09 bounds a tile's memory, not the time spent building tiles. Its ticket said both
"once per symbol, tile size, scale, and rotation in a render, never per feature" and "a seventh
dropping the one built longest ago", which conflict: with more than six tiles in turn, every drawing
misses the cache and rebuilds. One style is enough, as `SIZE [attr]` makes a tile per distinct
value. By reading its cache MapServer does the same; it was not timed. ADR-0040 leaves which fill
tiles are built and how many are kept out of its scope, so nothing decides this yet, and the
specification says never time without bound.

Measured in a release build, single-threaded, on a 256 × 256 map with 20 × 20 polygons, in the
review of 59a9790. The figures for a million features are extrapolated from small runs:

| Case | ms per feature | 1M features |
|---|---|---|
| One tile, SIZE 64 (baseline) | 0.004 | about 4 s |
| Seven styles in turn, SIZE 64–70 | 0.40 | 6.6 min |
| Seven styles, SIZE 256–262 | 5.0 | 1.4 h |
| Seven styles, SIZE 494–500 (under the default MAXSIZE) | 18.6 | 5.2 h |
| Seven styles, SIZE 3338–3344 (the worst that fits six in 256 MiB) | 962 | 267 h |
| Seven styles, GAP 3338–3344, scale 1 | 178 | 49 h |
| One style, `SIZE [s]` 3300–3313, all distinct | 141 | 39 h |
| One style, `ANGLE [a]`, SIZE 64 | 0.053 | 53 s |
| Plain fill covering a 3344² image, opaque / translucent | 11.8 / 20.2 | 3.3 / 5.6 h |

Wasm was not measured and is likely slower. The cost is finite and of the kind of a full-image fill
per feature, some 7 to 12 times dearer per style drawing, and independent of the image's size.

The maintainer chooses:

1. Accept it as MapServer's cost: state the cost in `docs/semantics/symbol.md`, whose "only the cost
   differs" says too little, and resolve the old ticket's wording in the pull request. The agent
   recommends this, unless Forager is to take untrusted mapfiles in a browser.
2. Add a bound on the pixels of tiles built in one render. That is a new limit of Q-9, whose number
   is the maintainer's, drafted as a specification change.

## Acceptance criteria

- [ ] Given the choice, when it lands, then `docs/semantics/symbol.md` and the code agree with it.
- [ ] Given a new limit, when a render passes it, then the style is not drawn, with the limit's
  diagnostic, and a test states the number.

## Read first

- `crates/symbolize/src/tile.rs`, `crates/render/src/tile.rs`;
  [ADR-0040](../../adr/0040-a-decoded-pixmap-is-held-once-per-render.md);
  `docs/SPECIFICATION.md` (Q-9).

## Blocked by

- None (can start immediately)

## Mode

with the human: a limit of Q-9 and the choice between the two are the maintainer's
([ADR-0007](../../adr/0007-harness.md)).
