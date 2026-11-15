# Ticket: The cases of a non-goal leave the scope

## What to build

The conformance list stops counting cases that the specification leaves out. Each such case leaves
the scope with a reason that names its non-goal. The list's header says that a non-goal is a reason.
One new non-goal, SVG symbols, is drafted for the maintainer to land.

The cases, each now `geometry fails` in `crates/cli/tests/conformance/cases.txt`:

| Case | Non-goal | Line of *Non-Goals* |
|------|----------|---------------------|
| `misc/grid_without_label.map grid_without_label.png` | a layer's `GRID` | the sources line |
| `renderers/chartlayer_bar`, `_pie`, `_pie2`, `_pie_expressions`, `_vbar` (5) | `TYPE CHART` | "A layer's `TYPE` of `CHART` …" |
| `renderers/point_pixmap.map point_pixmap.png` | a symbol fetched by its URL | the pixmap line |
| `renderers/point_pixmap_gif.map point_pixmap_gif.png` | pixmaps other than PNG | the pixmap line |
| `renderers/point_svg.map point_svg.png`, `renderers/line_marker_svg.map line_marker_svg.png` | SVG symbols | drafted here |
| `misc/encoding.map encodings.png` | see below | none yet |

**`misc/encoding.map` is not covered by the non-goal its note implies.** The *Non-Goals* name
`ENCODING` in the line of a label's features ("Bitmap fonts, a label's shadow, `LEADER`, `ENCODING`,
…"), which is `LABEL ENCODING` (`label->encoding`, `src/mapfile.c:1793`). The case uses a
**layer's** `ENCODING "latin1"` (`layer->encoding`, `src/mapfile.c:4539`). That is the character set
of the shapefile's `.dbf`, which `src/mapshape.c` transcodes to UTF-8 before a class's expression
`"éà toto"` compares it. The case has no label. The session drafts one of these for the maintainer
to decide, and moves the case only if the maintainer lands the first:

- a layer's `ENCODING` added to the sources line of *Non-Goals* ("a layer's `GRID`, `ENCODING`,
  `CONNECTIONOPTIONS`, and `BINDVALS`"). That change names ADR-0021 and ADR-0023, the shapefile
  reader and its typed properties, as ADRs whose basis it moves;
- the case kept in scope as failing, with its note corrected to "the ENCODING of a layer, the
  character set of its .dbf", and a backlog entry for the transcoding.

The drafted line for SVG symbols, in the pixmap line of *Non-Goals*:

> - Pixmap symbols in formats other than PNG, SVG symbols (`TYPE svg`, and a symbol named by a file
>   name ending in `.svg`), a symbol of `TYPE simple`, a symbol fetched by its URL, and a pixmap
>   tiled along a line without a `GAP`.

The change names the accepted ADRs whose basis it moves. ADR-0029 holds a decoded pixmap and names
the decoders of `crates/encode`. The session reads it and says whether SVG touches it, and names no
ADR without checking. Q-3's fuzz target "the decoder of symbol images" is unchanged.

The header of the list, lines 24 to 30, names the reasons for leaving a case out as "their
categories' exclusions". The session adds a non-goal of the specification, named in the reason, as a
kind of reason. The harness (`list` in `crates/cli/tests/conformance/mod.rs`) reads a reason after
`out:` as free text and refuses only an empty one, so the harness does not change. The ten cases of
`SCALETOKEN` give "a runtime substitution by scale", which the header already names. The
reasons follow the list's form, such as
`out: a non-goal, a layer's TYPE CHART`.

Moving failing cases out raises the share of Q-4 without a change in what Forager draws.
`crates/cli/tests/conformance/fidelity.txt` records no release yet, so `scripts/check-fidelity.sh`
holds no earlier share. The hand-off states the new counts.

## Acceptance criteria

- [ ] Given the list, when the harness reads it, then each case of the table except
  `misc/encoding.map` is `out:` with a reason that names its non-goal, and the harness's check of
  the list against the suite passes.
- [ ] Given the header, when it is read, then it names a non-goal of the specification as a reason
  for leaving a case out.
- [ ] Given the drafted specification change, when the maintainer reads it, then it adds SVG symbols
  to the pixmap line of *Non-Goals*, offers the choice on a layer's `ENCODING`, and names the ADRs
  whose basis it moves.
- [ ] Given `misc/encoding.map`, when the maintainer has decided, then it is out of scope with the
  non-goal or in scope as failing with a corrected note.
- [ ] Given every other case in scope, when the harness renders them natively and on wasm32, then
  each holds its outcome.

## Read first

- Touches: `crates/cli/tests/conformance/cases.txt` (its header and the lines of the table),
  `docs/SPECIFICATION.md` (*Non-Goals*)
- `crates/cli/tests/conformance/mod.rs` (`list`, `unlisted`, `Tally`),
  `crates/cli/tests/conformance/fidelity.txt`, `scripts/check-fidelity.sh`
- MapServer at the pin: `src/mapfile.c` (`case (ENCODING)` in `loadLabel` and in `loadLayer`),
  `src/mapshape.c` (the `.dbf`'s encoding)
- The mapfiles of the cases in the fetched suite (`scripts/fetch-msautotest.sh` names where)
- `docs/adr/0021-shapefile-adapter.md`, `docs/adr/0023-typed-properties.md`,
  `docs/adr/0029-pixmaps-held-once-per-render.md`
- Seam: the msautotest harness (`crates/cli/tests/msautotest.rs`, run by `scripts/check-rust.sh`)

## Blocked by

- None (can start immediately)

## Mode

with the human: the SVG non-goal, and a layer's `ENCODING` if chosen, are changes to the
specification, which the human lands ([`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules) rule 7). The
moves of cases already covered by a non-goal could run unattended. The session moves those cases and
edits the header in one commit, drafts the specification change in another, and stops at the
hand-off. `encoding.map` moves only after the maintainer has decided.
