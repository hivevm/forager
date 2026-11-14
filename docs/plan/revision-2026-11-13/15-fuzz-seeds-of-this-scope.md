# Ticket: Bring the fuzz seeds this scope's new readers and paths left out

## What to build

The convention *A change that gives a target of Q-3 a new reader or a new path brings the fuzz seed
that reaches it* had its occasion several times in this scope and was not followed: of the range
b310803..51d3133, only `fuzz/Cargo.lock` changed under `fuzz/`. The layer's `GEOMTRANSFORM` parser
of 44cc731 overflowed the stack on a flat chain of operators, found by review, not by a seed, and
the font set's reading of 5ed366c was quadratic at Q-9's bound until review. Bring a seed for each
path still in the tree:

- the mapfile target (`fuzz/fuzz_targets/mapfile.rs`, which answers every file it loads with the
  same bytes): a `FONTSET` and its lines with GDAL's line ends, `MAXFEATURES` and the
  `maxfeaturestodraw` metadata, `POSTLABELCACHE`, `labelcache_map_edge_buffer`, and a class's
  `GEOMTRANSFORM` expression;
- the render target (`fuzz/fuzz_targets/render.rs`): its structured input reaching a layer's
  `max_features`, `post_label_cache`, the label cache's edge buffer, and a label's `labelpoint` and
  `labelcenter` pixmap styles, extending the input where it cannot reach them.

## Acceptance criteria

- [ ] Given each path above, when `scripts/fuzz.sh 30` runs, then its target reaches the path from
  a seed, shown by a coverage run or a seed that a debug assertion at the path would fail on.
- [ ] Given the extended render input, when the target runs over the existing seeds, then they
  still decode.

## Read first

- Touches: `fuzz/fuzz_targets/render.rs`, `fuzz/seeds/mapfile/`, `fuzz/seeds/render/`
- `docs/CONVENTIONS.md`, the fuzz seed entry; `scripts/fuzz.sh`; `docs/adr/0007-rust-toolchain.md`.

## Blocked by

- None (can start immediately)

## Mode

unattended: the fuzz run decides it, and a seed costs one revert. Stop when a seed finds a crash:
hand it off as a backlog entry with the artifact, without fixing it on this ticket.
