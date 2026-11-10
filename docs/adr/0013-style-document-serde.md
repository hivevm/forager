# ADR-0013: The style document is serialized through serde, as JSON with `serde_json` and as YAML with `serde-saphyr`

- **Status:** 🟢 accepted
- **Date:** 2026-10-24
- **Deciders:** Markus Brigl
- **Applies to:** `crates/style-ir` and every crate that reads or writes a style document

## Context

The style IR is written and read as a style document in YAML and in JSON, through one schema,
convertible losslessly into each other (G-3); a newer minor version loads with a diagnostic naming
each unknown field and its location (G-4); the crate builds for `wasm32-unknown-unknown` (Q-1) and
must not panic on user input (Q-3). serde is the Rust ecosystem's data model for this; the choice
is which format crates sit on it.

For YAML, `serde_yaml` is deprecated by its author; `serde_yml`, its best-known fork, carries the
RustSec advisory RUSTSEC-2025-0068 (unsound, archived); `serde_norway`, another fork, has had no
release since December 2024. `serde-saphyr` is maintained, written without unsafe code, built and
tested in CI for `wasm32-unknown-unknown`, licensed `MIT OR Apache-2.0`, and reports source spans.

Today `crates/style-ir/src/document.rs` reads and writes the style document through both crates,
at `serde-saphyr` 2.0.0 and `serde_json` 1.0.154, the latter with its `float_roundtrip` feature so
that every number reads back bit for bit (G-3). The format versioning, its JSON Schema, and the
lenient reading of a newer minor sit on the same serde types
([ADR-0014](0014-style-document-versions.md)).

## Decision

We will derive serde's `Serialize` and `Deserialize` for the style IR and read and write the style
document with `serde_json` for JSON and `serde-saphyr` for YAML.

**Out of scope:** the JSON Schema of each format version, its generator, and the migration of
older majors, which [ADR-0014](0014-style-document-versions.md) decides; the shape of the style
IR.

## Alternatives considered

- **`serde_yml`** — unsound per RUSTSEC-2025-0068 and archived.
- **`serde_yaml`** — deprecated, no further releases.
- **`serde_norway`** — a fork without a release since 2024, built on a transpiled C parser
  (`unsafe-libyaml`).
- **A YAML parser without serde (`saphyr`, `yaml-rust2`)** — a second, hand-written mapping beside
  the JSON one, against the single schema of G-3.

## Sources / Prior art

- crates.io, checked 2026-10-23: `serde-saphyr` 2.0.0 (2026-10-13, repository
  <https://github.com/bourumir-wyngs/serde-saphyr>), `serde_json` 1.0.154, `serde_yml` 0.0.13,
  `serde_norway` 0.9.42, `serde_yaml` 0.9.34+deprecated.
- RustSec, RUSTSEC-2025-0068, <https://rustsec.org/advisories/RUSTSEC-2025-0068.html>.

## Consequences

- Positive: one derived schema for both formats; spans from the YAML reader for the diagnostics
  of G-4; no unsafe code in the YAML path.
- Negative / trade-offs: `serde-saphyr` is younger and has one main maintainer; a change of the
  YAML crate is local to `style-ir`.
- Follow-ups: none; the JSON Schema and the format versioning came with
  [ADR-0014](0014-style-document-versions.md).

## Enforcement

The tests `a_document_reads_back_equal_from_yaml_and_from_json` and
`numbers_read_back_from_json_bit_for_bit` in `crates/style-ir/src/document.rs` read one style IR
from YAML and from JSON and expect it equal, and cite this ADR with `Verifies:`. The build of every
browser crate for `wasm32-unknown-unknown` in `scripts/check-rust.sh` builds `style-ir` with both
crates.
