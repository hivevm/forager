# Ticket: Give the bounds module the style's bound defaults and every site that bounds by them

## What to build

Since #139, `crates/engine/src/bounds.rs` holds MapServer's constants and one function per site that
bounds a style's width or size, and keeps the defaults of a style's bounds private
(`min_width`, `max_width`, `min_size`, `max_size`, each `style.….unwrap_or(CONST)`). Four sites
outside it write the same defaults again: `outline_style` and `point_ops` in `draw.rs`,
`marker_size` in `symbol.rs`, and `Hatching::for_class` in `hatch.rs`. The module's header names two
sites that bound a size outside it, `symbol::marker_size` and `hatch::Hatching::for_class`; the
stand-in square of `point_ops`, which clamps its size to `(0, MAX_SIZE)` and ignores the style's
`MINSIZE` and `MAXSIZE`, is a third it does not name, and its header carries a comment line past 100
columns. Lend the defaults from `bounds.rs`, move the sites that bound by MapServer's functions into
it, and name in its header each one that stays outside with its reason.

## Acceptance criteria

- [ ] Given `crates/engine/src`, when it is searched, then no `unwrap_or` of a bound constant
  stands outside `bounds.rs`.
- [ ] Given the engine's tests and the harness, when they run, then they pass unchanged and no
  reference image is re-blessed.
- [ ] Given `bounds.rs`'s header, when it is read, then it names every site that bounds a width or
  a size outside it, the stand-in square included.

## Read first

- `crates/engine/src/bounds.rs` and its tests, `draw.rs`, `symbol.rs`, `hatch.rs`; #139's doubts
  (`gh pr view 139`); the entry on ported behaviour in `docs/CONVENTIONS.md`.

## Blocked by

- None (can start immediately)

## Mode

unattended: the engine's tests and the harness decide its criteria; an interface-preserving
refactoring inside the engine, undone by one revert; it touches none of the parts
[ADR-0007](../../adr/0007-harness.md) keeps with the maintainer and counts against its cap of
three; a wrong result shows as a moved test or image; stop: when a site cannot move without a
change of what it draws, the session leaves it named in this file and hands off. Ticket
`docs/plan/revision-2026-10-31/01-symbolize-crate.md` moves the module later; this one does not wait
for it.
