# Ticket: Q-3 says what a guarded input comes to and that each fuzz finding keeps a test

## What to build

A drafted change of the specification, for the maintainer to decide
([`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules), rule 7). An agent drafts it and never lands it.

Q-3 reads: "Invalid geometries, broken files, and errors of third-party crates become
diagnostics", measured by the weekly fuzz run. Two things the tests now decide are not in its
text:

- **What a guarded input comes to.** Since the revision of 2026-11-16, five fuzz findings in
  agg-rust, geo, and i_overlay were kept out by guards. One fails the layer with a diagnostic
  (5a003cb0). One leaves a piece of a brushed line undrawn without a diagnostic (44a91d4e). One
  raises a line's buffer distance to a floor, a drawing GEOS does not make (cb56e3ba). Two move
  nothing (8bb2ed83, a13dc8bb). Q-3's "become diagnostics" holds the first only.
  `docs/adr/draft-third-party-failures-guarded.md` records the maintainer's answer that a
  refusal warns on its layer, and binds every guard, the older ones of `render` and `render-agg`
  included.
- **The regression tests.** Some 29 markers under `crates/` and every fuzz target cite `Verifies:
  Q-3`. Each test under `crates/` decides one input, not the weekly run Q-3 is measured by. The
  convention "A `Verifies:` marker claims every *shall* of its criterion" lets a test cite a part
  only where the criterion asks for a test per item, as Q-8 and Q-9 do. Q-3 does not ask for one.

The draft, which the maintainer chose as it stands on 2026-11-18:

> **Q-3** — Robustness: mapfiles and data come from users, and a panic in the browser loses the
> whole map. Invalid geometries, broken files, and errors of third-party crates become
> diagnostics; an input a guard leaves undrawn to keep a third-party crate from failing is a
> diagnostic too. Each failure a fuzz run finds is held by a test of its input. Measured by a
> weekly scheduled fuzz run of 5 minutes per target […, as today].

The accepted ADRs whose basis it moves: ADR-0046 (its *No panic*
clause decides the brushed start's reductions only, and its *What is not drawn* asks a refused
tile to warn), ADR-0025 (the renderer's safety rests on agg-rust and the clipping), ADR-0041
(`buffer` is geo's `buffer_with_style`), and ADR-0002 with the convention on `Verifies:` markers,
whose list of criteria with a test per item gains Q-3.

## Acceptance criteria

- [ ] Given the draft, when the ticket lands, then `docs/SPECIFICATION.md` carries it word for
  word.
- [ ] Given the wording landed, when `scripts/check-traceability.sh` runs, then it passes.
- [ ] Given the wording landed, when `docs/CONVENTIONS.md` is read, then the `Verifies:` entry
  names Q-3 among the criteria with a test per item.

## Read first

- Touches: `docs/SPECIFICATION.md` (Q-3), `docs/CONVENTIONS.md` (the `Verifies:` entry)
- `docs/adr/draft-third-party-failures-guarded.md`; ADR-0002, ADR-0025, ADR-0041, ADR-0046
  (*Decision*); the merge messages 2156170e and 53421100.

## Blocked by

- None (can start immediately). It is decided with ADR-draft-third-party-failures-guarded.

## Mode

with the human: a change of the specification is the maintainer's
([`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules), rule 7; ADR-0006).
