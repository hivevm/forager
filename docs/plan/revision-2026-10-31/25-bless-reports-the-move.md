# Ticket: Have `FORAGER_BLESS` report how far each reference image moved

## What to build

The engine's reference images are Forager's own and are rewritten with `FORAGER_BLESS=1`
(`crates/engine/tests/render.rs`, `labels.rs`). Under the blending of #116 and #117, twelve moved by
up to 3 a channel; the pull request said so because its author measured it. The re-blessing of
images after a renderer change was one of the mistakes of this round: an image blessed from the
renderer under test holds nothing of that change. Make the bless path print, per image it rewrites,
the share of pixels and the largest move per channel, so the commit can state it as the convention
on reference images in [`docs/CONVENTIONS.md`](../../CONVENTIONS.md) asks.

## Acceptance criteria

- [ ] Given a change that moves one image, when the tests run with `FORAGER_BLESS=1`, then they
  print that image's name, its share of moved pixels, and its largest move per channel, and nothing
  for the images that did not move.

## Read first

- the bless path in `crates/engine/tests/render.rs` and `labels.rs`; `crates/cli/tests/metric/` for
  the comparison.

## Blocked by

- None (can start immediately)

## Mode

unattended: a test of the report on a moved fixture decides its criteria; its scope is the engine's
test support, and a wrong result costs one revert; it touches none of the parts
[ADR-0007](../../adr/0007-harness.md) keeps with the maintainer and counts against its cap of three;
a wrong result shows as a bless that reports nothing; stop: after two attempts that leave
`scripts/check-all.sh` red, or when the report needs a dependency, the session leaves its state in
this file and hands off.
