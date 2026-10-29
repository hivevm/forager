# ADR-0026: A feature carries its properties as typed values, a shapefile's as its record in a `.dbf` its layer shares

- **Status:** 🟢 accepted
- **Date:** 2026-10-29
- **Deciders:** Markus Brigl
- **Applies to:** the `Feature` type of `crates/ports`, `crates/adapter-geojson`, `crates/adapter-shapefile`, and every `adapter-<kind>` crate that delivers features

## Context

A feature as the port delivers it has an id and a geometry and nothing else, so no class
`EXPRESSION`, no binding such as `SIZE [attr]`, and no label `TEXT` can read what the data says
about it. The expressions are decided: [ADR-0019](0019-cql2-expressions.md) wraps every property
an expression reads in `to_number`, `to_string`, or `to_timestamp`, so that the style IR means the
same whatever type a source delivers. What a source delivers, and what it costs, is open, and
[ADR-0024](0024-shapefile-reader.md) left it so.

Four forces bound the answer. The two sources differ in kind: a GeoJSON property is a JSON value,
typed, and may be `null`, an array, or an object, while MapServer reads every `.dbf` field as a
string (`msDBFReadAttribute`) and types it only by the syntax of the expression that reads it.
Memory must follow the files' size (Q-9): ADR-0024 requires that a feature's properties cost
memory in proportion to its record in the `.dbf`, beside a field table its layer shares, and a
`.dbf` header can declare thousands of zero-wide fields over a record of a few bytes, so any value
kept per field and record multiplies the file. The values must read as MapServer reads them, for
the reference images to agree (Q-4): `msDBFReadAttribute` copies a field up to its first NUL byte,
trims trailing blanks, trims leading blanks of an `N`, `F`, or `D` field, and reads a null `N`,
`F`, or `D` field as `"0"`; `msDBFGetItemIndex` finds a field by its name without regard to ASCII
case, the first match winning. And the port is a public interface, implemented by every adapter
and the fuzz targets and read by the engine, which the maintainer keeps
([ADR-0007](0007-harness.md)).

## Decision

We will have a feature carry its properties as values of the port's own type — null, boolean,
number kept as its text, or string — looked up by name, where a GeoJSON feature holds its
properties as a list of its own and a shapefile feature names its record in the `.dbf` its layer
shares with its field table, a value decoded only when a property is read.

- **Values.** A number keeps its text, so that a reader that wants a string gets the digits the
  source delivered and one that wants a number parses them. A GeoJSON number's text is the one
  `serde_json` prints for the number it parsed: a 64-bit integer exact, any other number in the
  shortest form that reads back as the same `f64` (`1.50` reads `1.5`). A GeoJSON array or object
  is its JSON text, a string. A `.dbf` value is always a string, as `msDBFReadAttribute` reads it,
  decoded as UTF-8 with an invalid byte replaced (ADR-0024); a `.dbf` never yields null.
- **Storage.** A shapefile layer keeps its `.dbf` and a field table of each field's place in the
  header, its type, its offset, and its width, once for the layer; a feature holds the number of
  its record and a shared reference to them, and nothing per field. A GeoJSON feature holds its
  names and values in a list its clones share, which cost what its text cost.
- **Lookup.** A shapefile feature finds a property by name without regard to ASCII case, the first
  field winning, as `msDBFGetItemIndex` does; a GeoJSON feature finds it by the exact key. Each
  source decides its lookup, since a JSON object can hold keys that differ only in case.
- **Absence.** A feature whose shape lies past the `.dbf`'s records carries no properties, as
  MapServer's shape carries no values there, and a name a feature does not hold reads as absent,
  which is not null.

**Out of scope:** what an expression, a binding, or a label does with an absent property, which
the tickets that read properties decide against MapServer's behaviour; a layer's `ENCODING` and
`.cpg` files, which ADR-0024 leaves out; the properties of other sources; a number's source text
where `serde_json` does not keep it.

## Alternatives considered

- **Every value a string** — as MapServer holds its items, and enough for ADR-0019's conversions,
  but a GeoJSON `true`, `null`, or `1` would read as text the source never wrote, and a reader
  could not tell a null from the string `"null"`.
- **A number as `f64`** — the simplest typed form, but a reader that wants the string loses the
  digits of an integer past 2^53 and the form the source wrote.
- **A number's exact source text, through `serde_json`'s `arbitrary_precision`** — keeps `1.50` as
  written, but Cargo unifies the feature across the workspace, so it would change how `style-ir`,
  `mapfile`, `cli`, and `bindings-wasm` read every number; the port's text form leaves the
  change to the adapter if a case ever asks for it.
- **Decoded values per feature** — one owned string per field and record is simpler to read, but
  a `.dbf` of many zero-wide fields costs the field count times the size of a value per record,
  which breaks ADR-0024's bound.
- **A lookup without regard to case everywhere** — one rule for every source, but a GeoJSON object
  with `Name` and `name` would hide one of them; MapServer reads GeoJSON only through OGR, which is
  not the reference here.
- **The properties beside the features, as a table the query returns** — saves the shared
  reference per feature, but every reader of a feature would need the table too, and features
  could no longer be passed on alone.

## Sources / Prior art

- MapServer rel-8-6-6, `src/mapxbase.c`: `msDBFReadAttribute` (the copy, the trims, and the null
  `"0"`), `DBFIsValueNULL` (`*` for `N` and `F`, `00000000` for `D`, `?` for `L`, empty for the
  rest), `msDBFGetFieldInfo` (the name: 11 bytes, NUL-terminated, trailing blanks trimmed),
  `msDBFGetItemIndex` (`strcasecmp`, the first match).
- MapServer rel-8-6-6, `src/mapshape.c`: `msSHPLayerNextShape` sets `numvalues` to 0 where
  `msDBFGetValueList` finds no record, and returns the shape.
- `serde_json` 1.0.154: a parsed number prints as `1.5` for `1.50`, `100.0` for `1e2`, and exactly
  for integers within 64 bits; one past them prints as an `f64` (checked with a scratch program).
- RFC 8259, section 4 (https://www.rfc-editor.org/rfc/rfc8259#section-4): the names of a JSON
  object should be unique, and their comparison is not defined to ignore case.

## Consequences

- Positive: expressions, bindings, and labels can read what the data holds (Q-4); a shapefile's
  properties cost its `.dbf` and one table per layer and a record number per feature, whatever
  its header declares (Q-9); the trims and the null reading live in one place, the table's reader.
- Negative / trade-offs: every implementation of `FeatureSource` builds features with properties,
  the fuzz targets' included; a property is decoded and trimmed each time it is read, which costs
  time where an expression reads it per feature; the port's value type grows by a borrowed form,
  since a shapefile's string is decoded on read.
- Follow-ups: whether a property an expression names but the `.dbf` lacks fails the layer, as
  `msDBFGetItemIndexes` makes MapServer's layer fail; how a browser host delivers properties of
  its own features.

## Enforcement

Tests cite this ADR with `Verifies:`: one of `adapter-geojson` queries a feature with a string, a
number, a boolean, and a null property and expects each of its kind; one of `adapter-shapefile`
reads a `.dbf` with a field of each type MapServer reads and expects each value as
`msDBFReadAttribute` reads it, a shape past the records without properties, and a name found
without regard to case; and one of `adapter-shapefile`, under the counting allocator, opens a
`.dbf` of many zero-wide fields over many records and expects the peak allocation under four
times the files' size plus one mebibyte.
