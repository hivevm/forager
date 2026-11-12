# Ticket: Report a label left out for its style's transform once, and say what goes with it

## What to build

A label's style with a `GEOMTRANSFORM` the frontend does not carry over leaves the label out, and
its class's other labels with it (b35e2b0). The frontend says so twice and differently: the style's
reader warns `unsupported-value` that "a label's leaves its label out"
(`crates/mapfile/src/translate/style.rs:215-216`), and the label's reader warns `label-not-drawn`
(`translate/label.rs:174-178`). `docs/semantics/style-symbol.md:41-42` says the label is left out
and not that its class's other labels go too; `docs/semantics/label.md:93` says both. PR #176 named
the stale text. Report one diagnostic for the one cause, which names the class's other labels, and
correct `style-symbol.md`.

## Acceptance criteria

- [ ] Given a `LABEL` whose `STYLE` has `GEOMTRANSFORM "foo"`, when it loads, then exactly one
  diagnostic names it, and its message says the class's other labels are left out with it.
- [ ] Given a class's `STYLE` with `GEOMTRANSFORM "foo"`, when it loads, then its warning is as at
  the base.

## Read first

- Touches: `crates/mapfile/src/translate/style.rs`, `crates/mapfile/src/translate/label.rs`,
  `docs/semantics/style-symbol.md`
- `docs/semantics/label.md`; `docs/CONVENTIONS.md` (*Diagnostic codes are kebab-case and stable*).
- Overlap: `docs/plan/maxfeatures-and-layer-geomtransform/01-maxfeatures-and-layer-geomtransform.md`
  and `docs/plan/geomtransform-bbox/01-geomtransform-bbox.md` change which transforms are carried
  over; whichever lands second keeps this one diagnostic.

## Blocked by

- None (can start immediately)

## Mode

unattended: the tests above decide it; one revert; nothing ADR-0006 keeps with the human. Stop if
a test of another diagnostic would have to change.
