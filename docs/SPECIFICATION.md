# Specification — <Project Name>

> The **specification** of this project: the problem it solves, where it is going, and what has to
> be true for it to have succeeded, written in the vocabulary of [`GLOSSARY.md`](GLOSSARY.md). It
> is the **constitution**: every Architecture Decision Record in [`adr/`](adr/) derives from it and
> must not contradict it ([ADR-0001](adr/0001-agent-governance-model.md)).

## Problem

Teams that maintain their cartography as UMN MapServer mapfiles can render it only on a server:
MapServer is a C program bound to CGI/FastCGI and to C libraries (GDAL, PROJ, AGG, FreeType), and
it does not run in a browser. To show the same maps client-side they render raster tiles on the
server, which costs server capacity and round-trips and leaves the client unable to restyle or
label per viewport, or they rewrite the styling by hand for a web renderer such as MapLibre or
OpenLayers, which loses MapServer semantics (classes, expressions, symbol scaling, the label
cache) and doubles maintenance. Nothing renders a mapfile with MapServer's semantics in
WebAssembly, and nothing embeds that semantics as a pure-Rust library in a server.

The evidence that this is worth solving now is not yet recorded; see *Assumptions*.

## Mission

Forager brings existing MapServer cartography to the browser without a map server, and keeps the
server path on the same code.

## Vision

Forager replaces MapServer's rendering path wherever pixels are produced: in the browser, in a
Rust server, and in a wasmtime sandbox. MapServer remains only as the semantic reference and the
test oracle.

## Strategy

Translate mapfiles at load time into a MapServer-free style IR whose expressions are CQL2. Keep
the core synchronous, free of I/O, and deterministic, with all data behind async ports and
adapters. Grow it in phases, each ending with a running WASM build and green conformance tests
against msautotest.

## Goals / Success Criteria

Acceptance criteria a test can decide, where it helps in the form *When <trigger>, the system
shall <response>*. Each starts with a bold `G-n`, which a test cites with `Verifies: G-n`. Until
that test exists the criterion is pending work, and the traceability check lists it
([ADR-0002](adr/0002-decisions-verified-by-tests.md)). Identifiers are never reused.
Only a normalization renumbers them ([`AGENTS.md` §3](../AGENTS.md#3-adr-rules), rule 5).

1. **G-1** — …
2. **G-2** — …

## Quality Goals

The few properties that drive decisions, in priority order. Every ADR weighs its *Consequences*
against them. Each has a measure concrete enough to settle a disagreement and starts with a bold
`Q-n`, cited like a goal. Security goals, what the system must never do, belong here, derived
from the threats below.

- **Q-1** — <Quality, e.g. Performance>: why it matters, and how it is measured (e.g. "p95 response
  under 200 ms at 100 requests per second").
- **Q-2** — <Quality, e.g. Security, Availability, Portability, Accessibility>: …

## Non-Goals

What this project explicitly does not try to do.

- …

## Constraints

What the solution has to live with regardless of the better choice: platforms, standards,
systems to interoperate with, limits of time, budget, and team. A constraint not written here is
rediscovered as a rejected alternative in an ADR.

- …

## Threats & Forbidden Actions

The assets, the actors who threaten them, and the trust boundaries between the parts. Then what
the system must never do, regardless of what a task asks. A prohibition not written here does not
exist for an agent. One a test can decide is restated as a `Q-n`, so that it is verified.

- **Assets:** …
- **Actors and trust boundaries:** …
- The system shall never …

## Assumptions

What is believed true today and would invalidate the decisions above if it turned out false:
expected scale, the shape of the data, who operates the result, which dependency stays
maintained.

- …
