# Ticket: Bring the specification in line with the shapefile reader (drafted specification change)

## What to build

[ADR-0024](../../adr/0024-shapefile-reader.md) decided that the shapefile adapter reads with a
reader of its own, derived from MapServer's, and not with the `shapefile` crate; its first draft
had chosen that crate (#74). The specification's *Assumptions* still list `shapefile` among the
pure-Rust crates the work leans on, which #74 raised as a doubt for the maintainer. A second line
has drifted the same way: Q-3's measure names the fuzz targets "the mapfile parser, the YAML/JSON
loader, and the renderer", while the fuzz workspace also fuzzes the GeoJSON path and ADR-0024
makes the shapefile reader a target; the readers of data files are the inputs Q-3's first
sentence names ("mapfiles and data come from users").

Draft for the maintainer, in `docs/SPECIFICATION.md`:

> - The pure-Rust crates the work leans on — agg-rust, proj4rs, rustybuzz, unicode-bidi, skrifa,
>   i_overlay, tiff, flatgeobuf — build for `wasm32-unknown-unknown` without C code and stay
>   maintained. The ADR that picks each one verifies it.

> - **Q-3** — Robustness: mapfiles and data come from users, and a panic in the browser loses the
>   whole map. Invalid geometries, broken files, and errors of third-party crates become
>   diagnostics. Measured by a weekly scheduled fuzz run of 5 minutes per target — the mapfile
>   parser, the YAML/JSON loader, the reader of each data format a source adapter reads, and the
>   renderer — that ends without a panic.

Accepted ADRs whose basis it moves, for the maintainer to confirm: ADR-0024 (it is the reason for
the first change and names the fuzzing as its follow-up); ADR-0016 (the GeoJSON adapter, now a
named target); ADR-0008 (its slow-sensor workflow runs Q-3's measure, the weekly fuzz run, whose
list of targets the second change widens). The proposed ADR-0025 rests on Q-3's second
sentence, which the draft keeps.

## Acceptance criteria

- [ ] Given the maintainer's decision, when it lands, then the two lines change in a pull request
  of their own, apart from code, naming the ADRs it moves.
- [ ] Given the changed Q-3, when the traceability check runs, then each data reader's fuzz target
  cites Q-3 (`fuzz/fuzz_targets/geojson.rs` does; the shapefile target follows
  docs/plan/revision-2026-10-25/03-fuzz-the-shapefile-adapter.md).

## Read first

- *Assumptions* and Q-3 of `docs/SPECIFICATION.md`;
  [`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules), rule 7; the PR body of #74.

## Blocked by

- None (can start immediately)

## Mode

with the human: a specification change is the maintainer's
([`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules), rule 7).
