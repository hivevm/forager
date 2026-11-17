# A TrueType marker costs one point of the marker budget, however long its glyph's path

**Observed:** ticket revision-2026-11-16/07 (2026-11-17), with the maintainer. Every marker is
priced against `MAX_MARKER_POINTS` by what it draws at one place (`MarkerBudget::price`): a
vector its outline's points, an ellipse its vertices, and a pixmap and a glyph one point each, as
each is one operation the price counts as one. A pixmap's cost does not grow with a point count,
but a glyph's does: the renderer flattens and fills its path, and a font's glyph can hold
thousands of commands. A million glyphs of such a font pass the budget as a million points. The
maintainer kept the price at one point when 07 moved the marks into one dispatch, so that the
refactor changed nothing a test sees.

**Where:** `crates/symbolize/src/line_marker.rs` (`MarkerBudget::price`, `points`),
`crates/symbolize/src/mark.rs` (`Styled::price`); Q-9 of `docs/SPECIFICATION.md`.

**What a change weighs:**

- **Price a glyph by its path's commands**, as a vector is priced by its points: the budget then
  bounds the renderer's work on a font's glyphs too. Fewer glyphs of a complex font would be
  drawn before the budget is spent, which no reference case reaches today; a resource limit's
  count changes, which ADR-0006 keeps with the human.
- **Keep one point:** no work. A glyph's path is bounded only by its font's file, which a render
  loads up to `MAX_FONT_OR_IMAGE_FILE_BYTES` (`crates/engine/src/resource.rs`, Q-9).
