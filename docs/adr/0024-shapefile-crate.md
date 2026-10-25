# ADR-0024: The shapefile adapter reads with the `shapefile` and `dbase` crates

- **Status:** 🟡 proposed
- **Date:** 2026-10-25
- **Deciders:** Markus Brigl
- **Applies to:** `crates/adapter-shapefile`

## Context

Most cases of the msautotest suite read their features from ESRI shapefiles, so the conformance
harness (G-6) needs a reader for them, and so do users whose mapfiles name a shapefile in `DATA`.
The adapter reads the `.shp`, its `.shx`, and its `.dbf` from bytes the resource loader hands it,
without panicking on a broken file (Q-3) and without letting one exhaust memory (Q-9), and it
builds for `wasm32-unknown-unknown` without C code (Q-1), although only the CLI binds it for now.
MapServer passes a `.dbf`'s text on as bytes and converts it only by a layer's `ENCODING`.

## Decision

We will read shapefiles with the `shapefile` crate and their attributes with the `dbase` crate it
re-exports, both without optional features, from in-memory readers, with `.dbf` text decoded as
UTF-8 and an invalid byte replaced.

**Out of scope:** code pages, `.cpg` files, and a layer's `ENCODING`; the index files `.qix` and
`.sbn`; writing shapefiles; whether a browser host binds the adapter.

## Alternatives considered

- **A reader of Forager's own** — the formats are small and every guard would be ours, but each
  shape type, the `.dbf` field types, and their edge cases would be written and tested again, for
  a reader the crate already hardens.
- **The `shapefile` crate for geometry, an own `.dbf` reader** — it would drop `dbase` and its
  `time` dependency, but splits one format's reading in two and keeps the larger half, the
  attribute types, ours.
- **Code pages through the `encoding_rs` feature** — MapServer's own default is the raw bytes, so
  honouring code pages is a feature beyond it, and it adds a dependency to a crate a browser may
  load; it waits until a case or a user needs it.

## Sources / Prior art

- `shapefile` 0.9.0 and `dbase` 0.8.0, <https://github.com/tmontaigu/shapefile-rs> and
  <https://github.com/tmontaigu/dbase-rs> (MIT), both released 2026-07-05, checked 2026-10-25 in a
  scratch crate:
  - their dependencies are `byteorder`, `bufrw`, and `time` with its own, all MIT or
    `MIT OR Apache-2.0`; `cargo deny check` with this repository's `deny.toml` passes
    advisories, bans, licences, and sources;
  - they build for `wasm32-unknown-unknown`, and `cargo tree` names no `-sys` crate; a release
    module that reads all three files is 47 KB gzipped;
  - `ShapeReader::with_shx` and `dbase::Reader::new` read from any `Read + Seek`, a `Cursor` over
    the loader's bytes among them;
  - a record's point and part counts are checked against its stated size before its points are
    read, and its vectors are reserved with `try_reserve`, so a count too large is an error;
  - truncating a written shapefile of three polygons with holes at every length, and setting
    each byte of each of its three files to 0x00, 0x7f, 0x80, and 0xff, about 4,800 inputs, gave
    errors and no panic;
  - `dbase` reads the clock only when it writes a header.
- MapServer rel-8-6-6, `src/mapshape.c` and `src/mapxbase.c`: the reader the adapter stands in
  for.

## Consequences

- Positive: a maintained reader in wide use, which already turns broken files into errors (Q-3)
  and builds for the browser (Q-1).
- Negative / trade-offs: the reservation follows the counts a record states, bounded by the
  record's stated size but not by the file's length; the adapter checks each `.shx` entry against
  the `.shp`'s length before it reads a record, and the data-file limit of Q-9 bounds the rest.
  Text that is not UTF-8 reads with replacement characters where MapServer would pass the bytes on.
  `time` comes in with `dbase`, for a clock the reading never calls (Q-2).
- Follow-ups: code pages and `ENCODING`, when a case or a user needs them; the size of the bundle
  (Q-5) once a browser host binds the adapter.

## Enforcement

A test of `adapter-shapefile` reads every shape type it supports, a `.dbf` with a field of each
type, and truncated and inconsistent files, and cites this ADR with `Verifies:`.
