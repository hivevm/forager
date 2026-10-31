# Ticket: Let the notice check reach the files that follow MapServer's sources under `src/apps/`

## What to build

`scripts/check-notice.sh` holds a file to MapServer's notice when it names a source matching
`src/map[a-z_]*\.(c|h|l|y)`. Its header and
[ADR-0029](../../adr/0029-label-positions-from-mapservers-label-cache.md)'s Enforcement say it holds
the reader of the label cache, `crates/cli/tests/conformance/mapserver_labels.c`, to the notice of
`src/apps/map2img.c`, which it follows; but `src/apps/map2img.c` does not match, and the reader is
checked only because its header also names `src/mapdraw.c`. A port of `map2img.c` or another
program under `src/apps/` that names only that file is not checked. Read the notice `map2img.c`
carries at the pin first; where it is the Minnesota notice, widen the pattern to the sources under
`src/apps/`, and where it is another, say so and stop. The pnpoly notice of `msPointInPolygon` is
the open ticket `docs/plan/revision-2026-10-31/28-pnpoly-notice.md`, not this one.

## Acceptance criteria

- [ ] Given a fixture C or Rust file that names only `src/apps/map2img.c` and lacks the notice, when
  `scripts/test-check-notice.sh` runs, then the check refuses it with its message.
- [ ] Given the tree as it is, when `scripts/check-notice.sh` runs, then it passes and counts the
  reader among the files checked.

## Read first

- `scripts/check-notice.sh`, `scripts/test-check-notice.sh`; `src/apps/map2img.c` of MapServer at
  the pin; ADR-0029; the licence constraint of `docs/SPECIFICATION.md`.

## Blocked by

- None (can start immediately)

## Mode

with the human: it decides which MapServer files carry which notice, and the licences are the
maintainer's ([ADR-0007](../../adr/0007-harness.md)).
