# Semantics of the mapfile parameters

What each mapfile parameter the style IR carries means, which behaviour of MapServer it
reproduces, with the source it follows, and how the mapfile frontend resolves MapServer's quirks
on the way in. One page per parameter; the reference is the MapServer release the specification
pins, named in [`scripts/fetch-msautotest.sh`](../../scripts/fetch-msautotest.sh). These pages
describe and decide nothing: the decisions are the ADRs they cite.

- [`CLASSITEM`](classitem.md)
- [`EXPRESSION`](expression.md) — and the expression language every parameter below shares
- [`FILTER` and `FILTERITEM`](filter.md)
- [Attribute bindings `[name]`](binding.md)
- [`COLOR` and `OUTLINECOLOR`](color.md)
- [`SIZE`](size.md), [`WIDTH`](width.md), [`ANGLE`](angle.md) of a style
