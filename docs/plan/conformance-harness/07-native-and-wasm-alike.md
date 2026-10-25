# Ticket: Render the cases on wasm32 and compare them byte for byte

## What to build

The engine with the reference renderer is built for `wasm32-unknown-unknown` as a module that
takes a style document and features and returns an RGBA buffer; a native test runs it over every
in-scope case and compares its buffer byte for byte with the native render (G-9). What runs the
module, a pure-Rust interpreter in the test or a JavaScript runtime on CI, is decided first,
through `propose-adr`.

## Acceptance criteria

- [ ] Given a `proposed` ADR on the runner, when the maintainer accepts it, then the test runs the
      module that way, inside the dev container as on CI.
- [ ] Given each in-scope case, when it is rendered natively and in the module, then the buffers
      are identical, and a difference names the case and the first differing pixel.
- [ ] Given the module, then it is built from the same crates as the browser bindings, without the
      JavaScript glue.
- [ ] The test cites G-9 and Q-2 with `Verifies:` markers.

## Read first

- ADR-0009, ADR-0012, ADR-0015; Q-1, Q-2, Q-5, G-9 in the specification.
- The browser bindings and the wasm32 build in the Rust checks.

## Blocked by

- docs/plan/conformance-harness/05-geometry-cases.md

## Mode

with the human: it decides an ADR, and its module stands beside the public API of the browser
bindings, which the harness keeps with the maintainer ([ADR-0007](../../adr/0007-harness.md)).
