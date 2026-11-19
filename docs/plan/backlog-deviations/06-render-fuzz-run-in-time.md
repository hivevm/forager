# Ticket: The render fuzz target's run holds Q-3's 10 s on two layers' nested smoothsia

<!-- Copy to docs/plan/<spec>/<NN>-<slug>.md, next to the spec it slices, if it has one. A
     vertical slice: a narrow but complete path through every layer the change touches, demoable
     or verifiable on its own, sized to one fresh agent session. Claim it by creating the branch
     ticket/<spec>/<NN>-<slug> BEFORE any work, so parallel sessions sharing the repository skip
     it. The branch that lands it deletes this file. Decided in
     docs/adr/0005-feature-layer.md. -->

## What to build

Ticket 03's first 300-second run of the render target timed out on an input of two layers that
share a nested `smoothsia` tree, drawn on a polygon and on a multipoint: 16.7 s alone on the fuzz
build, about 0.3 s in a release build. The backlog entry
`docs/backlog/smoothsia-budget-outlasts-the-fuzz-timeout.md` holds the input, what it decodes to,
the timings, and the options. The maintainer chose on 2026-11-19 to fix it in this plan, and moved
ticket 03's last criterion, a clean fuzz run, here.

The session measures first: the visits the input charges, per layer and per call, and what one
visit costs in `smooth_part`, whose window walk does more per point than the charge counts. If
the charge undercounts the work, the fix charges what the walk does, within the bound as it
stands. A change of a limit itself, of `MAX_TRANSFORM_VISITS` or of a bound per layer into one per
render, is the maintainer's (ADR-0006); the session brings the measurement and the options. The
change deletes the backlog entry.

## Acceptance criteria

- [ ] Given the input of the backlog entry, when it replays on the fuzz build, then it ends within
  10 s.
- [ ] Given the measurement, when the hand-off is written, then it names the visits the input
  charges per layer and per call, and the cost of a visit against what the charge counts.
- [ ] Given a nested `smoothsia` tree that spends the visit budget, when a unit test draws it,
  then the layer stops at the bound with its warning, as before.
- [ ] Given the render target, when it is fuzzed for the time the fuzz seed convention names,
  then it finds no panic and no timeout, or each one found is fixed or filed.

## Read first

- Touches: `crates/symbolize/src/layer_transform.rs` (`smooth_sia`, `smooth_part`,
  `preprocess`, `Budget`, `MAX_TRANSFORM_VISITS`), its tests
- `docs/backlog/smoothsia-budget-outlasts-the-fuzz-timeout.md`; Q-3 and Q-9 of
  `docs/SPECIFICATION.md`; `scripts/fuzz.sh` for the limits.
- ADR-0006, which keeps the resource limits with the human.

## Blocked by

- docs/plan/backlog-deviations/03-render-fuzz-builds-every-transform.md

## Mode

with the human: it touches the charge against a resource limit, which ADR-0006 keeps with the
human. The session may change how the work is charged; it stops before it changes a limit or the
scope of a bound, and hands off with the measurement and the options.
