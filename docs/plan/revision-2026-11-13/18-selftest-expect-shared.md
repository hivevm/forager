# Ticket: Share the self-tests' expect helper

## What to build

An `expect` helper, which runs a command and compares its exit status and a needle of its output, is
written in 23 `scripts/test-*.sh` files, the new `test-package.sh:46` and
`test-sensor-refactor-tests.sh:47` among them, and a `commit` helper in five.
`scripts/lib/selftest.sh` offers only `check`. Move one `expect` into `scripts/lib/selftest.sh`, and
the `commit` of the fixtures where its five copies agree, and let the self-tests call them. No
self-test changes what it asserts.

## Acceptance criteria

- [ ] Given every self-test at the base, when the helper is shared, then each passes and reports the
  same count of checks as before.
- [ ] Given `rg -l '^expect\(\)' scripts/test-*.sh`, when the change lands, then it finds none.

## Read first

- Touches: `scripts/lib/selftest.sh`, `scripts/test-*.sh`
- `docs/CONVENTIONS.md`, *A self-test asserts what it builds*; `CONTRIBUTING.md`, the layout of
  `scripts/`.

## Blocked by

- docs/plan/revision-2026-11-13/17-sensor-texts-aligned.md

## Mode

unattended: the self-tests' counts decide it; one revert. Stop when a copy differs in what it
asserts, or when the files touched are the template's; hand off naming them.
