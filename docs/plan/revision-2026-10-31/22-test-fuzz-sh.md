# Ticket: Give `scripts/fuzz.sh` the self-test its siblings have

## What to build

`scripts/fuzz.sh` reads its targets from `cargo metadata` and fails a target without seeds, logic
that its siblings of this round, `mutants.sh`, `build-mapserver.sh`, and `check-fidelity.sh`, each
hold with a self-test run by `scripts/check-all.sh`; it has none. Its header also lists the targets
and has drifted: it leaves out `font`. Add `scripts/test-fuzz.sh` over fixture manifests (no bins, a
bin without seeds, a bin declared with a path), run it from `check-all.sh` and the workflow that
runs the other self-tests, and drop the list from the header
([ADR-0008](../../adr/0008-rust-toolchain.md)).

## Acceptance criteria

- [ ] Given a fixture manifest with a bin and no seeds for it, when `scripts/test-fuzz.sh` runs
  `fuzz.sh` against it, then the run fails naming the target, before anything is fuzzed.
- [ ] Given `scripts/check-all.sh`, when it runs, then it runs the self-test.

## Read first

- `scripts/fuzz.sh`; `scripts/test-mutants.sh` as the pattern; `scripts/check-all.sh`;
  `.github/workflows/checks.yml`.

## Blocked by

- None (can start immediately)

## Mode

unattended: the self-test itself decides its criteria; its scope is `scripts/`, and a wrong result
costs one revert; it touches none of the parts [ADR-0007](../../adr/0007-harness.md) keeps with the
maintainer and counts against its cap of three; a wrong result shows as a red `check-all.sh` or a
red weekly fuzz run; stop: after two attempts that leave `scripts/check-all.sh` red, or when
`fuzz.sh` needs to change beyond reading a root it is passed, the session leaves its state in this
file and hands off.
