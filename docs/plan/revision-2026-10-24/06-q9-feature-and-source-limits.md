# Ticket: Test the feature and data file limits of Q-9

## What to build

The specification part of this ticket has landed: Q-9 of `docs/SPECIFICATION.md` names the
features one layer draws (1,000,000) and the size of a data file a host reads (256 MiB), the
numbers in the code, `MAX_FEATURES_PER_LAYER` (`crates/engine/src/lib.rs`) and the CLI's
`MAX_SOURCE_BYTES` (`crates/cli/src/lib.rs`). The ticket is narrowed to the test per limit Q-9
asks for, which neither has:

- No engine test feeds a layer past `MAX_FEATURES_PER_LAYER`; the limit tests of
  `crates/engine/tests/render.rs` hold the image size and the markers, not the features.
- No CLI test binds a `--source` GeoJSON file past `MAX_SOURCE_BYTES`, which `read_bounded` in
  `crates/cli/src/lib.rs` refuses; `crates/cli/tests/shapefile.rs` holds a shapefile's files past
  the same limit, and `crates/cli/tests/render.rs` the fonts.

Measured on `main` 9d2a9bc (release build), a GeoJSON file of 245 MiB with 2,200,000 line
features peaks at 1.1 GB resident in `forager render`, and the layer is skipped only after the
source has delivered every feature: the feature limit bounds the drawing, not the memory. Two
questions the draft left open stay the maintainer's and are not this ticket's: whether a feature
source should stop at the limit while it reads, which changes the `FeatureSource` port, and
whether Q-9 adds a budget of bytes across one render.

## Acceptance criteria

- [ ] Given a source that delivers one feature more than `MAX_FEATURES_PER_LAYER`, when the
  engine renders its layer, then the layer is skipped with `layer-skipped` naming the limit
  (`crates/engine/tests/render.rs`, citing Q-9).
- [ ] Given a GeoJSON file one byte past `MAX_SOURCE_BYTES`, when `forager render` binds it with
  `--source`, then the layer is skipped with `source-unreadable` naming the limit
  (`crates/cli/tests/render.rs`, citing Q-9; a sparse file keeps the fixture small).

## Read first

- Q-9 of `docs/SPECIFICATION.md`; `MAX_FEATURES_PER_LAYER` and where it skips a layer in
  `crates/engine/src/lib.rs`; `MAX_SOURCE_BYTES` and `read_bounded` in `crates/cli/src/lib.rs`.
- `each_file_of_a_shapefile_past_the_data_file_limit_skips_its_layer` in
  `crates/cli/tests/shapefile.rs`, the golden path of a test past the data file limit.

## Blocked by

- None (can start immediately)

## Mode

unattended: the two tests decide its criteria; it adds tests only, so a wrong result costs one
revert. It changes no resource limit, which [ADR-0007](../../adr/0007-harness.md) keeps with the
maintainer: it tests two limits as the specification and the code already state them. Its branch
counts against ADR-0007's cap of three open agent branches. A wrong result shows as a test that
fails or does not cite Q-9; the diagnostics the tests assert prove it right. Stop: if either test
needs a constant, a diagnostic, or a port to change, or past two hours, leave the state here and
hand off.
