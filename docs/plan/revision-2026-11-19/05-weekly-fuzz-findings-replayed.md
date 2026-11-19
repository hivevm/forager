# Ticket: The weekly fuzz run's four findings are replayed on main, and each that still fails is filed

## What to build

The scheduled Fuzz workflow on `main` has failed four weeks in a row, always on the render
target. Nobody looked at the runs. Each kept its input as the artifact `fuzz-artifacts`, which
GitHub holds until January or February 2027:

| Run | Date | Finding | Input kept |
| --- | --- | --- | --- |
| 40177650370 | 2026-10-25 | `timeout after 12 seconds` (and a slow unit) | `render/timeout-e5bbc9a5…`, `render/slow-unit-52255f14…` |
| 41298924359 | 2026-11-01 | panic at `crates/label/src/layout.rs:226:21` | `render/crash-77ac77c5…` |
| 42472632667 | 2026-11-08 | panic at `crates/label/src/layout.rs:227:21` | `render/crash-fea099f7…` |
| 43708928938 | 2026-11-15 | `timeout after 10 seconds` | `render/timeout-101b651b…` |

Q-3 asks that each failure a fuzz run finds be held by a test of its input. Fixes since then may
have closed some of them. Ticket 06 of backlog-deviations sped up `smoothsia`, and the label
code has moved since 11-08. Nothing in the tree says which.

Replay each input against `main`, on the fuzz build and in a release build, and decode it with
`fuzz/fuzz_targets/render_input.rs`. The decoding has changed since the oldest runs, so an input
may decode to another map today. Read each failure on the code of its own run's commit
(`gh run view <id> --json headSha`). For each finding:

- **still fails on main:** a backlog entry with its decoded input, the cause read from the code,
  and the input in base64, as `docs/backlog/` entries of fuzz findings hold it. A fix is a ticket
  of its own, which the human chooses.
- **passes on main:** name the commit that fixed it, found by `git bisect` on the replay. If no
  test of the tree holds its input, add the input as a seed of the render target with its path
  in `fuzz/tests/render_seeds.rs`, re-derived if the decoding has moved
  (`docs/CONVENTIONS.md`, the fuzz seeds).

## Acceptance criteria

- [ ] Given the four artifacts, when the ticket closes, then its hand-off has a table with one
  row per input. Each row gives the result on `main`, on the fuzz build with its time and on a
  release build, and the commit that fixed the input or the backlog entry that holds it.
- [ ] Given an input that panics on `main`, when the ticket closes, then a backlog entry holds
  it with its decoded map and the panicking line, and no fix is in this ticket's change.
- [ ] Given an input that passes on `main` and that no test holds, when
  `fuzz/tests/render_seeds.rs` runs, then a seed of it builds the path it was found on.
- [ ] Given the tree after the change, when `scripts/check-all.sh` runs, then it passes.

## Read first

- Touches: `fuzz/seeds/render/`, `fuzz/tests/render_seeds.rs`, `docs/backlog/`
- `scripts/fuzz.sh` (`--replay`), `.github/workflows/fuzz.yml` (the step "Keep the inputs that
  failed"), `fuzz/fuzz_targets/render_input.rs`
- `docs/CONVENTIONS.md`: *A change that gives a target of Q-3 a new reader or a new path brings
  the fuzz seed that reaches it*; *A fix of a fuzz finding is cut from the cause its decoded input
  shows*
- Ticket 02 of this directory: one of the timeouts may be its input, or a sibling of it

## Blocked by

- None (can start immediately)

## Mode

with the human. The artifacts come down through `gh run download`, which waits for the human's
instruction ([`AGENTS.md` §6](../../../AGENTS.md#6-project-rules)). The human may also download
them into `.scratch/` beforehand. After that, the session works the ticket unattended. Stop
after the four inputs, or if an input reproduces neither on the commit of its run nor on `main`.
Hand off with the table.
