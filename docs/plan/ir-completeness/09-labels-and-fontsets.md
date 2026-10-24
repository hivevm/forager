# Ticket: Carry labels, text expressions, and the FONTSET into the IR

## What to build

A class's `LABEL` blocks load into the IR with what placement and drawing need: `TEXT` (a string
expression of the ADR-0019 tree, with `tostring` and concatenation), the layer's `LABELITEM` as
the default text, `FONT` and `TYPE`, `SIZE` (incl. bindings), `COLOR`, `OUTLINECOLOR`,
`OUTLINEWIDTH`, `SHADOWCOLOR`, `SHADOWSIZE`, `POSITION`, `OFFSET`, `ANGLE` (incl. `AUTO`,
`FOLLOW`), `BUFFER`, `MINDISTANCE`, `MINFEATURESIZE`, `PARTIALS`, `FORCE`, `PRIORITY`, `WRAP`,
`MAXLENGTH`, `ALIGN`, `REPEATDISTANCE`, scale ranges, and the `STYLE` blocks inside a label,
loaded as ticket 07's styles. The `FONTSET` file loads through the resource loader inside the
base context (Q-8) as a table from font name to path; font files are not read. Writes the
`docs/semantics/` pages of the label parameters and `FONTSET`.

## Acceptance criteria

- [ ] Given `LABELITEM "name"` and a `LABEL` without `TEXT`, when it loads, then the label's text
  is `to_string` of the property `name`.
- [ ] Given `TEXT ("Area: " + tostring([area],"%.2f"))`, when it loads, then the text is `concat`
  of the literal and `tostring` of `to_number` of `area`.
- [ ] Given `FONTSET "fonts.txt"` and `FONT "sans"`, when it loads, then the IR holds the table
  and the label names `sans`; an unknown font is a warning.
- [ ] Given `FONTSET "../fonts.txt"`, when it loads without a base above it, then it is refused
  (citing Q-8).
- [ ] Given a label with a `STYLE` and each parameter above, when it loads, then each is in the IR
  and none is reported as unsupported, and the document round-trips through YAML and JSON.

## Read first

- [ADR-0019](../../adr/0019-cql2-expressions.md),
  [ADR-0014](../../adr/0014-style-document-serde.md);
  the *Label cache* glossary entry; Q-8.
- MapServer `rel-8-6-6`: `loadLabel` in `src/mapfile.c`, `msLoadFontSet` in `src/mapfile.c` or
  `src/fontcache.c`.
- Tickets 01 and 07's translations; ticket 08's reading of a file through the loader.

## Blocked by

- docs/plan/ir-completeness/07-styles-complete.md
- docs/plan/ir-completeness/08-symbols-and-symbolsets.md

## Mode

with the human: it changes the style document format and resolves resource paths, both kept with
the maintainer ([ADR-0007](../../adr/0007-harness.md)).
