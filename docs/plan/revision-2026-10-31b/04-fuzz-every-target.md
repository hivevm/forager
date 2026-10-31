# Ticket: Fuzz every target though one finds something, and self-test it with `fuzz.sh`'s own pins

## What to build

`scripts/fuzz.sh` runs under `set -e` and stops at the first target that fails. The weekly run of
2026-10-25 (run 40177650370) timed out in the `render` target, its third, and so never ran `expr`,
`expr_input`, `font`, `geojson`, or `shapefile` that week; the timeout was fixed in 5e8fcf8, but a
finding in one target hides the others for a week each time. Q-3 is measured by "a weekly scheduled
fuzz run of 5 minutes per target … that ends without a panic". Run every target for its time, keep
the artifacts of each that fails, and fail the run at the end, naming each target that failed.

Its self-test `scripts/test-fuzz.sh` writes `fuzz.sh`'s pins as literals: `nightly-2026-10-20` and
`cargo-fuzz 0.13.2`, five times, so a bump of a pin in `fuzz.sh` turns the self-test red until it is
edited too. It reads them from `fuzz.sh` instead, as `scripts/test-build-mapserver.sh` reads its
pin from the script it tests.

## Acceptance criteria

- [ ] Given a fixture with two targets whose first fails under the stand-in cargo, when
  `scripts/fuzz.sh` runs, then the second runs too, and the script exits non-zero naming the first.
- [ ] Given every target passing, when it runs, then it exits 0 as before.
- [ ] Given `NIGHTLY` or `CARGO_FUZZ_VERSION` changed in `scripts/fuzz.sh`, when
  `scripts/test-fuzz.sh` runs, then it passes without an edit.
- [ ] Given `.github/workflows/fuzz.yml`, when a target fails, then the artifacts of every failing
  target are uploaded.

## Read first

- `scripts/fuzz.sh`, `scripts/test-fuzz.sh`, `scripts/test-build-mapserver.sh` (how it reads its
  pin), `.github/workflows/fuzz.yml`; Q-3 in `docs/SPECIFICATION.md`;
  [ADR-0008](../../adr/0008-rust-toolchain.md); the entry on refusals asserting their reason in
  `docs/CONVENTIONS.md`.

## Blocked by

- None (can start immediately)

## Mode

unattended: `scripts/test-fuzz.sh` decides its criteria; its scope is `fuzz.sh`, its self-test, and
the workflow's upload step, and a wrong result costs one revert; it touches none of the parts
[ADR-0007](../../adr/0007-harness.md) keeps with the maintainer and counts against its cap of
three; a wrong result shows as a red self-test or a weekly run that reports no target; stop: after
two attempts that leave `scripts/check-all.sh` red, the session leaves its state in this file and
hands off.
