# ADR-0012: Crate dependencies come only from crates.io, are locked in `Cargo.lock`, and pass `cargo deny` for licences, bans, advisories, and sources

- **Status:** 🟡 proposed
- **Date:** 2026-10-23
- **Deciders:** Markus Brigl
- **Applies to:** `Cargo.lock`, `deny.toml`, and every `[dependencies]` entry of the workspace

## Context

Forager is released as `MIT OR Apache-2.0` and may depend only on permissive licences (Q-6). The
core must link no C code (Q-1), and the browser bundle has a budget (Q-5). Every dependency runs in
the user's browser and in hosts that hold credentials (Q-7). RustSec publishes advisories for
crates.io; `cargo deny` checks licences, banned crates, advisories, and the origin of every crate.

## Decision

We will commit `Cargo.lock`, take crates only from crates.io, and run `cargo deny check` with all
four checks in `scripts/check-rust.sh`: `licenses` allows only permissive licences (MIT,
Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Zlib, Unicode-3.0, and others the maintainer adds in
review); `bans` denies the C-binding crates of the core's domain (`proj-sys`, `gdal-sys`,
`freetype-sys`, and the like) and duplicate versions as warnings; `advisories` fails on a RustSec
vulnerability or an unmaintained crate; `sources` fails on any registry or Git source other than
crates.io.

**Out of scope:** the choice of each crate, made by the ADR that first needs it.

## Alternatives considered

- **Licences only** — Q-6 alone; leaves advisories and Git sources unchecked for code that runs in
  every user's browser.
- **Vendoring** — reproducible offline builds; heavy for a workspace this size, and `Cargo.lock`
  with crates.io checksums already pins the content.

## Sources / Prior art

- cargo-deny, <https://github.com/EmbarkStudios/cargo-deny> (0.20.2, checked 2026-10-23).
- RustSec advisory database, <https://rustsec.org/>.

## Consequences

- Positive: a non-permissive licence, a vulnerable crate, or a Git dependency fails the pull
  request that adds it.
- Negative / trade-offs: an advisory published overnight turns `main` red on the weekly run
  without a change; it is fixed or ignored with a reason in `deny.toml`.
- Follow-ups: none.

## Enforcement

`cargo deny check` in `scripts/check-rust.sh`; a test that runs it against a fixture manifest
with a forbidden licence and expects the failure cites this ADR with `Verifies:`.
