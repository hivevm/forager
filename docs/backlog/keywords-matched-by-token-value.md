# Directive keywords, a CONNECTIONTYPE, and a VALIDATION pair are still read by their words

Observed in the review of the lexer ticket (revision-2026-10-24/28), which made a value's keyword
choice compare by token value (crates/mapfile/src/keywords.rs, docs/semantics/tokens.md). Three
readings still go by the word where MapServer's loader goes by the token:

- **A directive or block keyword.** MapServer's loaders switch on the lexer's token value, so two
  keywords of one value are one statement: `SIMPLE`, `MS_SYMBOL_SIMPLE` (1000), inside a `STYLE` is
  `ANGLE` (1000), and `POSTLABELCACHE` inside a `LABEL` is `FORCE` (both 1098). The parser
  (crates/mapfile/src/syntax.rs) matches statements by name.
- **`CONNECTIONTYPE`.** loadLayer reads it with `getSymbol(14, MS_OGR, …)`, so `small` (7) is
  `MS_WMS`; the frontend keeps the word (`translate/mod.rs`, the layer's source and the graticule
  check). At the pin `graticule` lexes as `GRATICULE` (1140), not `MS_GRATICULE`, so MapServer
  refuses `CONNECTIONTYPE graticule` itself.
- **A `VALIDATION` pair of a bare keyword.** loadHashTable reads both with `getString`, failing the
  load; the frontend drops the pair without a diagnostic (`translate/defaults.rs`), as it already
  dropped a pair with a number.

Each diverges only for mapfiles no corpus case holds.
