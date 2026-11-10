# Ticket: Propose the ADR that admits a vendored font into the browser example

## What to build

f8fb03e vendored DejaVu Sans (`examples/web/vendor/dejavu/DejaVuSans.ttf` and its `LICENSE`) for
the labels of the browser example, listed in `examples/web/vendor/SHA256SUMS` and checked by
`scripts/check-vendor.sh`, whose header now says it checks "its JavaScript and font", citing
[ADR-0021](../../adr/0021-vendored-leaflet.md). ADR-0021 decides "every third-party script and
stylesheet" of the example, and names Leaflet; it says nothing of a font, of where the copy comes
from, or which release, and the font's licence (Bitstream Vera's, with DejaVu's changes in the
public domain) is not the BSD-2-Clause it weighed. A reader of ADR-0021 would take the font for a
mistake, so the decision is to be recorded ([`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules),
rule 2).

Draft with the maintainer, through `propose-adr`, an ADR that supersedes ADR-0021 with the same
rule for every third-party file of the example, scripts, stylesheets, and fonts, and names the
font's source, its release, and its licence; or one that admits the font beside ADR-0021. The check
stays as it is unless the ADR asks more of it.

## Acceptance criteria

- [ ] Given the draft, when the maintainer reads it, then it names DejaVu Sans's release, the URL
  the copy was taken from, its checksum as published, and its licence, in an ADR-only pull request.
- [ ] Given `scripts/check-vendor.sh`, when it runs after the ADR lands, then its header cites the
  ADR in force for the font.

## Read first

- [ADR-0021](../../adr/0021-vendored-leaflet.md), [ADR-0012](../../adr/0012-crate-supply-chain.md)
  (Q-6 for crates); Q-6 in [`docs/SPECIFICATION.md`](../../SPECIFICATION.md);
  `scripts/check-vendor.sh`, `examples/web/vendor/`.

## Blocked by

- None (can start immediately)

## Mode

with the human: it is an ADR, and only the maintainer accepts it.

## Maintainer's decision (2026-11-07)

An ADR that supersedes ADR-0021 with the same rule for every third-party file of the browser
example, scripts, stylesheets, and fonts, naming DejaVu Sans's source, release, checksum, and
licence.

## Maintainer's go-ahead (2026-11-08)

The agent drafts the ADR as Proposed; the maintainer sets it Accepted, and any ADR it supersedes to
Superseded.
