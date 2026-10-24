# Semantics of the mapfile parameters

What each mapfile parameter the style IR carries means, which behaviour of MapServer it
reproduces, with the source it follows, and how the mapfile frontend resolves MapServer's quirks
on the way in. One page per parameter; the reference is MapServer release 8.6.6, the release the
specification pins. These pages describe and decide nothing: the decisions are the ADRs they
cite.

- [`CLASSITEM`](classitem.md)
- [`EXPRESSION`](expression.md) — and the expression language every parameter below shares
- [`FILTER` and `FILTERITEM`](filter.md)
