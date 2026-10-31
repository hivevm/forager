# Ticket: Keep W. Randolph Franklin's notice where `msPointInPolygon` is ported

## What to build

`msPointInPolygon` in `src/mapsearch.c` at the pin is not under the notice of the Regents of the
University of Minnesota that the rest of the file carries: it sits under W. Randolph Franklin's
pnpoly licence ("Copyright (c) 1970-2003, Wm. Randolph Franklin"), whose condition 1 asks that a
redistribution of source keeps his copyright notice, its conditions, and its disclaimers, and whose
condition 2 asks that a redistribution in binary form reproduces them in its documentation.
`crates/label/src/cache.rs` ports it, and the pairwise reference of
`crates/label/tests/cache_pairwise.rs` (#135) ports it again; neither carries his notice, and
`scripts/check-notice.sh` checks only the Minnesota one, so it does not see the gap. Carry the
notice where the function is ported, say where a binary of Forager reproduces it, and have the
notice check hold a file that follows `msPointInPolygon` or names `pnpoly` to it.

## Acceptance criteria

- [ ] Given `crates/label/src/cache.rs` and every other file that ports `msPointInPolygon`, when
  they are read, then each carries Franklin's copyright line, his three conditions, and his
  disclaimer, beside the function it ports.
- [ ] Given a file under `crates/` or `fuzz/` that names `msPointInPolygon` or `pnpoly` without
  Franklin's notice, when `scripts/check-notice.sh` runs, then it fails naming the file; its
  self-test holds both cases, with and without the notice.
- [ ] Given a binary of Forager, the CLI or the browser bindings, when its documentation is read,
  then it reproduces Franklin's notice as condition 2 asks, in a place the maintainer chooses.

## Read first

- `src/mapsearch.c` at the pin (the comment above `msPointInPolygon`); `scripts/check-notice.sh`
  and `scripts/test-check-notice.sh`; the licence constraint of
  [`docs/SPECIFICATION.md`](../../SPECIFICATION.md) (Q-6).

## Blocked by

- None (can start immediately)

## Mode

with the human: it is a matter of licences, which [ADR-0007](../../adr/0007-harness.md) keeps with
the maintainer, and where a binary's documentation reproduces the notice is the maintainer's
choice.
