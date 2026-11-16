# Ticket: msPointInPolygon and msDistancePointToPoint are ported once, in `style_ir::c`

## What to build

ADR-0017 decides that C's and MapServer's small functions are ported once, in the module `c` of
`crates/style-ir`, and called there from every crate. Two of them are ported twice:

- `msPointInPolygon`, W. Randolph Franklin's pnpoly, in `crates/label/src/cache.rs` and in
  `crates/symbolize/src/layer_transform.rs` (3652dae0), each with its own copy of Franklin's
  notice;
- `msDistancePointToPoint`, as `fn distance` in `layer_transform.rs` and in `line_marker.rs`.

A third copy of pnpoly is a test's (`crates/label/tests/cache_pairwise.rs`); it calls the port
once the port is public.

Both move into `style_ir::c`, generic over the point type or taking coordinates, with the notice
once. `label` and `symbolize` call them. ADR-0017's list of helpers names them. That is a change
of an accepted ADR's text, which the maintainer accepts (ADR-0006). The notice check and
`THIRD-PARTY-NOTICES.md` follow the move.

## Acceptance criteria

- [ ] Given the tree, when `crates/*/src` and `crates/*/tests` are searched for a definition of
  pnpoly or of msDistancePointToPoint, then each is found once, in `crates/style-ir/src/c.rs`,
  and `layer_transform.rs` and `line_marker.rs` define no `fn distance`.
- [ ] Given `scripts/check-notice.sh`, when it runs, then it passes with Franklin's notice in one
  file.
- [ ] Given `cargo test --workspace` and the msautotest harness, when they run, then all pass
  unchanged.

## Read first

- Touches: `crates/style-ir/src/c.rs`, `crates/label/src/cache.rs`,
  `crates/symbolize/src/{layer_transform,line_marker}.rs`, `THIRD-PARTY-NOTICES.md`,
  `docs/adr/0017-*.md`
- ADR-0017, ADR-0035 (vendored and ported notices); `scripts/check-notice.sh`.

## Blocked by

- None (can start immediately)

## Mode

with the human: the change amends the text of ADR-0017, which the maintainer accepts.
