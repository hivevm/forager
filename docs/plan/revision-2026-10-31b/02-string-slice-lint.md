# Ticket: Refuse slicing a `str` at an index clippy cannot see is a character's boundary

## What to build

Two panics of one class reached `main`: `text_bindings` of the mapfile frontend sliced
`text[i..i + 8]` after an `f` and panicked on a multi-byte character that followed (fixed in #140,
found only by a differential test), and the browser's credential filter sliced one byte past a
character it found by `rfind` (fixed in 82a1000, ticket 01 of this revision). Fuzzing caught
neither: the mapfile target had no seed with a label's `TEXT`, and no target reaches the bindings.
A check decides the class at build time: clippy's restriction lint `clippy::string_slice` warns on
every index or range into a `str` or `String`. Run it against the workspace's libraries and
binaries to list the sites:
`cargo clippy --workspace --lib --bins -- -A clippy::all -W clippy::string_slice`. The counts per
crate this ticket first gave, 37 sites in six crates, predate the move of the symbolization into
`crates/symbolize` and the fix of the credential filter, so they are dropped; the session counts
again. When they were counted, every site but the credential filter's stood on an index that
`find` or `match_indices` gave for an ASCII byte, or that stepped by `len_utf8`, and could not
panic.

Turn the lint on in `[workspace.lints.clippy]` of `Cargo.toml`, so that `scripts/check-rust.sh`
fails on a new slice; rewrite each site it raises to what states its boundary (`split_once`,
`strip_prefix`, `get(..)`, `char_indices`) where that reads as plainly, and give each other one an
`#[expect(clippy::string_slice, reason = "…")]` that names the ASCII byte it stands on. Test code
may allow it. No behaviour changes.

## Acceptance criteria

- [ ] Given a function under `crates/*/src` that slices a `&str` by a computed index, when
  `scripts/check-rust.sh` runs, then clippy fails on it.
- [ ] Given the workspace after the change, when `scripts/check-all.sh` runs, then it is green and
  no test or reference image changed.
- [ ] Given each `#[expect(clippy::string_slice)]` left, when it is read, then its reason names why
  the index stands on a character's boundary.

## Read first

- [ADR-0007](../../adr/0007-rust-toolchain.md) (the toolchain and its lints), `Cargo.toml`'s
  workspace lints and `clippy.toml`; `cargo clippy --explain string_slice`; the sites the command
  above lists; Q-3 in `docs/SPECIFICATION.md`.

## Blocked by

- None (can start immediately)

## Mode

with the human: the sites it rewrites include the credential filter of the browser bindings and the
reading of resource paths, which [ADR-0006](../../adr/0006-harness.md) never leaves unattended.
