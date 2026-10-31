# Ticket: Give the noise measurement a module of its own and one way out of the push checks

## What to build

`scripts/check-rust.sh` runs the ignored tests of `msautotest` with `--skip
mapserver_renders_the_cases_in_scope_within_its_noise`, and `.github/workflows/noise.yml` runs the
same name with `--exact`: renamed, the test runs on every push without `map2img`, and a second
ignored test that needs MapServer joins the push checks unnoticed. Its code (`measure`,
`reproduced`, `noise_report`) sits at the top of `msautotest.rs` beside the G-1 loading, the G-3
round trip, and the conformance render, where `conformance/` and `metric/` are modules. Move it into
a `noise/` module, and keep it out of the push checks by one rule a test holds, such as skipping
itself without `FORAGER_MAP2IMG`, instead of a name written twice
([ADR-0027](../../adr/0027-mapserver-built-from-the-pin.md)).

## Acceptance criteria

- [ ] Given `scripts/check-rust.sh`, when it runs without `FORAGER_MAP2IMG`, then the noise test
  does not run, whatever it is named, and no `--skip` names it.
- [ ] Given the noise workflow, when it runs, then it measures as before.

## Read first

- [ADR-0027](../../adr/0027-mapserver-built-from-the-pin.md); `crates/cli/tests/msautotest.rs`;
  `scripts/check-rust.sh`; `.github/workflows/noise.yml`.

## Blocked by

- None (can start immediately)

## Mode

unattended: `scripts/check-rust.sh` and a run of the noise workflow decides its criteria; its scope
is the noise measurement's code and its two callers, and a wrong result costs one revert; it touches
none of the parts [ADR-0007](../../adr/0007-harness.md) keeps with the maintainer and counts against
its cap of three; a wrong result shows as a red push check or a noise run that measures nothing;
stop: after two attempts that leave `scripts/check-all.sh` red, or when the noise workflow cannot be
run to confirm it, the session leaves its state in this file and hands off.
