# ADR-0030: The encoder writes PNG with the image-rs `png` crate

- **Status:** 🟢 accepted
- **Date:** 2026-10-24
- **Deciders:** Markus Brigl
- **Applies to:** `crates/encode`

## Context

The core encodes its RGBA buffer as PNG in pure Rust (*Constraints*), losslessly (G-12). The
image-rs `png` crate (MIT OR Apache-2.0) is the common pure-Rust codec; the specification's
*Assumptions* name it among the crates the work leans on. `encode` also decodes the PNG images of
pixmap symbols ([ADR-0029](0029-pixmaps-held-once-per-render.md)), and the conformance harness
decodes the reference images with the same crate.

## Decision

We will encode and decode PNG with the image-rs `png` crate in `encode`.

**Out of scope:** JPEG and WebP.

## Alternatives considered

None — cheap to reverse.

## Sources / Prior art

None — cheap to reverse.

## Consequences

None — cheap to reverse.

## Enforcement

The round-trip test of G-12 in `crates/encode/src/lib.rs` cites this ADR with `Verifies:`, and so
do the tests of the pixmap decoder in `crates/encode/src/pixmap.rs`.
