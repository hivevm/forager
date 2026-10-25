# Ticket: Render the cases on wasm32 and compare them byte for byte

## What to build

The engine with the reference renderer is built for `wasm32-unknown-unknown` as a module that
takes a style document and features and returns an RGBA buffer; a native test runs it over every
in-scope case and compares its buffer byte for byte with the native render (G-9). What runs the
module, a pure-Rust interpreter in the test or a JavaScript runtime on CI, and which crate builds
it, is decided first, through `propose-adr`, in its own pull request; should the module need a
host crate of its own, the ADR supersedes ADR-0009 to add it.

## Acceptance criteria

- [ ] Given the proposed ADR, when the test runs, then it runs the module as the ADR decides,
      inside the dev container as on CI.
- [ ] Given each in-scope case, when it is rendered natively and in the module, then the buffers
      are identical, and a difference names the case and the first differing pixel.
- [ ] Given the module, then it is built from the same core crates as the browser bindings.
- [ ] The test cites G-9 and Q-2 with `Verifies:` markers.

## Read first

- ADR-0009, ADR-0012, ADR-0015; Q-1, Q-2, Q-5, G-9 in the specification.
- The browser bindings and the wasm32 build in the Rust checks.

## Blocked by

- docs/plan/conformance-harness/06-geometry-cases.md

## Mode

with the human: it decides an ADR, may supersede ADR-0009, and its module stands beside the public
API of the browser bindings, which the harness keeps with the maintainer
([ADR-0007](../../adr/0007-harness.md)).
