# Ticket: The lines MapServer cuts with GEOS, decided and stated

## What to build

MapServer cuts a line shape with GEOS where it crosses the dateline from a polar source into
longitude and latitude (`msProjectGetLineCuttingCase`, `src/mapproject.c`). Forager does not port
it. The engine draws such lines uncut and warns `lines-not-cut`
(`crates/engine/src/lib.rs:718-730`). Three things are open:

- `Reprojection::cuts_lines_with_geos` (`crates/proj/src/shape.rs:222-225`) says that "a target
  written `init=epsg:3857`" is a case no projection here takes. That is false. MapServer tests
  `out->args[0]` for the text `init=epsg:3857` (`mapproject.c:1063-1065` at the pin), and
  `adapter-epsg` resolves that code. The frontend expands `init=` at load, so the fact is gone
  before `proj` sees it. A polar layer drawn into such a map is neither cut nor warned of.
- `docs/semantics/projection.md` does not name the `lines-not-cut` divergence.
- Whether GEOS's cut is ever ported, or the warning stays the answer. A port takes a dependency
  or a geometry algorithm, which is a decision for `propose-adr`.

With the maintainer: decide whether the IR carries "the map's projection was written
`init=epsg:3857`" so that the warning fires, or whether the divergence is stated instead. Then
correct the doc comment, and add the divergence to `docs/semantics/projection.md`.

## Acceptance criteria

- [ ] Given `shape.rs`, when the change lands, then its doc comment states each case of
  `msProjectGetLineCuttingCase` as the C reads at the pin, with the case Forager cannot see named.
- [ ] Given `docs/semantics/projection.md`, when the change lands, then it names the lines
  MapServer cuts with GEOS, the warning Forager gives instead, and its condition.
- [ ] Given the maintainer's choice to warn for a map written `init=epsg:3857`, when a polar layer
  with lines is rendered into it, then the engine's test sees `lines-not-cut`.

## Read first

- Touches: `crates/proj/src/shape.rs`, `crates/engine/src/lib.rs`, `docs/semantics/projection.md`,
  possibly `crates/style-ir/src/model.rs` and `crates/mapfile/src/translate/`
- `docs/adr/0019-projection-resolver.md`, `docs/adr/0037-reprojection-ported-in-proj.md`; the
  pinned `src/mapproject.c`, `msProjectGetLineCuttingCase`.

## Blocked by

None (can start immediately)

## Mode

with the human: a field of the style IR, or a stated divergence from MapServer, is the maintainer's
choice, and a port of GEOS's cut is a decision for an ADR.
