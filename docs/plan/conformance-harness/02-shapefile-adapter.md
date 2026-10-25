# Ticket: Read shapefile geometry at the feature source port

## What to build

A new crate `adapter-shapefile` with a reader derived from MapServer's `src/mapshape.c` and
`src/mapxbase.c`, as [ADR-0024](../../adr/0024-shapefile-reader.md) decides: a feature source that
reads a `.shp` with its `.shx` and `.dbf` through the resource loader and yields the features an
extent selects, with their geometry, in the shape the GeoJSON adapter yields them. The `.dbf` is
opened and checked as MapServer opens it, but its attributes do not reach the port yet; that is
ticket 12. The reading goes into `docs/semantics/shapefile.md`, each rule with the MapServer
function it follows and the test that holds it. Implemented while ADR-0024 is `proposed`
([`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules), rule 4).

## Acceptance criteria

- [ ] Given fixture shapefiles of each shape type MapServer reads (1, 3, 5, 8, 11, 13, 15, 18, 21,
      23, 25, 28, x and y only), a null shape, and a type it does not read, when the source is
      queried with an extent that contains them, then it returns their features, the null shape
      and the unread type skipped: a point as a `Point`, a multipoint as a `MultiPoint`, an arc as
      a `LineString` of one part or a `MultiLineString` of more, a polygon as one `Polygon` whose
      rings are all its parts in order, as the reference renderer fills them even-odd.
- [ ] Given an extent outside the `.shx` header's bounds, one that contains them, and one that
      cuts through them, when the source is queried, then it selects the records MapServer's
      `msShapefileWhichShapes` selects, by each record's stored box.
- [ ] Given a record past each of MapServer's guards on counts, sizes, and parts, when the source
      is queried, then that record is read as no shape and the next record is read.
- [ ] Given each geometry memory case of ADR-0024's Enforcement, when the source is queried in a
      test binary with a counting allocator, then the peak allocation stays under four times the
      files' size plus one mebibyte.
- [ ] Given each header error MapServer's open functions raise for a `.shp`, `.shx`, or `.dbf`, and
      a missing `.dbf`, then the source returns an error; a `.dbf` whose update date is zero or
      whose language driver names a code page opens; no fixture panics.
- [ ] Given each case of ADR-0024's departure — a `.shx` size of 0 to 3, an offset of 0 or past the
      `.shp`, a box cut short, a least x that is not a number — when the source selects by boxes,
      then that record is no shape and the layer reads on.
- [ ] Given the Rust checks, then they build `adapter-shapefile` for `wasm32-unknown-unknown`.
- [ ] A test cites ADR-0024 with a `Verifies:` marker, each rule of `docs/semantics/shapefile.md`
      names its test, and every file derived from MapServer's passes `scripts/check-notice.sh`.

## Read first

- ADR-0024 and the MapServer functions its Sources name, at the pin; ADR-0009 (an
  `adapter-<kind>` crate and its edges); the licence constraint and `scripts/check-notice.sh`.
- One of the pages under `docs/semantics/`, `color.md`, as the shape of the new page.
- Glossary: *adapter*, *port*, *resource loader*.
- Golden path: the GeoJSON adapter and its tests; the seam is the feature source port.
- What the reviews of ADR-0024 found in MapServer's reader, each to be checked against the source
  and written into the page; the list is a start, not the whole — every rule comes from the source:
  - the size formula `44 + 4 × parts + 16 × points` compares with a record's content length, which
    is the `.shx` size; MapServer's code adds the 8 bytes of the record header to both sides; a
    polygon or arc of content under 44 bytes, a multipoint under 40, a point under 20, and a
    negative count are no shape, a null shape is content of 4, a `.shx` size under 4 is no shape;
  - the caps of 50,000,000 points and 10,000,000 parts; a part that starts below 0, ends past the
    points, or holds none; points are read in sequence from 0, not from each part's start;
  - a multipoint with M or Z needs `16 + 8 × points` more than one without;
  - only the `.shx` file code is checked, and MapServer accepts `0x0000270a` and `0x0000270d`; the
    `.shp` file code, its shape type, and each record's own type and header are not read; the
    file's shape type is one byte of the `.shx`;
  - the `.shx` record count comes from its header's file length, an error under 50 or past
    256,000,000; a short read zeroes the whole index page, so entries the file holds on that page
    are lost too;
  - a `.dbf` shorter than 32 bytes or than its header length, a header length of 32 or less, and
    fields that run past the record length are errors; a record count whose high byte is 128 or
    more reads as 0; N and F fields take one byte of size and one of decimals, other types two
    bytes of size; fields may be zero wide;
  - a shape past the `.dbf`'s records is still returned, without values; values are trimmed of
    trailing blanks, N, F, and D values of leading ones too, an empty N, F, or D value reads as
    `0`, and a value ends at a NUL byte;
  - for an extent that contains the file's bounds every record is selected, for one outside them
    none; a point file is selected by the point itself;
  - `msSHPOpen` and `msShapefileOpen` try `.SHP`, `.SHX`, and `.DBF` where the lower-case names are
    missing; which sibling names the adapter asks the resource loader for is written into the page,
    and how the host resolves them is ticket 03's.

## Blocked by

- docs/plan/conformance-harness/01-shapefile-crate-adr.md

## Mode

unattended: tests at the port decide every criterion; one new crate that nothing yet binds, undone
by one revert; it changes no port trait, no resource path resolution, no limit, nothing else
ADR-0007 keeps with the maintainer. A wrong result shows as a fixture feature read wrong. It counts
against the cap of three open agent branches of [ADR-0007](../../adr/0007-harness.md), and is
dispatched only while that cap allows. Stop when a criterion needs a change to the port traits, or
a rule of MapServer's that no fixture can reach, and hand off.
