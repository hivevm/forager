# Ticket: Report a source bound to no layer in the browser as the CLI does

## What to build

Both hosts bind GeoJSON to layers by name and report a source that cannot be read
(`source-unreadable`), each in code of its own: `render` in `crates/cli/src/main.rs:149-177` and
`bind` in `crates/bindings-wasm/src/lib.rs:95-121`. Only the CLI warns about a name that matches
no layer (`source-unbound`, `crates/cli/src/main.rs:151-162`); the bindings bind it silently.
On the example page, where the cartographer types each layer name by hand
(`examples/web/index.html:104-107`), a typo leaves only `layer-skipped` for the layer that got
nothing, and nothing names the source that went nowhere.

The binding of sources to layers by name is written once, in the host-facing part of the GeoJSON
adapter or as a function both hosts call, and both hosts report the same diagnostics for the same
inputs.

Two smaller findings of the same hosts go with it:

- The module comment of `crates/dev-server/src/main.rs:3` says "one request at a time"; the
  server spawns a thread per connection (`crates/dev-server/src/main.rs:45-55`).
- The glossary has no entry for *feature source*, *renderer*, or *viewport*, which the code
  (`FeatureSource`, `Renderer`, `Viewport`), the overview, and the specification use
  ([`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules), rule 6).

## Acceptance criteria

- [ ] Given sources for the layer names `areas` and `area`, of which only `areas` is a layer,
  when the bindings render, then the diagnostics hold one `source-unbound` warning naming `area`.
- [ ] Given the same mapfile and sources, when the CLI and the bindings render them, then both
  report the same codes in the same order.
- [ ] Given the example page's files, when they render, then there are still no diagnostics.
- [ ] The dev server's module comment says what it does, and `docs/GLOSSARY.md` defines the three
  terms.

## Read first

- [ADR-0009](../../adr/0009-ports-and-adapters-crates.md) (who may depend on what),
  [ADR-0016](../../adr/0016-geojson-crate.md); G-8 of `docs/SPECIFICATION.md`.
- `crates/cli/tests/render.rs` and the tests of `crates/bindings-wasm/src/lib.rs` are the seams.

## Blocked by

<!-- Both change crates/cli/src/main.rs and crates/bindings-wasm/src/lib.rs. -->

- docs/plan/revision-2026-10-24/03-one-poll-once.md

## Mode

unattended: a test decides each criterion; the change adds a warning and moves code without
touching the credential filter or the signature of the bindings' `render`, so a wrong result costs
one revert, and the review notices a changed diagnostic. Stop and hand off if keeping the binding
in one place needs an edge [ADR-0009](../../adr/0009-ports-and-adapters-crates.md) does not
allow, or a change to the bindings' public API.
