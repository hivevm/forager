# ADR-0021: The shapefile adapter reads shapefiles with a reader derived from MapServer's

- **Status:** 🟢 accepted
- **Date:** 2026-10-25
- **Deciders:** Markus Brigl
- **Applies to:** `crates/adapter-shapefile`, `docs/semantics/shapefile.md`

## Context

Most cases of the msautotest suite read their features from ESRI shapefiles, so the conformance
harness (G-6) needs a reader for them, and so do users whose mapfiles name a shapefile in `DATA`.
The adapter reads the `.shp`, its `.shx`, and its `.dbf` from bytes the resource loader hands it,
without panicking on a broken file (Q-3) and without letting one exhaust memory (Q-9), and it
builds for `wasm32-unknown-unknown` without C code (Q-1). Only the CLI binds it; the browser
bindings do not ([ADR-0024](0024-browser-bindings-bind-geojson-only.md)).

Which records MapServer reads decides what the reference images show (Q-4), and its reader,
`src/mapshape.c` and `src/mapxbase.c`, decides more than the ESRI description does: it takes the
file's shape type from the `.shx`; for an extent that cuts through the file's bounds it selects
records by the bounding box each record stores, by the point itself in a point file; it reads a
record that fails one of its guards on counts and sizes as no shape and reads the next; and it
reads only some fields of the `.dbf` header, neither the update date nor the language driver.

Its memory, though, is guarded by reading, drawing, and freeing one shape at a time. It allocates
a record's buffer to the size the `.shx` claims and a record's part list before it checks the
sizes against each other, and its index arrays to the record count of the `.shx` header. A feature
source of Forager's returns a layer's features at once, so a `.shx` that points many entries at
one large record, or a `.dbf` header of many zero-width fields, would multiply the files' size in
memory.

Today the source reports the records it skips ([ADR-0022](0022-skipped-records.md)), a feature
carries its record of the `.dbf` as typed properties ([ADR-0023](0023-typed-properties.md)), and
the adapter is a target of the weekly fuzz run (`fuzz/fuzz_targets/shapefile.rs`).

## Decision

We will read shapefiles in `adapter-shapefile` with a reader of the adapter's own, derived record
for record from MapServer's `src/mapshape.c` and `src/mapxbase.c` at the pinned release, with
memory that follows the files' size where MapServer's does not.

- **Arithmetic.** The reader computes counts, sizes, and offsets in checked arithmetic.
- **Text.** The reader decodes `.dbf` text as UTF-8 and replaces an invalid byte.
- **Memory.** The reader keeps memory in proportion to the files:
  - it allocates nothing from a count or a size of a header or a record before it checks it
    against the bytes present;
  - the records read from one `.shp` add up to no more bytes than it holds, and the records past
    that read as no shape;
  - a feature's attributes cost memory in proportion to its record in the `.dbf`, beside a field
    table its layer shares.
- **Semantics page.** `docs/semantics/shapefile.md` writes the reading down, each rule with the
  MapServer function it follows, as the other semantics pages do. Where the reader departs from
  MapServer, the page says so and why.
- **Departure.** Where MapServer selects records by their boxes and `msSHPReadBounds` fails on
  one, MapServer fails the layer. The adapter reads that record as no shape and reads on. A box
  fails to read where its `.shx` size is not 4:
  - a size of 0 to 3;
  - an offset of 0 or past the `.shp`;
  - a box cut short by the end of the file;
  - a box whose least x is not a number.

**Out of scope:** code pages, `.cpg` files, and a layer's `ENCODING`; the index files `.qix` and
`.sbn`; the measure and height of a shape beyond what MapServer's guards check; writing
shapefiles; how attributes reach the feature source port, which
[ADR-0023](0023-typed-properties.md) decides; whether a browser host binds the adapter, which
[ADR-0024](0024-browser-bindings-bind-geojson-only.md) decides.

## Alternatives considered

- **The `shapefile` and `dbase` crates** — the reader in wide use, MIT, wasm32-clean, and no panic
  over about 5,800 truncated and mutated inputs; but the size check casts the computed size to
  `i32` before it compares, so the counts of a record can wrap past it and reserve gigabytes; the
  parts are read and reserved before any check; and `dbase` refuses a `.dbf` whose update date is
  zero, which MapServer opens. The adapter would parse every record header again to guard the
  crate, which leaves little for the crate to do.
