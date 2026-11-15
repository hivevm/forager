# Ticket: Propose the ADR for G-11's reference rectangles and shapes

## What to build

ADR-0038 decides one list, `crates/proj/tests/reference/points.tsv`, reprojected by
`msProjectPoint` into one record, `mapserver.tsv`. Its *Out of scope* says that the ortho and
`+over` cases "add lines to the list". The work grew past it:

- `crates/cli/tests/conformance/mapserver_proj.c:16-28` also reprojects rectangles by
  `msProjectRect` and shapes by `msProjectShape`. Both projections are loaded afresh for each
  rectangle, so that no `is_polar` is kept (see ticket 05).
- `crates/cli/tests/conformance/noise/mod.rs:128-142` runs two more pairs: `rects.tsv` and
  `mapserver-rects.tsv`, `shapes.tsv` and `mapserver-shapes.tsv`.
- `crates/proj/tests/reference.rs` holds the rectangles and the shapes within G-11's tolerance.
  The ortho and `+over` cases went into `shapes.tsv`, not into the list.

No ADR names the two new lists, their records, or the reader's fresh load. ADR-0038 is accepted,
so the decision is superseded, not edited. Run `propose-adr` with the maintainer: one ADR that
supersedes ADR-0038 and decides the three lists, their records, the three MapServer functions the
reader calls, and how it loads projections. It travels in a commit of ADRs only.

## Acceptance criteria

- [ ] Given the maintainer's answers, when the ADR is submitted, then it is `proposed`, supersedes
  ADR-0038, names each list and record under `crates/proj/tests/reference/`, and is in a commit of
  ADRs only.
- [ ] Given `scripts/check-docs.sh`, when it runs on the ADR's branch, then it passes.

## Read first

- Touches: `docs/adr/`, `docs/adr/README.md`
- `docs/adr/0038-proj-references-from-mapserver.md` whole, `docs/adr/0033-noise-measurement.md`;
  the skill `propose-adr`.

## Blocked by

- docs/plan/revision-2026-11-15/18-spec-g11-what-it-measures.md

## Mode

with the human: an ADR is developed with the human
([`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules), rule 3).
