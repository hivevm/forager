# Ticket: Hold the direction in which `symbolize`'s modules import one another by a test

## What to build

`crates/symbolize/src/lib.rs` states that the crate's modules import one another in one direction
only: `draw` imports every other; `hatch` imports `bounds`, `extent`, `style`, and `symbol`;
`line_marker` and `tile` import `bounds` and `symbol`; `symbol` imports `bounds` and `style`;
`geom_transform` imports `extent`; `bounds`, `extent`, `offset`, and `style` import none.
[`docs/ARCHITECTURE.md`](../../ARCHITECTURE.md) repeats it and says that no check holds it, since
[ADR-0009](../../adr/0009-ports-and-adapters-crates.md) decides the direction only between crates
and `crates/arch-test` tests only that.

The pixmap work broke it: the `Pixmaps` alias was defined in `draw.rs` and imported by
`symbol.rs`, below it, until a fresh-session review of the branch found it and 5ce3686 moved the
alias to `symbol`. Nothing but a reader saw it, and the planned split of `draw.rs` into modules by
concern (parked on `parked/tickets-2026-11-07` as `revision-2026-11-07/09`) will move most of the
crate's imports at once.

Add a test to `crates/arch-test`, beside `c_numbers.rs`, that reads each source file of
`crates/symbolize/src/`, collects the modules of the crate it names by a `crate::<module>` path,
and fails, naming the file and the module, where one is not among those the table allows that
module. The table lives in the test, in the order of `lib.rs`'s sentence, and `lib.rs` and the
overview cite the test instead of the sentence standing alone. Code under `#[cfg(test)]`, a
module's `tests.rs` and an inline `mod tests`, is outside the check: `hatch/tests.rs` calls
`draw::clip_pixmaps` to test the clip buffer of a class, which is a test reaching up, not a module
depending on one above it. A doc comment is outside it too. A module the table does not know
fails the test, so that a module added without its row is noticed.

## Acceptance criteria

- [ ] Given the crate as it stands, when `cargo test -p forager-arch-test` runs, then the new test
  passes.
- [ ] Given `use crate::draw::Canvas;` added to `crates/symbolize/src/symbol.rs` (outside a test),
  when the test runs, then it fails naming `symbol.rs` and `draw`; given a new file
  `crates/symbolize/src/foo.rs` declared in `lib.rs` without a row, then it fails naming `foo`;
  both shown red in the session and stated in the pull request.
- [ ] Given `hatch/tests.rs`'s import of `crate::draw`, when the test runs, then it is not
  reported.
- [ ] Given `docs/ARCHITECTURE.md`'s `symbolize` bullet and `lib.rs`'s module comment, when they
  are read, then they name the test that holds the direction, and the overview no longer says that
  no check holds it.

## Read first

- [ADR-0009](../../adr/0009-ports-and-adapters-crates.md); `crates/symbolize/src/lib.rs`;
  `crates/arch-test/tests/c_numbers.rs`, whose scan of the sources is the pattern; the `symbolize`
  bullet of [`docs/ARCHITECTURE.md`](../../ARCHITECTURE.md).

## Blocked by

- None (can start immediately)

## Mode

unattended: the test and its two mutations shown red decide it; the scope is one test file of
`crates/arch-test` and two comments, and a wrong result costs one revert. It changes no behaviour
of the system and none of the parts [ADR-0007](../../adr/0007-harness.md) keeps with the
maintainer, and counts against its cap of three. Stop: when the crate as it stands fails the test
outside code under `#[cfg(test)]`, the session records the import here and hands off rather than
move code, since the move is a refactor parked until after the normalization.
