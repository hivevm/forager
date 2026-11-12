# Ticket: Decide the direction of imports between a crate's modules

## What to build

[ADR-0008](../../adr/0008-ports-and-adapters-crates.md) decides the dependency edges between
crates only. `symbolize` holds its modules to one direction, stated in its `lib.rs` and held by
`crates/arch-test/tests/symbolize_modules.rs`, and no ADR decides that rule. `engine` has none and
two of its modules import each other: `labels` uses `label_styles`, and `label_styles.rs:245,256`
call `crate::labels::push` and `make_room`. Its modules also take helpers from the crate's root
(`layer_opacity`, `points_in_view`, `quoted`, `Selected`), which is ordinary Rust but leaves no
leaf module for them. Run `propose-adr` with the maintainer on whether modules within a crate
import in one direction, and for which crates; the catalogue's `concepts/layered.md` is the nearest
entry. If it is accepted, extend `arch-test` to the crates it names and break the engine's cycle.

## Acceptance criteria

- [ ] Given the maintainer's decision, when it is recorded, then a `proposed` ADR states it, or
  the ticket closes with the reason no ADR is due.
- [ ] Given an accepted ADR that names `engine`, when one of its modules imports one the rule puts
  above it, then `arch-test` fails.

## Read first

- Touches: `docs/adr/`, `crates/arch-test/tests/`, `crates/engine/src/`
- `.agents/skills/propose-adr/SKILL.md` and `concepts/layered.md`; the `symbolize` paragraph of
  `docs/ARCHITECTURE.md`.

## Blocked by

- None (can start immediately)

## Mode

with the human: it takes a decision no ADR in force decides, and the ADRs are the maintainer's
(ADR-0006).
