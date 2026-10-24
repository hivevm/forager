# Ticket: Read every number of a JSON style document back exactly, and fuzz the round trip

## What to build

A style document written as JSON and read back holds the same numbers bit for bit. Today it does
not: `serde_json` parses floats best-effort unless its `float_roundtrip` feature is on, and
`Cargo.toml:23` takes it without features. Probed on 2026-10-24 with the locked `serde_json`
1.0.154: of 200,000 decimals shaped like mapfile coordinates (below 1e8, nine places), 15,585
come back one unit in the last place off from what Rust's own parse gave, the first being
`27374987.048899982`; `serde-saphyr` 2.0.0 returned every one of 200,000 random finite `f64`
exactly. So `forager read --format json` prints an extent that reads back as another, and G-3
(mapfile → style IR → YAML → JSON → style IR equal) cannot hold.

The unit test of ADR-0014 (`crates/style-ir/src/document.rs`) round-trips one sample with round
numbers, and the `style_document` fuzz target (`fuzz/fuzz_targets/style_document.rs`) only reads;
neither could see it. The check is a property in the `mapfile` fuzz target: every document the
frontend loads round-trips through YAML and JSON to an equal document.

## Acceptance criteria

- [ ] Given a style IR whose extent is `27374987.048899982`, when it is written as JSON and read
  back, then the extent is bit-identical (a regression test in `style-ir` that fails on `main`).
- [ ] Given any input the `mapfile` fuzz target loads to a document, when the target writes it as
  YAML and as JSON and reads each back, then both equal the loaded document, or the target
  panics.
- [ ] Given the bundle, when `scripts/check-rust.sh` measures it, then it stays under Q-5's 3 MB.

## Read first

- [ADR-0014](../../adr/0014-style-document-serde.md),
  [ADR-0012](../../adr/0012-crate-supply-chain.md),
  and G-3 and Q-3 of `docs/SPECIFICATION.md`.
- `crates/style-ir/src/document.rs`, `fuzz/fuzz_targets/mapfile.rs`, `scripts/fuzz.sh`.
- The feature is workspace-wide once enabled, so the GeoJSON adapter and the bindings parse
  coordinates exactly too; that is the intended consequence, not a side effect to avoid.

## Blocked by

- None (can start immediately)

## Mode

with the human: it changes how a style document is read, which the harness keeps with the
maintainer ([ADR-0007](../../adr/0007-harness.md)).
