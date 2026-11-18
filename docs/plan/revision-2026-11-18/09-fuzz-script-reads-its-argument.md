# Ticket: `scripts/fuzz.sh` refuses an argument that is no number of seconds, and runs one named target

## What to build

`scripts/fuzz.sh` takes one argument, the seconds per target. Ticket 01 of revision-2026-11-16
had a criterion that read `scripts/fuzz.sh render`, written by the revision of 2026-11-16, and
the script hands "render" to libFuzzer's `-max_total_time` after it has installed the toolchain
and run `cargo deny`. Every session that ran one target on one input since, for tickets 16 to 21,
copied the target and called `cargo +nightly-2026-10-20 fuzz run render <input>` by hand.

- The script refuses an argument that is no positive whole number of seconds before it installs
  anything, with its usage.
- It takes an optional target name after the seconds and runs that target alone. A name the
  manifest does not list is refused with the names it lists.

## Acceptance criteria

- [ ] Given `scripts/fuzz.sh render`, when it runs, then it fails before the toolchain is
  installed, and its message names the seconds it expects.
- [ ] Given `scripts/fuzz.sh 30 render`, when the self-test runs it with its stand-in for cargo,
  then only `render` is run, for 30 seconds.
- [ ] Given `scripts/fuzz.sh 30 nothing`, when it runs, then it fails and lists the targets of
  `fuzz/Cargo.toml`.

## Read first

- Touches: `scripts/fuzz.sh`, `scripts/test-fuzz.sh`, `CONTRIBUTING.md` (where it describes the
  script, if it does)
- Q-3; `scripts/lib/expect.sh`, `scripts/lib/selftest.sh`; `docs/CONVENTIONS.md` (a test that
  expects a refusal asserts its reason).

## Blocked by

- None (can start immediately)

## Mode

unattended: `scripts/test-fuzz.sh` decides it, and a wrong result costs one revert. Stop when the
self-test cannot stub cargo for a named target; hand off with what it would need.