- **The crates' geometry with a `.dbf` reader of Forager's own** — the record guards are still the
  adapter's, and `dbase` still compiles in as a dependency of `shapefile`.
- **A reader written from the ESRI specification alone** — the same amount of code, but which
  records MapServer drops and which headers it accepts would be found only by failing cases.
- **MapServer's reader without the memory rules** — what MapServer does, but a few megabytes of
  index or of `.dbf` header could make the source return gigabytes.
- **Records that must follow each other without overlap** — a simpler rule than the byte total,
  but it refuses a reordered `.shx` that MapServer reads.
- **The layer fails on a record whose box cannot be read, as in MapServer** — faithful, but one
  broken entry would hide every feature of a layer, which G-8 reports only as a whole.
- **Code pages honoured now** — MapServer reads a `.cpg` and converts by the layer's encoding,
  which needs an encoding table in a crate a browser may load (Q-5); it waits until a case or a
  user needs it.

## Sources / Prior art

- MapServer rel-8-6-6: `src/mapshape.c` — `msSHPOpen`, `msSHPOpenVirtualFile`, `msSHXLoadPage`,
  `msSHXReadSize`, `msSHPReadAllocateBuffer`, `msSHPReadShape`, `msSHPReadBounds`,
  `msShapefileOpen`, `msShapefileWhichShapes`, `msSHPLayerOpen`, and `msSHPLayerNextShape`;
  `src/mapxbase.c` — `msDBFOpenVirtualFile`, `msDBFReadAttribute`, `msDBFOpen`, and
  `msReadCPGEncoding`.
- ESRI, *ESRI Shapefile Technical Description*, July 1998,
  <https://www.esri.com/content/dam/esrisites/sitecore-archive/Files/Pdfs/library/whitepapers/pdfs/shapefile.pdf>.
- `shapefile` 0.9.0 and `dbase` 0.8.0, <https://github.com/tmontaigu/shapefile-rs> and
  <https://github.com/tmontaigu/dbase-rs>, checked 2026-10-25 in a scratch crate and in three
  reviews: `src/record/polyline.rs` and `src/record/multipoint.rs` cast the size to `i32`;
  `src/record/io.rs` reads the parts before the check; `dbase`'s `src/header.rs` refuses a zero
  date.

## Consequences

- Positive: the adapter drops and keeps the records MapServer drops and keeps (Q-4); a layer costs
  memory in proportion to its files, which the data-file limit of Q-9 bounds; no third-party crate
  in the adapter (Q-6).
- Negative / trade-offs: the reader is Forager's to maintain and to fuzz, and the files derived
  from MapServer's carry its notice (the licence constraint, `scripts/check-notice.sh`).
- Negative / trade-offs: text that is not UTF-8 reads with replacement characters where MapServer
  would convert it by the `.cpg`.
- Negative / trade-offs: a `.shx` that names one record twice reads the second as no shape once
  the byte total is spent, where MapServer draws it again.
- Negative / trade-offs: the source still returns a layer's features before the engine counts them
  against the feature limit of Q-9; the bound is the files' size, not that limit.
- Follow-ups: a source that stops at the feature limit, with the Q-9 limits of the sources; code
  pages and `ENCODING`, when a case or a user needs them; the size of the bundle (Q-5) if a
  browser host ever binds the adapter.

## Enforcement

Tests in `crates/adapter-shapefile/tests/read.rs` cite this ADR with `Verifies:`, each rule of
`docs/semantics/shapefile.md` by a test of its own, and each case of the departure, read as no
shape while the layer reads on.

The test `memory_follows_the_files_size` in `crates/adapter-shapefile/tests/memory.rs` cites this
ADR with `Verifies:`. It runs in a test binary of its own, with a counting allocator, and asserts
that the peak allocation stays under four times the files' size plus one mebibyte for:

- a record with counts under MapServer's guards but past its size;
- a record whose offset or size lies outside the `.shp`;
- a `.shx` header whose record count exceeds the entries the file holds;
- a `.shx` that points many entries at one large record, read until the byte total is spent;
- a `.dbf` header of many zero-width fields over many records.
