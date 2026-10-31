# Ticket: Decide in the frontend alone which layers hold their features inline

## What to build

The frontend marks a layer inline when it has any `FEATURE` (`translate/mod.rs`); the engine's
`is_inline` in `crates/engine/src/inline.rs` checks that its features are not empty and that its
source's kind is not `graticule`, a MapServer name in the core, though the overview names the
frontend as the only crate that knows MapServer's syntax. Let the frontend, which reads
`CONNECTIONTYPE`, leave out with a warning the features of a connection type MapServer exempts, and
the engine test only for features ([ADR-0033](../../adr/0033-inline-features-in-the-style-ir.md)).

## Acceptance criteria

- [ ] Given a `GRATICULE` layer with a `FEATURE`, when it is loaded, then the frontend warns and the
  layer holds no features, and the engine draws it as before.
- [ ] Given `crates/engine/src`, when it is searched, then it names no connection type.

## Read first

- [ADR-0033](../../adr/0033-inline-features-in-the-style-ir.md); `is_inline` in
  `crates/engine/src/inline.rs`; `crates/mapfile/src/translate/mod.rs` and `feature.rs`.

## Blocked by

- None (can start immediately)

## Mode

unattended: a frontend test and the engine's inline tests decides its criteria; its scope is the
inline features of the frontend and the engine, and a wrong result costs one revert; it touches none
of the parts [ADR-0007](../../adr/0007-harness.md) keeps with the maintainer and counts against its
cap of three; a wrong result shows as a failing inline test; stop: after two attempts that leave
`scripts/check-all.sh` red, or when the change needs a field of the style document, the session
leaves its state in this file and hands off.
