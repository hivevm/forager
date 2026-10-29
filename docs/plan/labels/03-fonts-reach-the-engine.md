# Ticket: Fonts reach the engine from the CLI and the browser

## What to build

The font files a mapfile's `FONTSET` names reach the engine as the text-stack ADR decides: the CLI
reads them from disk inside the base context, and the browser bindings take them from the page by
the path the font set names, as they take GeoJSON. A font that does not load, or exceeds the bound,
is a diagnostic naming the font and the path; nothing panics.

## Acceptance criteria

- [ ] Given a mapfile whose font set names a font beside it, when the CLI renders, then the font is
      read through its loader and handed to the engine.
- [ ] Given a page that passes a font under the path the font set names, when the bindings render,
      then the same font reaches the engine.
- [ ] Given a font path outside the base context, a missing font, or one past the size bound, then a
      diagnostic names it, and no label that needs it is drawn.

## Read first

- The ADR of ticket 01; ADR-0023; Q-8 and Q-9; the convention on readers that fail closed.
- The golden path of `forager render` and the bindings' `render`; their tests are the seams.

## Blocked by

- docs/plan/labels/01-text-stack-adr.md

## Mode

with the human: the bound on a font file's size is a resource limit, which ADR-0007 keeps with the
maintainer.
