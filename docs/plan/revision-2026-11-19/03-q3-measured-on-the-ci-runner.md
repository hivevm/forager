# Ticket: Q-3 says on which machine its fuzz run's 10 s is measured (specification change, drafted)

## What to build

Q-3 is met by "a weekly scheduled fuzz run of 5 minutes per target … that ends without a panic, a
crash, a hang past 10 s, or more than 2 GiB of memory". It does not say on which machine the 10 s
hold. The fuzz build's times move with the machine's load. In the work of backlog-deviations,
one input of the render target took 6.2 s at a load of 44 and 16 s at a load of about twice the
cores. Ticket 06's own input took 5.0 s and 12.7 s on one build, minutes apart. On a shared
development machine a local run decides a timeout by its load as much as by its input. Q-3 names
the weekly run, and only `.github/workflows/fuzz.yml` runs it, on a runner of its own. The
specification does not say that this run is the measure. Sessions have taken local runs on a
loaded machine as Q-3's verdict, and ticket 03 of backlog-deviations stopped on such a run.

A change to the specification is the human's
([`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules), rule 7). This ticket carries the drafted text
and the ADRs it would move. The human decides and lands it.

**Draft**, Q-3's last sentence, in place of "Measured by … more than 2 GiB of memory.":

> Measured by the weekly scheduled fuzz run in CI, 5 minutes per target on the runner its
> workflow names — the mapfile parser, the YAML/JSON loader, the reader of each data format a
> source adapter reads, the expression evaluator, the reprojection, the reader of fonts, the
> decoder of symbol images, and the renderer — that ends without a panic, a crash, a hang past
> 10 s on that runner, or more than 2 GiB of memory. A run elsewhere finds inputs, and a panic or a
> crash it finds counts as the weekly run's would. A time it takes decides no hang.

**Accepted ADRs whose basis it may move**, for the human to judge
([`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules), rule 7):

- [ADR-0007](../../adr/0007-rust-toolchain.md) names the weekly fuzz run of Q-3 and
  `scripts/fuzz.sh` as its tooling. Its decision stands. Its context reads Q-3 as it is.
- [ADR-0049](../../adr/0049-third-party-failures-guarded.md) restates Q-3's limits, "a hang past
  10 s", in its context. The restatement goes stale, and its decision is unaffected.
- [ADR-0006](../../adr/0006-harness.md) keeps the limits with the maintainer. The draft moves no
  limit. It names where one is measured.

The draft leaves open what a hang on the runner then asks. The weekly run has failed four times
in a row, on 2026-10-25, 11-01, 11-08, and 11-15 (ticket 05). The specification says nothing of a
run that fails and is not looked at.

## Acceptance criteria

- [ ] Given this draft, when the human has decided it, then `docs/SPECIFICATION.md` holds the
  text the human chose, in a commit the human lands. The commit names each accepted ADR whose
  basis it moves, or says that the human judged none moved.
- [ ] Given the change landed, when `scripts/check-docs.sh` and `scripts/check-traceability.sh`
  run, then they pass.
- [ ] Given the human rejects the draft, when this ticket closes, then its file is deleted with
  the reason in the commit message.

## Read first

- Touches: `docs/SPECIFICATION.md` (Q-3), nothing else unless the human asks
- `docs/adr/0007-rust-toolchain.md`, `docs/adr/0049-third-party-failures-guarded.md` (their
  *Context*), `scripts/fuzz.sh`, `.github/workflows/fuzz.yml`
- Ticket 02 and ticket 05 of this directory: the input at 10 s, and the weekly run's findings

## Blocked by

- None (can start immediately)

## Mode

with the human. A change to the specification is a decision that only the human lands
([`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules), rule 7).
