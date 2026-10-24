# Ticket: A check that every file following MapServer's code carries its notice

## What to build

The specification's licence constraint asks that a file whose logic is ported from MapServer's C
code keeps MapServer's copyright notice, and `docs/CONVENTIONS.md` says how. Two files of the
mapfile frontend follow MapServer's code, name its sources, and carry no notice:
`crates/mapfile/src/translate.rs` (it cites `src/mapserver.h`, `loadStyle`, `loadColor`,
`loadSymbol`, `msLoadFontSet`, `msGetSymbolIndex`, `msShapeGetLabelAnnotation`, and more) and
`crates/mapfile/src/expression.rs` (it follows `src/maplexer.l`, `src/mapparser.y`, and
`msEvalExpression`). `lexer.rs`, `syntax.rs`, and `crates/engine/src/extent.rs` carry it. The
convention prevented nothing here; a check decides it.

A check script under `scripts/`, run by `scripts/check-all.sh` and CI, fails on a Rust file under
`crates/` or `fuzz/` that names a MapServer source file (`src/map….c`, `.h`, `.l`, `.y`) and
does not hold MapServer's copyright and permission notice, naming the file. A self-test feeds it
a file of each kind. The two files get the notice, and the convention entry *Logic ported from
MapServer keeps MapServer's notice* is dropped from `docs/CONVENTIONS.md` in the same change, as
a convention a check decides is the check.

## Acceptance criteria

- [ ] Given a Rust file that cites `src/mapfile.c` without the notice, when the check runs, then
  it fails and names the file.
- [ ] Given the tree with the notice added to `translate.rs` and `expression.rs`, when
  `scripts/check-all.sh` runs, then the check passes.
- [ ] Given a file that cites only a MapServer function name and no source file, when the check
  runs, then it passes; the self-test pins that limit, so that the check's reach is stated.

## Read first

- [ADR-0002](../../adr/0002-decisions-verified-by-tests.md) (checks are bash scripts under
  `scripts/`); the licence constraint and Q-6 of `docs/SPECIFICATION.md`.
- The notice in `crates/mapfile/src/lexer.rs`; a self-test such as
  `scripts/test-check-shell.sh` for the pattern.

## Blocked by

- None (can start immediately)

## Mode

with the human: whether a file that follows PROJ's code, as `crates/mapfile/src/init_file.rs`
follows `get_init_string`, needs PROJ's notice as well is a licence judgement the specification
does not make; the session asks before it extends the check to PROJ.
