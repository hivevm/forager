# Ticket: A check holds a Rust comment to 100 columns

## What to build

rustfmt wraps code at 100 columns, but not comments: its `wrap_comments` and `comment_width`
are unstable, and the pinned toolchain is stable. In the work this revision covers, four reviews
found a comment past 100 columns, and each needed a commit after the review:

- ticket 13 of revision-2026-11-16: a doc comment of 114 columns (6f3435fe);
- ticket 10 of revision-2026-11-18: a doc line of 117 columns (77b80bac);
- ticket 03 of revision-2026-11-18: a line of 133 columns (e609857d);
- ticket 02 of revision-2026-11-18: a comment of 147 columns (9e349fc2).

Check 15 of `scripts/check-docs.sh` holds Markdown prose at 100 columns the same way. A mistake a
check could have caught becomes a check
([`AGENTS.md` §5](../../../AGENTS.md#5-quality-bar--definition-of-done)).

A check refuses a line of a `.rs` file tracked under `crates/` and `fuzz/` that is a comment, `//`,
`///`, or `//!` after indentation, and is longer than 100 characters. Characters are counted, not
bytes, since a comment may hold `×` or `π`. A string literal is not a comment, and a raw string
of a fixture may run long. The check runs where the other checks run: a line in
`scripts/check-rust.sh` or a script of its own with a self-test, listed in `scripts/check-all.sh`
and the CI workflow as `scripts/check-guards.sh` is. The tree holds 21 such lines today, which the
same change wraps.

## Acceptance criteria

- [ ] Given a fixture with a `///` line of 101 characters, when the check runs, then it fails and
  names the file, the line, and its width.
- [ ] Given a fixture with a comment of 100 characters, some of them multi-byte, and a string
  literal of 150 characters, when the check runs, then it passes.
- [ ] Given the tree after the change, when `scripts/check-all.sh` runs, then the check passes.

## Read first

- Touches: `scripts/check-rust.sh` or a new `scripts/check-*.sh` and its `scripts/test-*.sh`,
  `scripts/check-all.sh`, `.github/workflows/checks.yml`, the 21 comment lines (list them with
  the check), `docs/ARCHITECTURE.md` if the check is a script of its own
- `scripts/check-notice.sh` and `scripts/test-check-notice.sh`: some of the 21 lines are the
  provenance lines of a notice header (`crates/engine/src/classes.rs:2`, `crates/expr/src/c.rs:3`,
  `crates/symbolize/src/geometry.rs:9`, `crates/symbolize/src/geometry/topology.rs:7`), which
  `check-notice.sh` parses (its lines 97-111). A header is wrapped only as that check still reads
  it.
- Check 15 of `scripts/check-docs.sh` (`check_prose_width`), the precedent;
  `scripts/test-check-guards.sh`, a self-test of a check of the same size; the convention *A
  self-test asserts what it builds*.

## Blocked by

- None (can start immediately)

## Mode

unattended: its self-test decides it, the wrapping changes only comments, and a wrong result costs
one revert. Stop when telling a comment from a string literal needs more than the line's first
non-blank characters; hand off with the cases that need more.
