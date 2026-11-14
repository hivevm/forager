# Ticket: Share the self-tests' expect helper

## What to build

An `expect` helper, which runs a command and compares its exit status and a needle of its output, is
written in 23 `scripts/test-*.sh` files, and a `commit` helper in five. `scripts/lib/selftest.sh`
and the self-tests the NUC template brought (62f7935, the take-ups 2c1c3ab and 43791c5) are the
template's: a change to them is lost at the next take-up (ADR-0004). Move one `expect`, and the
`commit` of the fixtures where its copies agree, into a project file, `scripts/lib/expect.sh`, and
let the project's own self-tests call it: those no template commit added, such as `test-package.sh`,
`test-sensor-refactor-tests.sh`, `test-sensor-backlog-commits.sh`, `test-check-notice.sh`,
`test-check-vendor.sh`, `test-check-fidelity.sh`, `test-fuzz.sh`, `test-mutants.sh`,
`test-generate-example.sh`, `test-build-mapserver.sh`, and `test-fixture-git-env.sh` (check each by
`git log --diff-filter=A`). Leave the template's copies, and record in the backlog, in a commit of
its own, that the template's self-tests share no `expect`, for NUC. No self-test changes what it
asserts.

## Acceptance criteria

- [ ] Given every self-test at the base, when the helper is shared, then each passes and reports the
  same count of checks as before.
- [ ] Given `rg -l '^expect\(\)'` over the project's own self-tests, when the change lands, then it
  finds none; no file the template added is changed.

## Read first

- Touches: `scripts/lib/expect.sh`, the project's own `scripts/test-*.sh`
- `docs/CONVENTIONS.md`, *A self-test asserts what it builds*; `CONTRIBUTING.md`, the layout of
  `scripts/`.

## Blocked by

- docs/plan/revision-2026-11-13/17-sensor-texts-aligned.md

## Mode

unattended: the self-tests' counts decide it; one revert. Stop when a copy differs in what it
asserts.
