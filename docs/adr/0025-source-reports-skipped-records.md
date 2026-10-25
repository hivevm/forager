# ADR-0025: A feature source reports the records it skips, and the engine turns them into one warning per layer

- **Status:** 🟡 proposed
- **Date:** 2026-10-25
- **Deciders:** Markus Brigl
- **Applies to:** the `FeatureSource` port of `crates/ports`, its call in `crates/engine`, and every `adapter-<kind>` crate that implements it

## Context

Q-3 asks that invalid geometries and broken files become diagnostics. The feature source port
gives a source two outcomes only: the features of the layer, or a `SourceError` that fails the
layer, which the engine reports as `layer-skipped` (G-8). A source that reads on past a broken
record therefore drops it without a word. The shapefile adapter does so by decision: a record that
fails one of MapServer's guards, or whose box cannot be read, is read as no shape and the layer
reads on ([ADR-0024](0024-shapefile-reader.md)), and that ADR names a diagnostic channel of the
source port as its follow-up. The GeoJSON adapter drops a feature without finite coordinates the
same way. The map then draws fewer features than the data holds, and nothing says why.

Three forces bound the answer. A broken file can hold millions of broken records, and every
diagnostic costs memory and a line of output, so the channel must not report one per record (Q-9).
A source knows records, not the mapfile: the span and the layer's name are the engine's, as they
are for `layer-skipped`. And the port is a public interface, implemented by every adapter and by
the fuzz target `fuzz/fuzz_targets/render.rs` and called by the engine, which the maintainer
keeps ([ADR-0007](0007-harness.md)).

## Decision

We will have a feature source return, beside the features of a query, the number of records it
skipped for that query and the reason of the first, and the engine report a layer with a nonzero
count as one warning `source-records-skipped` naming the layer, its source kind, the count, and
that reason, at the layer's source span.

The count is per query, not per source: it counts the records the query would have considered
had they been readable. A source that drops records once, when it is opened, reports them with
every query whose extent they could fall in; one that has no box to test them against, as the
GeoJSON adapter's features without finite coordinates have none, reports them with every query.
A source that skips per record visited, as the shapefile adapter does, reports the records the
query visited, so a query its file's bounds exclude reports none. Each record counts once per
query, however many passes the source makes over it.

**Out of scope:** what a source counts as a skipped record, which each adapter's ADR and semantics
page decide; a strict mode that fails the map on a skipped record, akin to the strict mode for an
unavailable source that the specification lists among its Non-Goals; diagnostics of the resource
loader and of the projection resolver; the properties of a feature.

## Alternatives considered

- **No channel; Q-3 reworded to exempt records a source skips** — the smallest change, but the
  user sees a map that lacks features without a hint, which is the silence Q-3 forbids.
- **One diagnostic per skipped record** — the most precise, but a broken file of a million records
  makes a million diagnostics; a cap would make it a counted summary with extra steps.
- **A diagnostic sink passed into `features`** — the source writes as it reads, but it either
  needs the span and layer it does not know, or the engine rewrites what it writes; and a sink
  shared across `await` points is harder to hold without `Send` than a returned value.
- **A count without a reason** — enough to say that something was dropped, not enough to say what
  to fix in the file.
- **The source fails the layer on the first broken record** — what MapServer does for some cases,
  which [ADR-0024](0024-shapefile-reader.md) rejected: one broken record would hide a layer.

## Sources / Prior art

- MapServer rel-8-6-6, `src/mapshape.c`: `msSHPReadShape` marks a broken record `MS_SHAPE_NULL`
  and `msSHPLayerNextShape` moves on, logging nothing to the caller; MapServer's own behaviour is
  the silence this ADR departs from.
- GDAL/OGR: a driver reports a skipped feature through `CPLError(CE_Warning, …)` to a
  process-wide error handler, which Forager's ports have no equivalent of; recalled, not yet
  checked against GDAL's source, which the review of this draft does before it is accepted.

## Consequences

- Positive: a map that draws fewer features than its data holds says so, per layer, with a reason
  to act on (Q-3); the number of diagnostics a render adds stays bounded by its layers (Q-9).
- Negative / trade-offs: every implementation of `FeatureSource`, the fuzz target's included,
  changes its return type; the reasons after the first are lost; a source must count while it
  reads. The shapefile adapter's two passes count the matched features, the first to size the
  list and the second to fill it; only one of them may count the skips, or each skip counts
  twice.
- Follow-ups: whether a render's
  diagnostics as a whole are bounded like a load's.

## Enforcement

A test of the engine cites this ADR with `Verifies:`: a source that skips records yields one
`source-records-skipped` warning naming the layer, the count, and the first reason, and a source
that skips none yields none. A test of `adapter-shapefile` feeds a file with one record past its
guards and expects the count of one with its reason.
