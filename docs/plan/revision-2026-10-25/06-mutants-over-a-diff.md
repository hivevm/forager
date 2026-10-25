# Ticket: Run the mutation sensor over a change before its review

## What to build

The review of #78 found that no test failed without the shapefile reader's shortcut for an extent
outside the file's bounds: every fixture record lay inside them (fixed in af6bbe3). That is the
mistake cargo-mutants exists to report, and `scripts/mutants.sh` runs weekly over the whole
workspace, so it would have reported it a week after the merge. Run over the lines a branch
changed, it reports it before the review. `scripts/mutants.sh` takes a shard today; it also takes
a base revision, and then mutates only the code the diff from that base touches, through
cargo-mutants' option for a diff (`--in-diff` in recent releases; verify it at the pinned
27.1.0). It stays a sensor: it reports and fails only on a broken run.

## Acceptance criteria

- [ ] Given a branch that changes one function whose branch no test decides, when
  `scripts/mutants.sh --base origin/main` runs, then the surviving mutant is listed and the run
  exits 0.
- [ ] Given a branch that changes no Rust code, when it runs with a base, then it reports that
  there is nothing to mutate and exits 0.
- [ ] Given no base, when it runs, then it behaves as today (weekly workflow unchanged).
- [ ] Given the script's header, when it is read, then it says the review runs it with a base; a
  self-test under `scripts/` decides the argument handling without running cargo-mutants.

## Read first

- [ADR-0008](../../adr/0008-rust-toolchain.md) (the slow sensors),
  [ADR-0002](../../adr/0002-decisions-verified-by-tests.md) (checks and sensors as bash scripts).
- `scripts/mutants.sh`, `.github/workflows/mutants.yml`; commit af6bbe3.

## Blocked by

- None (can start immediately)

## Mode

unattended: the self-test under `scripts/` decides the argument handling and the run without a
base, and a run on a scratch branch with a planted untested branch decides the first two
criteria, its output put in the pull request. One script and its self-test, isolated, undone by
one revert; it touches nothing ADR-0007 keeps with the maintainer, and leaves the weekly
workflow and the pinned cargo-mutants as they are. It counts against the cap of three open agent
branches of [ADR-0007](../../adr/0007-harness.md), and is dispatched only while that cap allows.
A wrong outcome shows as the weekly run changing, which the self-test's no-base case catches, or
as a scratch-branch run that lists no surviving mutant. Stop if the pinned cargo-mutants has no
diff option: the version is a supply-chain change and goes to the human.
