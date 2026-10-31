# Ticket: Scan MapServer's expression tokens once in the frontend

## What to build

`crates/mapfile/src/expression.rs` holds two scanners of the same rules of `maplexer.l`: one over
characters for a parenthesised expression (`tokenize`, `bracketed`, `string`), and one over bytes,
new in this round, for a plain `TEXT` (`text_bindings`, `binding_end`, `quoted_binding`,
`string_end`). Both read a binding `\[(?:\]\]|[^\]])*\]`, quoted bindings, and the escapes of a
string; a fix in one misses the other. One scanner that yields tokens, with `text_bindings` a filter
over them.

## Acceptance criteria

- [ ] Given the expression and text tests of `crates/mapfile`, when they run against the one
  scanner, then they pass unchanged.
- [ ] Given a binding with `]]` inside, in an expression and in a plain `TEXT`, when each is read,
  then both give the same binding.

## Read first

- `crates/mapfile/src/expression.rs`; `maplexer.l` at the pin; ticket 28 of the revision of
  2026-10-24 on the lexer.

## Blocked by

- None (can start immediately)

## Mode

unattended: the frontend's tests decides its criteria; its scope is
`crates/mapfile/src/expression.rs`, and a wrong result costs one revert; it touches none of the
parts [ADR-0007](../../adr/0007-harness.md) keeps with the maintainer and counts against its cap of
three; a wrong result shows as a failing frontend test or a G-1 mapfile that no longer loads; stop:
after two attempts that leave `scripts/check-all.sh` red, or when ticket 28 of the revision of
2026-10-24 has started on the same file, the session leaves its state in this file and hands off.
