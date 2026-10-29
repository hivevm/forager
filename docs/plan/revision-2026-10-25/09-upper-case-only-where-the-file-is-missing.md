# Ticket: Try a shapefile's upper-case extension only where the lower-case file cannot be opened

## What to build

The shapefile adapter loads `<name>.shp`, and where that load fails for any reason it loads
`<name>.SHP` (`load` in `crates/adapter-shapefile/src/lib.rs`), and so for `.shx` and `.dbf`.
MapServer retries the upper-case name only where the lower-case file cannot be opened
(`msSHPOpen`, `src/mapshape.c` l.420–436; `msDBFOpen`, `src/mapxbase.c` l.321–326). The two part
where the lower-case file exists but the loader refuses it: a `poly.shp` past the data-file limit
of Q-9 beside a valid `poly.SHP` draws `poly.SHP` here, where MapServer, which has no such limit,
reads `poly.shp`. Each file stays bounded, so nothing escapes a limit; the departure is
undocumented, and the layer draws data the mapfile did not name first.

The `ResourceLoader` port returns a `SourceError` without a kind, so the adapter cannot tell a
missing file from a refused one. Either the port's error grows a kind that says the file is not
there, which changes a port trait's types, or the departure is written down in
`docs/semantics/shapefile.md` and kept. Which one is the maintainer's.

## Acceptance criteria

- [ ] Given the maintainer's choice between a loader error kind and a documented departure, when
  it lands, then the adapter and `docs/semantics/shapefile.md` agree with it.
- [ ] Given the kind is chosen: given a `poly.shp` the loader refuses for its size beside a valid
  `poly.SHP`, when the source is opened, then it fails naming `poly.shp` and its limit, and
  `poly.SHP` is not loaded; given only `poly.SHP`, then it opens as before.
- [ ] Given the departure is chosen: the *Files* paragraph of `docs/semantics/shapefile.md` states
  it and why, and a test of `adapter-shapefile` pins the behaviour.

## Read first

- `load` and `ShapefileSource::open` in `crates/adapter-shapefile/src/lib.rs`; `ResourceLoader`
  and `SourceError` in `crates/ports/src/lib.rs`; the *Files* paragraph of
  `docs/semantics/shapefile.md`; [ADR-0024](../../adr/0024-shapefile-reader.md).

## Blocked by

- None (can start immediately)

## Mode

with the human: a loader error kind changes a port trait's types, which the harness keeps with
the maintainer ([ADR-0007](../../adr/0007-harness.md)).
