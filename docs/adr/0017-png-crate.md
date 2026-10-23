# ADR-0017: The encoder writes PNG with the image-rs `png` crate

- **Status:** 🟡 proposed
- **Date:** 2026-10-23
- **Deciders:** Markus Brigl
- **Applies to:** `crates/encode`

## Context

The core encodes its RGBA buffer as PNG in pure Rust (*Constraints*), losslessly (G-12). The
image-rs `png` crate (0.18, MIT OR Apache-2.0) is the common pure-Rust codec.

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

The round-trip test of G-12 in `encode` cites this ADR with `Verifies:`.
