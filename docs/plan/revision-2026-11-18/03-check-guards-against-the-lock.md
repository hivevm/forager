# Ticket: A check that every guard names the version of its crate that `Cargo.lock` pins

## What to build

The guards before agg-rust, geo, and i_overlay are derived from the versions `Cargo.lock` pins:
agg-rust 1.2.0, geo 0.33.1, i_overlay 4.5.2 under geo. Nothing ties them to those versions. An
update of the lock file can leave a guard that no longer bounds what it was written for.

As ADR-draft-third-party-failures-guarded's *Enforcement* asks:

- each guard carries a marker `// Guard: <crate> <version>` above it, with the lines of the crate
  it was derived from:
  - the brushed line's bounds in `crates/render-agg/src/brush.rs`;
  - the glyph outline's width in `crates/render-agg/src/glyphs.rs`;
  - the tiny shape's scaling and the line's floor in `crates/symbolize/src/geometry.rs`;
  - the finiteness refusals in `crates/symbolize/src/layer_transform.rs`;
- a check under `scripts/` lists the markers under `crates/`. It fails where `Cargo.lock` holds
  the crate at another version, or does not hold it. A crate held at two versions, as i_overlay
  is (4.5.2 under geo, 9.1.0 in `render-agg`), matches if one of them is the marker's;
- `scripts/check-all.sh` runs it, and CI runs it where it runs the other checks of `scripts/`.

## Acceptance criteria

- [ ] Given a fixture whose marker names the locked version, when the self-test runs the
  check, then it passes.
- [ ] Given a fixture whose marker names another version, when the self-test runs the check,
  then it fails and names the file, the crate, and both versions.
- [ ] Given the tree with the markers, when `scripts/check-all.sh` runs, then the check passes
  and lists at least the five guards above.

## Read first

- Touches: `scripts/check-guards.sh` (new), its self-test `scripts/test-check-guards.sh`,
  `scripts/check-all.sh`, `.github/workflows/checks.yml`, `CONTRIBUTING.md` (the scripts list),
  the five files above
- `docs/adr/draft-third-party-failures-guarded.md`; ADR-0007, ADR-0011; `scripts/lib/check.sh`,
  `scripts/lib/selftest.sh`.

## Blocked by

- None (can start immediately)

## Mode

with the human until the maintainer decides `docs/adr/draft-third-party-failures-guarded.md`,
which no blocker under `docs/plan/` can name. Its form, a marker or an exact `=` pin in
`Cargo.toml`, is one of the ADR's open questions. Once decided, the self-test decides it and a
wrong result costs one revert.
