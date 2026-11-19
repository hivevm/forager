# Ticket: The weekly mutants run makes its output directory, so it runs at all

## What to build

The scheduled Mutants workflow has failed every week since it was added. The runs of 2026-10-25,
11-01, 11-08, and 11-15 on `main` each stopped after 4 to 5 minutes, before the first mutant:

```text
Installed package `cargo-mutants v27.1.0` (executable `cargo-mutants`)
Error: create output parent directory "target/mutants"
Caused by:
    No such file or directory (os error 2)
mutants: the run failed (exit 1)
```

`scripts/mutants.sh` passes `--output target/mutants` (line 51). It creates `target/` only on the
path of a diff (`mkdir -p target`, line 63). The workflow caches `~/.cargo/bin` and
`~/.cargo/registry`, never `target/`, so the fresh checkout of every scheduled run has no parent
directory, and cargo-mutants 27.1.0 does not create it. No sensor of
ADR-0007's mutation testing has reported for four weeks.

Create the output's parent before cargo-mutants runs, on every path of the script. The script's
self-test, `scripts/test-mutants.sh`, gains a case that runs without `target/`.

## Acceptance criteria

- [ ] Given a copy of the repository without `target/`, when `scripts/test-mutants.sh` runs its
  new case, which stubs `cargo mutants` as the self-test's other cases do, then the stub sees an
  existing parent of its `--output`, and the case fails on the script before the fix.
- [ ] Given the tree after the change, when `scripts/check-all.sh` runs, then it passes.
- [ ] Given the change merged, when the human dispatches `.github/workflows/mutants.yml` on
  `main`, then the run passes its first mutant. Dispatching is the human's (`gh`,
  [`AGENTS.md` §6](../../../AGENTS.md#6-project-rules)), and the hand-off asks for it.

## Read first

- Touches: `scripts/mutants.sh`, `scripts/test-mutants.sh`
- `.github/workflows/mutants.yml` (its caches), ADR-0007's paragraph on mutation testing
- `docs/CONVENTIONS.md`: *A test that expects a refusal asserts its reason*

## Blocked by

- None (can start immediately)

## Mode

unattended. The self-test decides the fix, and a wrong result costs one revert. The run in CI
is the human's to dispatch. Stop if the stand-in `cargo` cannot see the directory without a
change to how the self-test stubs it. Hand off with the case as it stands.
