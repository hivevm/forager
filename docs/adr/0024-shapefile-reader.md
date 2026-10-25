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
Which records MapServer reads, which it drops as broken, and how it reads a `.dbf` header decide
what the reference images show (Q-4).

MapServer's own reader, `src/mapshape.c` and `src/mapxbase.c`, guards each record before it
allocates: a record of more than 50,000,000 points or 10,000,000 parts, one whose
`44 + 8 + 4 × parts + 16 × points` exceeds its size in the `.shx`, and a part without points or
out of order are read as no shape, and the next record is read. A `.shx` whose header is short or
whose record count does not fit is an error of the file. Of a `.dbf` header it reads the record
count, the header length, the record length, and the fields, and neither the date of the last
update nor the language driver; a `.cpg` names the layer's encoding where `ENCODING` does not.

## Decision

We will read shapefiles in `adapter-shapefile` with a reader of the adapter's own, derived from
MapServer's `src/mapshape.c` and `src/mapxbase.c` at the pinned release, with its guards, its
reading of a broken record as no shape, and its fields of the `.dbf` header, in checked
arithmetic, and with `.dbf` text decoded as UTF-8, an invalid byte replaced.

**Out of scope:** code pages, `.cpg` files, and a layer's `ENCODING`; the index files `.qix` and
`.sbn`; shapes with a measure or a height beyond reading their x and y; writing shapefiles;
whether a browser host binds the adapter.

## Alternatives considered

- **The `shapefile` and `dbase` crates** — the reader in wide use, MIT, wasm32-clean, and no panic
  over about 5,800 truncated and mutated inputs; but the size check casts the computed size to
  `i32` before it compares, so 2^28 points in a 48-byte record pass it and reserve 4 GiB; the
  parts are read and reserved before any check; `dbase::Reader::new` refuses a `.dbf` whose
  language driver names a code page and one whose update date is zero, both of which MapServer
  opens. The adapter would parse every record header again to guard the crate, and patch `.dbf`
  headers, which leaves little for the crate to do.
- **The crates' geometry with a `.dbf` reader of Forager's own** — the record guards are still the
  adapter's, and `dbase` still compiles in as a dependency of `shapefile`.
- **A reader written from the ESRI specification alone** — the same amount of code, but which
  records MapServer drops and which headers it accepts would be found only by failing cases.
- **Code pages honoured now** — MapServer reads a `.cpg` and converts by the layer's encoding, which
  needs an encoding table in a crate a browser may load (Q-5); it waits until a case or a user
  needs it.

## Sources / Prior art

- MapServer rel-8-6-6, `src/mapshape.c` (`msSHPOpen`, `msSHXReadSize`, `msSHPReadShape`,
  `msSHPReadPoint`, the `.cpg` read in `msShapefileOpen`) and `src/mapxbase.c` (`msDBFOpen`,
  `msDBFReadAttribute`, `msReadCPGEncoding`).
- ESRI, *ESRI Shapefile Technical Description*, July 1998,
  <https://www.esri.com/content/dam/esrisites/sitecore-archive/Files/Pdfs/library/whitepapers/pdfs/shapefile.pdf>.
- `shapefile` 0.9.0 and `dbase` 0.8.0, <https://github.com/tmontaigu/shapefile-rs> and
  <https://github.com/tmontaigu/dbase-rs>, checked 2026-10-25 in a scratch crate and in review:
  `src/record/polyline.rs` and `src/record/multipoint.rs` cast the size to `i32`;
  `src/record/io.rs` reads the parts before the check; `dbase`'s `src/file.rs` and `src/header.rs`
  choose the decoding from the language-driver byte and parse the update date.

## Consequences

- Positive: the adapter drops and keeps the records MapServer drops and keeps (Q-4); every count is
  checked before anything is allocated, so a record costs memory in proportion to its size in the
  file, and the data-file limit of Q-9 bounds a layer; no third-party crate in the adapter (Q-6).
- Negative / trade-offs: the reader is Forager's to maintain and to fuzz, and the files derived
  from MapServer's carry its notice (the licence constraint, `scripts/check-notice.sh`). Text that
  is not UTF-8 reads with replacement characters where MapServer would convert it by the `.cpg`.
- Follow-ups: the adapter as a target of the weekly fuzz run (Q-3); code pages and `ENCODING`, when
  a case or a user needs them; the size of the bundle (Q-5) once a browser host binds the adapter.

## Enforcement

A test of `adapter-shapefile` cites this ADR with `Verifies:` and reads: every shape type it
supports; a record past each of MapServer's guards, the counts of 2^28 points in a small record
among them, read as no shape while the next record is read; a `.shx` with a short header, an error
of the source; a `.dbf` with a field of each type, with a language driver that names a code page,
with a zero update date, and with a byte that is not UTF-8.
