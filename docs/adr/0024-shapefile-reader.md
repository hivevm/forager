# ADR-0024: The shapefile adapter reads shapefiles with a reader derived from MapServer's

- **Status:** 🟡 proposed
- **Date:** 2026-10-25
- **Deciders:** Markus Brigl
- **Applies to:** `crates/adapter-shapefile`, `docs/semantics/shapefile.md`

## Context

Most cases of the msautotest suite read their features from ESRI shapefiles, so the conformance
harness (G-6) needs a reader for them, and so do users whose mapfiles name a shapefile in `DATA`.
The adapter reads the `.shp`, its `.shx`, and its `.dbf` from bytes the resource loader hands it,
without panicking on a broken file (Q-3) and without letting one exhaust memory (Q-9), and it
builds for `wasm32-unknown-unknown` without C code (Q-1), although only the CLI binds it for now.

Which records MapServer reads decides what the reference images show (Q-4), and its reader,
`src/mapshape.c` and `src/mapxbase.c`, decides more than the ESRI description does: it takes the
file's shape type from the `.shx`, selects records for an extent by the bounding box each record
stores, reads a record that fails one of its guards on counts and sizes as no shape and reads the
next, and reads only some fields of the `.dbf` header, neither the update date nor the language
driver.

Its memory, though, is guarded by reading, drawing, and freeing one shape at a time. It allocates
a record's buffer to the size the `.shx` claims and a record's part list before it checks the
sizes against each other, and its index arrays to the record count of the `.shx` header. A
feature source of Forager's returns a layer's features at once, so a `.shx` that points many
entries at one large record, or a `.dbf` header of many zero-width fields, would multiply the
files' size in memory.

## Decision

We will read shapefiles in `adapter-shapefile` with a reader of the adapter's own, derived record
for record from MapServer's `src/mapshape.c` and `src/mapxbase.c` at the pinned release, in
checked arithmetic, with `.dbf` text decoded as UTF-8 and an invalid byte replaced, and with
memory that follows the files' size where MapServer's does not: nothing is allocated from a count
or a size of a header or a record before it is checked against the bytes present, the records read
from one `.shp` add up to no more bytes than it holds, the records past that read as no shape, and
a feature's attributes cost no more than its record in the `.dbf` and a field table its layer
shares.

The reading is written down in `docs/semantics/shapefile.md`, each rule with the MapServer
function it follows, as the other semantics pages are; where the reader departs from MapServer,
the page says so and why. One departure is decided here: a record whose bounding box cannot be
read is no shape, where MapServer fails the layer when it selects records by their boxes.

**Out of scope:** code pages, `.cpg` files, and a layer's `ENCODING`; the index files `.qix` and
`.sbn`; the measure and height of a shape beyond what MapServer's guards check; writing
shapefiles; how attributes reach the feature source port; whether a browser host binds the
adapter.

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
- **Code pages honoured now** — MapServer reads a `.cpg` and converts by the layer's encoding, which
  needs an encoding table in a crate a browser may load (Q-5); it waits until a case or a user
  needs it.

## Sources / Prior art

- MapServer rel-8-6-6: `src/mapshape.c` — `msSHPOpenVirtualFile`, `msSHXLoadPage`,
  `msSHXReadSize`, `msSHPReadAllocateBuffer`, `msSHPReadShape`, `msShapefileWhichShapes`,
  `msSHPLayerOpen`, and `msSHPLayerNextShape`; `src/mapxbase.c` — `msDBFOpenVirtualFile`,
  `msDBFReadAttribute`, `msDBFOpen`, and `msReadCPGEncoding`.
- ESRI, *ESRI Shapefile Technical Description*, July 1998,
  <https://www.esri.com/content/dam/esrisites/sitecore-archive/Files/Pdfs/library/whitepapers/pdfs/shapefile.pdf>.
- `shapefile` 0.9.0 and `dbase` 0.8.0, <https://github.com/tmontaigu/shapefile-rs> and
  <https://github.com/tmontaigu/dbase-rs>, checked 2026-10-25 in a scratch crate and in three
  reviews: `src/record/polyline.rs` and `src/record/multipoint.rs` cast the size to `i32`;
  `src/record/io.rs` reads the parts before the check; `dbase`'s `src/header.rs` refuses a zero
  date.

## Consequences

- Positive: the adapter drops and keeps the records MapServer drops and keeps (Q-4); a layer
  costs memory in proportion to its files, which the data-file limit of Q-9 bounds; no
  third-party crate in the adapter (Q-6).
- Negative / trade-offs: the reader is Forager's to maintain and to fuzz, and the files derived
  from MapServer's carry its notice (the licence constraint, `scripts/check-notice.sh`). Text that
  is not UTF-8 reads with replacement characters where MapServer would convert it by the `.cpg`.
  A `.shx` that names one record twice reads the second as no shape once the byte total is spent,
  where MapServer draws it again. The source still returns a layer's features before the engine
  counts them against the feature limit of Q-9; the bound is the files' size, not that limit.
- Follow-ups: features that carry their attributes through the port; a source that stops at the
  feature limit, with the Q-9 limits of the sources; the adapter as a target of the weekly fuzz run
  (Q-3); code pages and `ENCODING`, when a case or a user needs them; the size of the bundle (Q-5)
  once a browser host binds the adapter.

## Enforcement

A test of `adapter-shapefile` cites this ADR with `Verifies:`, and so does each rule of
`docs/semantics/shapefile.md` by a test of its own. Among them, run in a test binary of their own
with a counting allocator that asserts the peak allocation stays under a stated multiple of the
files' size: a record with counts under MapServer's guards but past its size; a record whose
offset or size lies outside the `.shp`; a `.shx` header whose record count exceeds the entries the
file holds; a `.shx` that points many entries at one large record, read until the byte total is
spent; a `.dbf` header of many zero-width fields over many records. And a record whose bounding
box lies outside the `.shp`, read as no shape while the layer reads on.
