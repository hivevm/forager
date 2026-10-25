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
      and the unread type skipped.
- [ ] Given an extent outside the `.shx` header's bounds, one that contains them, and one that
      cuts through them, when the source is queried, then it selects the records MapServer's
      `msShapefileWhichShapes` selects, by each record's stored box.
- [ ] Given a record past each of MapServer's guards on counts, sizes, and parts, when the source
      is queried, then that record is read as no shape and the next record is read.
- [ ] Given each memory case of ADR-0024's Enforcement, when the source is queried in a test binary
      with a counting allocator, then the peak allocation stays under the stated multiple of the
      files' size.
- [ ] Given each header error MapServer's open functions raise for a `.shp`, `.shx`, or `.dbf`, and
      a missing `.dbf`, then the source returns an error; a `.dbf` whose update date is zero or
      whose language driver names a code page opens; nothing panics (Q-3).
- [ ] Given a record whose bounding box lies outside the `.shp`, when the source selects by boxes,
      then that record is no shape and the layer reads on, as ADR-0024 decides.
- [ ] A test cites ADR-0024 with a `Verifies:` marker, each rule of `docs/semantics/shapefile.md`
      names its test, and every file derived from MapServer's passes `scripts/check-notice.sh`.

## Read first

- ADR-0024 and the MapServer functions its Sources name, at the pin; ADR-0009 (an
  `adapter-<kind>` crate and its edges); the licence constraint and `scripts/check-notice.sh`.
- One of the pages under `docs/semantics/`, `color.md`, as the shape of the new page.
- Glossary: *adapter*, *port*, *resource loader*.
- Golden path: the GeoJSON adapter and its tests; the seam is the feature source port.
- What the reviews of ADR-0024 found in MapServer's reader, each to be checked against the source
  and written into the page:
  - the size formula `44 + 4 × parts + 16 × points` compares with a record's content length, which
    is the `.shx` size; MapServer's code adds the 8 bytes of the record header to both sides; a
    point record shorter than 28 bytes counts those 8 bytes too, a null shape is content of 4;
  - polygon and arc content under 52 bytes, negative counts, and a `.shx` size under 4 are no
    shape;
  - a multipoint with M or Z needs `16 + 8 × points` more than one without;
  - MapServer accepts the file codes `0x0000270a` and `0x0000270d`; it reads the `.shp` header's
    length but neither uses it to read records nor reads its shape type;
  - the `.shx` record count comes from its header's file length; a short read zeroes the whole
    index page, so entries the file holds on that page are lost too;
  - a `.dbf` shorter than 32 bytes or than its header length is an error; N and F fields take one
    byte of size and one of decimals, other types two bytes of size; fields may be zero wide;
  - a shape past the `.dbf`'s records still draws, without values; values are trimmed of trailing
    blanks, and N, F, and D values of leading ones too;
  - the parts of a shape are the lines of one MapServer shape, with no ring classification; how
    they map onto the port's geometries is decided here and written into the page.

## Blocked by

- docs/plan/conformance-harness/01-shapefile-crate-adr.md

## Mode

unattended: tests at the port decide every criterion; one new crate that nothing yet binds, undone
by one revert; it changes no port trait, no resource path resolution, no limit, nothing else
ADR-0007 keeps with the maintainer. A wrong result shows as a fixture feature read wrong. It counts
against the cap of three open agent branches of [ADR-0007](../../adr/0007-harness.md), and is
dispatched only while that cap allows. Stop when a criterion needs a change to the port traits, or
a rule of MapServer's that no fixture can reach, and hand off.
