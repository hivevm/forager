# Ticket: The fuzz seeds and the target the reprojection's readers left out

## What to build

The convention *A change that gives a target of Q-3 a new reader or a new path brings the fuzz
seed that reaches it* had its occasion in the reprojection work, and left these gaps:

- the four seeds of `fuzz/seeds/style_document/` are all at version 1.0, and none carries format
  1.1's `projection_auto`;
- no seed under `fuzz/seeds/mapfile/` holds `PROJECTION AUTO`, of the map or a layer;
- `fuzz/seeds/prj/` holds `lcc`, `utm32n`, and `wgs84` only. The reader also takes Mercator 1SP
  and 2SP, the 1SP conic with its `Scale_Factor`, OGC's `AXIS`, and `LOCAL_CS` (9a2b860c);
- `proj`'s proj4 parser and its transforms read strings from mapfiles and style documents, and
  no target reaches them: `fuzz/fuzz_targets/mapfile.rs:20-25` resolves every `init=` to
  longitude and latitude, and the render target builds no projection.

Add the seeds, and a target `fuzz/fuzz_targets/proj.rs` that parses arbitrary strings and
reprojects arbitrary points, rectangles, and shapes between two of them.

## Acceptance criteria

- [ ] Given `scripts/fuzz.sh` run briefly over each target, when it starts, then the new seeds
  load and the target `proj` runs.
- [ ] Given the `proj` target, when it runs for 10 minutes locally, then no panic is found, or
  the panic is filed with its seed as its own ticket.

## Read first

- Touches: `fuzz/Cargo.toml`, `fuzz/fuzz_targets/proj.rs`, `fuzz/seeds/`
- Q-3 in `docs/SPECIFICATION.md`; `docs/adr/0037-reprojection-ported-in-proj.md` (*Failure*);
  `docs/CONVENTIONS.md`, the fuzz seed entry.

## Blocked by

- None (can start immediately)

## Mode

unattended: the fuzz run decides it, and a seed or a target costs one revert. Stop after 10
minutes of fuzzing; a panic found is a defect for its own ticket.
