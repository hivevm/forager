# Ticket: Report the records a feature source skips

## What to build

The shapefile adapter reads a record that fails MapServer's guards as no shape and drops it
without a diagnostic, because the feature source port has no channel for one
([ADR-0024](../../adr/0024-shapefile-reader.md), *Consequences*); the GeoJSON adapter drops a
feature without finite coordinates the same way. Q-3 asks that broken files become diagnostics.
The `proposed` [ADR-0025](../../adr/0025-source-reports-skipped-records.md) decides that a source
returns the count of the records it skipped and the reason of the first beside its features, and
that the engine reports a layer with a nonzero count as one warning `source-records-skipped`.
This ticket implements it, once the human has reviewed the ADR, and both adapters count what
they skip.

## Acceptance criteria

- [ ] Given a source that skips three records, when the engine renders its layer, then the
  diagnostics hold one `source-records-skipped` warning naming the layer, its source kind, the
  count 3, and the first reason, at the layer's source span (`crates/engine/tests/render.rs`,
  citing ADR-0025).
- [ ] Given a source that skips none, when the engine renders its layer, then no such warning is
  reported.
- [ ] Given a shapefile with one record past MapServer's guards and one whose box cannot be read,
  when the source is queried with an extent across the file's bounds, then it reports a count of
  two with the first one's reason, and the same two when it is queried again: the count is per
  query, each record once however many passes the adapter makes
  (`crates/adapter-shapefile/tests/read.rs`, citing ADR-0025).
- [ ] Given the same shapefile, when it is queried with an extent outside the file's bounds, then
  it reports a count of zero: the query visited no record.
- [ ] Given a shapefile with a valid null shape (a record of size 4), when the source is queried,
  then it reports a count of zero: a null shape is no shape as MapServer reads it, not a skipped
  record.
- [ ] Given a GeoJSON document with one feature without finite coordinates, when the source is
  queried twice with different extents, then each query reports a count of one: the feature is
  dropped once at parse time and reported with every query, as ADR-0025 decides for a record
  without a box.
- [ ] Given `docs/ARCHITECTURE.md`, when the change lands, then the `adapter-shapefile` bullet no
  longer says that a record it cannot read is dropped without a diagnostic, and says the engine
  reports it per layer.
- [ ] Given `docs/semantics/shapefile.md`, when the change lands, then it says the dropped records
  are reported, and the ADR-0024 consequence no longer holds.

## Read first

- [ADR-0025](../../adr/0025-source-reports-skipped-records.md),
  [ADR-0024](../../adr/0024-shapefile-reader.md),
  [ADR-0016](../../adr/0016-geojson-crate.md), [ADR-0007](../../adr/0007-harness.md) (the port
  traits stay with the maintainer); G-8 and Q-3 of `docs/SPECIFICATION.md`; the diagnostic-code
  entry of `docs/CONVENTIONS.md`.
- `crates/ports/src/lib.rs` (`FeatureSource`), `features` in `crates/engine/src/lib.rs`.

## Blocked by

- docs/plan/revision-2026-10-24/03-one-poll-once.md
- docs/plan/revision-2026-10-24/04-hosts-bind-sources-alike.md
- docs/plan/conformance-harness/12-features-carry-properties.md

## Mode

with the human: it changes a port trait, a public interface ADR-0007 keeps with the maintainer,
and the ADR it implements is still to be reviewed. The properties of conformance-harness ticket
12 change the same port first, so it waits for that one. revision-2026-10-24 ticket 03 adds
`poll_once` to `crates/ports/src/lib.rs` and rewrites the polling of
`fuzz/fuzz_targets/render.rs`, whose `FeatureSource` this ticket changes. Ticket 04 of that spec
is stacked on 03 and lands with its changes; on its own it edits the hosts, adds
`crates/engine/src/bind.rs` beside the engine's `features`, and rewrites the engine bullet of
`docs/ARCHITECTURE.md`, which this ticket edits too.
