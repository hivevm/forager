# Ticket: Decide what G-5 asks of the HTML example (drafted specification change)

## What to build

G-5 has two *shall*s: the WASM bindings return the rendered RGBA image, "and the HTML example
shall display it". The one test that cites it,
`the_cli_fixture_renders_to_the_engines_reference_image`
(`crates/bindings-wasm/src/lib.rs:259-273`),
decides the first natively; nothing decides the second. The browser tracer's spec left a test in
a browser to a later spec, #6 and #11 list the page as never opened in a browser, and
[ADR-0008](../../adr/0008-rust-toolchain.md) puts the browser test runner out of scope. The
traceability check counts G-5 as met all the same. The convention *A `Verifies:` marker claims
every shall of its criterion* in `docs/CONVENTIONS.md` makes the marker a defect until one of the
two drafts below lands.

**Draft A — split the criterion.** In `## Goals / Success Criteria`:

> 5. **G-5** — When the WASM bindings receive a style, GeoJSON features, and a viewport, they
>    shall return the rendered RGBA image.
>
> 13. **G-13** — When the HTML example is opened in a browser with the bundle served, it shall
>     display the image the bindings return for its sample mapfile and GeoJSON, and list their
>     diagnostics.

The marker stays on the bindings test; G-13 is pending until a browser test exists, and the
browser test runner that G-9 needs serves it as well.

**Draft B — keep G-5 as written.** No specification change: the `Verifies: G-5` marker leaves the
bindings test, and G-5 is pending until a test in a browser decides both parts.

Accepted ADRs whose basis either draft moves, for the maintainer to confirm:
[ADR-0008](../../adr/0008-rust-toolchain.md) (its browser test runner, out of scope, named for
G-9 alone), [ADR-0009](../../adr/0009-ports-and-adapters-crates.md) (cites G-5 for the browser
host), [ADR-0016](../../adr/0016-geojson-crate.md) (cites G-5 for the browser example rendering
GeoJSON — under draft A that is G-13).

## Acceptance criteria

- [ ] Given the maintainer's choice, when it lands, then `docs/SPECIFICATION.md` (draft A) or the
  marker (draft B) changes in a pull request of its own, apart from code, naming the ADRs above.
- [ ] Given the landed choice, when `scripts/check-traceability.sh` runs, then no test cites a
  criterion it decides only in part, and the pending list names what is left for a browser.

## Read first

- G-5 and G-9 of `docs/SPECIFICATION.md`; [`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules),
  rule 7; [ADR-0002](../../adr/0002-decisions-verified-by-tests.md).

## Blocked by

- None (can start immediately)

## Mode

with the human: a change to the specification is the maintainer's to land
([`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules), rule 7).
