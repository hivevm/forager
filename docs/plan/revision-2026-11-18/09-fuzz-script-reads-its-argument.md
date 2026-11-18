# Ticket: `scripts/fuzz.sh` refuses an argument it cannot read, runs one named target, and replays one input

## What to build

`scripts/fuzz.sh` takes one argument, the seconds per target (`README.md`, the *Fuzz* line). Two
things went wrong around it in revision-2026-11-16:

- Ticket 01 had a criterion that read `scripts/fuzz.sh render`, written by the revision of
  2026-11-16 (17823a65). The script hands "render" to libFuzzer's `-max_total_time`, after it has
  installed the toolchain and run `cargo deny`.
- Every session that checked a fix of tickets 16 to 21 against its crash input called
  `cargo +nightly-2026-10-20 fuzz run render <input>` by hand, the nightly pin copied out of the
  script.

The script serves both:

- it refuses an argument that is no positive whole number of seconds before it installs
  anything, with its usage;
- it takes an optional target name after the seconds and fuzzes that target alone. A name the
  manifest does not list is refused with the names it lists;
- `scripts/fuzz.sh --replay <target> <input>` runs the target once on the input file, on the
  script's pinned toolchain, and exits with the target's result. That is the run a fix's session
  needs, without copying the pin. It installs the pinned nightly and cargo-fuzz where they are
  missing, as a campaign does, and skips `cargo deny`, which guards the weekly run's supply chain
  and not one input's replay. It refuses a target the manifest does not list, and an input file
  that does not exist, before it installs anything.

`README.md`'s *Fuzz* line gives the new usage.

## Acceptance criteria

- [ ] Given `scripts/fuzz.sh render`, when it runs, then it fails before the toolchain is
  installed, and its message names the seconds it expects.
- [ ] Given `scripts/fuzz.sh 30 render`, when the self-test runs it with its stand-in for cargo,
  then only `render` is fuzzed, for 30 seconds.
- [ ] Given `scripts/fuzz.sh 30 nothing`, when it runs, then it fails and lists the targets of
  `fuzz/Cargo.toml`.
- [ ] Given `scripts/fuzz.sh --replay render <file>`, when the self-test runs it, then the
  stand-in records one `fuzz run render <file>` on the pinned nightly, no `-max_total_time`, and
  no `cargo deny`, and the script exits with the stand-in's status.
- [ ] Given `scripts/fuzz.sh --replay render <missing file>`, when it runs, then it fails before
  the toolchain is installed and names the file.

## Read first

- Touches: `scripts/fuzz.sh`, `scripts/test-fuzz.sh`, `README.md` (the *Fuzz* line of **Build,
  Test & Run**)
- Q-3; `scripts/lib/expect.sh`, `scripts/lib/selftest.sh`; `docs/CONVENTIONS.md` (a test that
  expects a refusal asserts its reason).

## Blocked by

- None (can start immediately)

## Mode

unattended: `scripts/test-fuzz.sh` decides it, and a wrong result costs one revert. Stop when the
self-test cannot stand in for cargo on a named target or a replay; hand off with what it would
need.
