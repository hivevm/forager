# Ticket: Align the sensors' texts and the scripts' documentation with what they do

## What to build

Four small drifts in what the scripts say:

- `scripts/sensor-tests-kept.sh:58` (`is_test_path`) reads every path with a segment holding
  `test` or `spec` as a test, so it listed the deleted ticket
  `docs/plan/revision-2026-11-11/10-adr-0033-noise-test-path.md` (82a2435). Skip `docs/`.
- `scripts/sensor-refactor-tests.sh:73` says each commit "is a fix with its regression test", a
  clause the convention *A commit typed `refactor`* dropped in 6109830. Say what the convention
  says.
- `CONTRIBUTING.md:67` says `scripts/lib/` holds "the ADR header, git, the ticket path, the
  self-tests"; it also holds `check.sh`, `notice.sh`, `package.sh`, and `rules.sh`.
- `scripts/test-sensor-refactor-tests.sh:21` unsets git's repository variables, which
  `scripts/lib/selftest.sh:20` unsets again after it sources it.

## Acceptance criteria

- [ ] Given a range that deletes `docs/plan/x/01-test-path.md`, when `sensor-tests-kept.sh` runs,
  then it lists nothing, by its self-test.
- [ ] Given the refactor sensor's self-test, when it runs, then it asserts the message without
  "regression test".
- [ ] Given `scripts/check-docs.sh`, when it runs, then it passes over `CONTRIBUTING.md`'s line
  naming the files of `scripts/lib/`, each of which exists.
- [ ] Given `rg -c 'local-env-vars' scripts/test-sensor-refactor-tests.sh`, when the change lands,
  then it counts 0.

## Read first

- Touches: `scripts/sensor-tests-kept.sh`, `scripts/test-sensor-tests-kept.sh`,
  `scripts/sensor-refactor-tests.sh`, `scripts/test-sensor-refactor-tests.sh`, `CONTRIBUTING.md`
- `docs/CONVENTIONS.md`, *A rule changes everywhere it is stated*.

## Blocked by

- None (can start immediately)

## Mode

unattended: the self-tests decide it; one revert. Stop when `sensor-tests-kept.sh` is the
template's and a change to it would be lost at the next take-up; hand off naming the template.
