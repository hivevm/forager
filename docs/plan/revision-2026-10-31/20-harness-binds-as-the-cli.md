# Ticket: Bind the harness's sources with the CLI's own code, fonts included

## What to build

`render` in `crates/cli/tests/msautotest.rs` copies the CLI's binding of shapefiles (`held_sources`,
`Shapefiles`, `open_shapefile` in `crates/cli/src/main.rs`) because the binding lives in the binary,
and lists four ways the copy departs. A fifth it does not list: it renders with
`forager_engine::render`, whose loader loads no file, where the CLI renders with `render_with_files`
and a font loader, so once a label case comes into scope the harness measures what `forager render`
would not draw. The inline-feature mistake of #119, the CLI reading `DATA` for an inline layer, had
to be fixed in both. Give `forager-cli` a library target the test calls, holding the binding, and
render through it with the suite's loader.

## Acceptance criteria

- [ ] Given a case with a `FONTSET` and a label, when the harness renders it, then the fonts are
  loaded as `forager render` loads them.
- [ ] Given `crates/cli/tests/msautotest.rs`, when it is read, then it binds through the CLI's
  library and its comment names no departure but the suite's loader and table.
- [ ] Given `scripts/check-fidelity.sh`, when it runs, then the share of passing cases does not
  fall.

## Read first

- `crates/cli/src/main.rs`; `render` in `crates/cli/tests/msautotest.rs`;
  [ADR-0009](../../adr/0009-ports-and-adapters-crates.md) (the hosts' edges);
  `docs/plan/labels/07-point-label-cases-in-scope.md`, which brings label cases into scope and is
  worked by another session.

## Blocked by

- None (can start immediately)

## Mode

unattended: the harness's own tests and `scripts/check-fidelity.sh` decides its criteria; its scope
is `crates/cli`, and a wrong result costs one revert; it touches none of the parts
[ADR-0007](../../adr/0007-harness.md) keeps with the maintainer and counts against its cap of three;
a wrong result shows as a falling fidelity share or a failing CLI test; stop: after two attempts
that leave `scripts/check-all.sh` red, or when the change needs the CLI's binding to change
behaviour, not only place, the session leaves its state in this file and hands off.
