# Ticket: Let a label read an inline feature's TEXT from the feature the port delivers

## What to build

[ADR-0033](../../adr/0033-inline-features-in-the-style-ir.md) makes a layer's inline features its
source, so that "the engine keeps one way of reading features, the port, whatever holds them"
(its *Consequences*). The labels that took up the feature's `TEXT` later (00994ba) do not read it
from the feature the port delivers: `feature_text` in `crates/engine/src/inline.rs` goes back into
`layer.features` of the style IR and takes the feature at the place its id names, and
`crates/engine/src/labels.rs` calls it twice, for the classes that draw a feature with a text of
its own and for a label without a text. It holds only because `InlineSource` numbers its features
by their place in the layer, a rule of the source that the labels now depend on without the port
saying so; the port's `Feature` (`crates/ports`) carries an id, a geometry, and properties, and no
text.

Decide with the maintainer how the text reaches the labels, then build it. The candidates seen:
the port's `Feature` carries a shape's text, `None` from every adapter but the inline source,
which changes the port of [ADR-0026](../../adr/0026-feature-properties-at-the-port.md); the inline
source delivers the text as a property under a name no `ITEMS` can give, which leaks into
expressions; or the side channel stays, with the overview naming it and a test that fails when
the inline source numbers its features otherwise.

## Acceptance criteria

- [ ] Given the maintainer's choice, when it changes the port, then a `proposed` ADR records it in
  a pull request of its own before the code changes.
- [ ] Given a point layer with inline features, some with a `TEXT`, and a class with a label
  without a text, when it renders, then each label draws its feature's text exactly as today,
  pinned by `crates/engine/tests/labels.rs` or `crates/engine/tests/inline.rs`, and the
  conformance outcome of the text cases does not move.
- [ ] Given the change, when `crates/engine/src` is searched, then no label reads
  `layer.features` of the style IR, or, where the side channel stays, a test decides that a
  feature's id is its place and `docs/ARCHITECTURE.md` names the channel.

## Read first

- [ADR-0033](../../adr/0033-inline-features-in-the-style-ir.md),
  [ADR-0026](../../adr/0026-feature-properties-at-the-port.md),
  [ADR-0009](../../adr/0009-ports-and-adapters-crates.md); `crates/engine/src/inline.rs`
  (`feature_text`, `InlineSource`), the two calls in `crates/engine/src/labels.rs`, `Feature` in
  `crates/ports/src/lib.rs`; `docs/semantics/feature.md`.

## Blocked by

- None (can start immediately)

## Mode

with the human: two candidates change the port's `Feature` or what a feature's properties hold,
a public interface [ADR-0007](../../adr/0007-harness.md) keeps with the maintainer, and the third
is the maintainer's to accept.
