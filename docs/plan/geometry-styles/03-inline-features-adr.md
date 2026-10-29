# Ticket: Decide how inline features live in the style IR

## What to build

An ADR, through `propose-adr`: how a layer's `FEATURE` blocks (`POINTS`, `WKT`, `ITEMS`) are
carried in the style IR and the style document, read as MapServer's `loadFeature` reads them, and
how the engine reads them as a source, a format change under ADR-0020.

## Acceptance criteria

- [ ] Given the proposed ADR, then it names the IR's shape for inline features, the format change,
      and how a source reaches the engine, with its alternatives.

## Read first

- ADR-0014, ADR-0020, ADR-0026; MapServer at the pin: `loadFeature`, `msLoadShapeFromWKT`.

## Blocked by

- None (can start immediately)

## Mode

with the human: it decides an ADR and changes the style document's format (ADR-0007).
