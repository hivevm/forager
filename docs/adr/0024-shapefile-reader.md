# ADR-0024: The shapefile adapter reads shapefiles with a reader derived from MapServer's

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
Which records MapServer reads, which it drops as broken, and how it reads the headers decide what
the reference images show (Q-4).

MapServer's reader, `src/mapshape.c` and `src/mapxbase.c`, reads as follows:

- **Files.** A `.shp` or `.shx` header shorter than 100 bytes, a `.shx` whose first four bytes are
  not the file code, and a `.shx` record count outside 0 to 256,000,000 are errors of the file.
  The file's shape type is byte 32 of the `.shx`; a record's own type is not read. Entries the
  count names beyond those the `.shx` holds read as size 0, so their records are no shape.
- **Records.** A record whose content is 4 bytes is a null shape. A record of more than
  50,000,000 points or 10,000,000 parts, one whose `44 + 8 + 4 × parts + 16 × points` exceeds its
  size in the `.shx`, and one with a part that starts below 0, ends past the points, or holds no
  point is read as no shape, and the next record is read. A multipoint has the same point guard
  and its own size; a point record shorter than 28 bytes is no shape. Points are read in order,
  whatever the first part's start.
- **Attributes.** Of a `.dbf` header it reads the record count, read as 0 where its high byte is
  128 or more, the header length, the record length, and the fields, `(header length − 32) / 32`
  of them; a header length of 32 or less and a field past the record length are errors of the
  file. It reads neither the version, nor the date of the last update, nor the language driver;
  a `.cpg` names the layer's encoding where `ENCODING` does not.
- **Allocation.** MapServer allocates before some of its checks: the record buffer to the size
  the `.shx` claims, the part list before the size formula, and the index arrays to the header's
  record count. It reads, draws, and frees one shape at a time, so records that the `.shx` points
  at the same bytes cost it time, not memory. A feature source of Forager's returns a layer's
  features at once.

## Decision

We will read shapefiles in `adapter-shapefile` with a reader of the adapter's own, derived from
MapServer's `src/mapshape.c` and `src/mapxbase.c` at the pinned release as the Context describes
it, in checked arithmetic, with `.dbf` text decoded as UTF-8 and an invalid byte replaced, and
with three checks MapServer does not make: a record's offset and size lie inside the `.shp` before
anything is allocated for it, the index is read only as far as the `.shx` holds entries, and the
records read from one `.shp` add up to no more bytes than it holds, the records past that read as
no shape.

**Out of scope:** code pages, `.cpg` files, and a layer's `ENCODING`; the index files `.qix` and
`.sbn`; the measure and height of a shape beyond reading its x and y; writing shapefiles; whether a
browser host binds the adapter.

## Alternatives considered

- **The `shapefile` and `dbase` crates** — the reader in wide use, MIT, wasm32-clean, and no panic
  over about 5,800 truncated and mutated inputs; but the size check casts the computed size to
  `i32` before it compares, so 2^28 points in a 48-byte record of one part pass it and reserve
  4 GiB; the parts are read and reserved before any check; and `dbase` refuses a `.dbf` whose
  update date is zero, which MapServer opens. Its refusal of a language driver that names a code
  page is avoided by opening the `.dbf` through `ReaderBuilder` with `UnicodeLossy`, as
  `shapefile` itself does. The adapter would parse every record header again to guard the crate,
  which leaves little for the crate to do.
- **The crates' geometry with a `.dbf` reader of Forager's own** — the record guards are still the
  adapter's, and `dbase` still compiles in as a dependency of `shapefile`.
- **A reader written from the ESRI specification alone** — the same amount of code, but which
  records MapServer drops and which headers it accepts would be found only by failing cases.
- **MapServer's reader without the three checks** — what MapServer does, but a `.shx` of a few
  megabytes could then point a million records at one large record and have the source return a
  million copies of it.
- **Records that must follow each other without overlap** — a simpler rule than the byte total,
  but it refuses a reordered `.shx` that MapServer reads.
- **Code pages honoured now** — MapServer reads a `.cpg` and converts by the layer's encoding, which
  needs an encoding table in a crate a browser may load (Q-5); it waits until a case or a user
  needs it.

## Sources / Prior art

- MapServer rel-8-6-6: `src/mapshape.c` — `msSHPOpenVirtualFile` (the headers, the record count,
  the shape type), `msSHXLoadPage` and `msSHXReadSize` (the index), `msSHPReadAllocateBuffer`,
  `msSHPReadShape` (the record guards), `msSHPLayerOpen` (the `.cpg` as the layer's encoding), and
  `msSHPLayerNextShape` (one shape at a time); `src/mapxbase.c` — `msDBFOpenVirtualFile` (the
  `.dbf` header), `msDBFReadAttribute`, `msDBFOpen`, and `msReadCPGEncoding`.
- ESRI, *ESRI Shapefile Technical Description*, July 1998,
  <https://www.esri.com/content/dam/esrisites/sitecore-archive/Files/Pdfs/library/whitepapers/pdfs/shapefile.pdf>.
- `shapefile` 0.9.0 and `dbase` 0.8.0, <https://github.com/tmontaigu/shapefile-rs> and
  <https://github.com/tmontaigu/dbase-rs>, checked 2026-10-25 in a scratch crate and in two
  reviews: `src/record/polyline.rs` and `src/record/multipoint.rs` cast the size to `i32`;
  `src/record/io.rs` reads the parts before the check; `dbase`'s `src/header.rs` refuses a zero
  date, and its `src/file.rs` chooses the decoding from the language-driver byte unless the
  reader is built with one.

## Consequences

- Positive: the adapter drops and keeps the records MapServer drops and keeps (Q-4). Nothing is
  allocated for a record before its counts and its place in the file are checked; each record
  then costs memory in proportion to its size in the file, and all of them in proportion to the
  `.shp`, which the data-file limit of Q-9 bounds. No third-party crate in the adapter (Q-6).
- Negative / trade-offs: the reader is Forager's to maintain and to fuzz, and the files derived
  from MapServer's carry its notice (the licence constraint, `scripts/check-notice.sh`). Text that
  is not UTF-8 reads with replacement characters where MapServer would convert it by the `.cpg`.
  A `.shx` that names one record twice reads the second as no shape once the byte total is spent,
  where MapServer draws it again.
- Follow-ups: the adapter as a target of the weekly fuzz run (Q-3); code pages and `ENCODING`, when
  a case or a user needs them; the size of the bundle (Q-5) once a browser host binds the adapter.

## Enforcement

A test of `adapter-shapefile` cites this ADR with `Verifies:` and reads: every shape type it
supports; a record past each of MapServer's guards, read as no shape while the next record is
read; a record whose offset or size lies outside the `.shp`, and 2^28 points in a 48-byte record
of one part, each read as no shape with a counting allocator showing nothing allocated beyond the
file's size; a `.shx` that points many entries at one record, read until the byte total is spent;
a record count past the entries the `.shx` holds; each header error of the Context; a `.dbf` with a
field of each type, with a language driver that names a code page, with a zero update date, and
with a byte that is not UTF-8.
