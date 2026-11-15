# Ticket: A style's `ANTIALIAS FALSE` draws aliased lines

## What to build

A line style with `ANTIALIAS FALSE` is stroked without anti-aliasing, as MapServer strokes it.
`loadStyle` (`src/mapfile.c:2777-2783`) sets `style->antialiased` false for `ANTIALIAS` with the
value false. `msDrawLineSymbol` (`src/maprendering.c:600`, the copy at `:646`) carries it into the
stroke. `agg2RenderLine` (`src/mapagg.cpp:176`, the branches at `:183` and `:241`) then renders the
stroke's scanlines with AGG's aliased solid renderer instead of the anti-aliased one. A label's
`ANTIALIAS` is ignored by MapServer and stays ignored.

The style IR gets an optional field for a style's anti-aliasing, absent for the default (true). The
version rule of ADR-0014 allows that as a minor change. No release tag holds a schema yet, so the
change regenerates `schema/style-document-1.1.json`, unless the maintainer moves the version. The
renderer's stroke carries the flag to `render-agg`. The session checks whether agg-rust offers AGG's
aliased scanline renderer (`renderer_scanline_bin_solid`), or whether a port next to the blender of
ADR-0026 is needed. If a port is needed, it says so before writing it.

Case: `renderers/line_aliased.map line_aliased.png`: `testlines` stroked at width 5 with
`ANTIALIAS FALSE`.

## Acceptance criteria

- [ ] Given `renderers/line_aliased.map`, when the harness renders `line_aliased.png`, then it
  passes by the metric of G-6, natively and bit-identically on wasm32 (G-9), and the list says
  `geometry passes`.
- [ ] Given a style of `ANTIALIAS` with a word `getSymbol(2, MS_TRUE, MS_FALSE)` reads as false, as
  the frontend's `keywords::chosen` reads a token's value, when the frontend loads it, then the
  style IR holds anti-aliasing off, and the style document round-trips (G-3). A style of
  `ANTIALIAS TRUE` or none holds no field.
- [ ] Given an aliased stroke, when `render-agg` draws it, then every pixel it touches has the full
  coverage of the style's colour or none. A unit test of `render-agg` decides it.
- [ ] Given the style IR, when the schema test runs, then the checked-in schema matches the derived
  one.
- [ ] Given every case in scope, when they are rendered, then each holds its outcome.

## Read first

- Touches: `crates/style-ir/src/model.rs` (`Style`), `schema/style-document-1.1.json`,
  `crates/mapfile/src/translate/style.rs`, `crates/symbolize/src/line.rs`,
  `crates/render/src/lib.rs` (the stroke), `crates/render-agg/src/lib.rs`,
  `crates/cli/tests/conformance/cases.txt`
- MapServer at the pin: `src/mapfile.c` (`loadStyle`'s `ANTIALIAS`), `src/maprendering.c`
  (`msDrawLineSymbol`), `src/mapagg.cpp` (`agg2RenderLine`)
- `docs/semantics/line.md`
- `docs/adr/0014-style-document-versions.md`, `docs/adr/0025-agg-rust-rasterizer.md`,
  `docs/adr/0026-agg-blender-and-resampler.md`
- Golden path: another optional field of a style, such as `LINEJOINMAXSIZE`, from frontend to
  renderer, with its tests
- Seam: the msautotest harness; the frontend's `load` with files in memory; `render-agg`'s unit
  tests; `crates/style-ir/tests/schema.rs`

## Blocked by

- None (can start immediately)

## Mode

with the human: it changes the style document, which ADR-0006 never leaves unattended. The session
stops before it commits the field and the schema, and asks the maintainer whether it is a minor
change of 1.1 or a version move. It also stops if `render-agg` needs a port of AGG's aliased
renderer outside ADR-0026's scope, and when the case still fails after two attempts at its remaining
difference.
