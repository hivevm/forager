# ADR-0045: The browser example loads every third-party file, its scripts, stylesheets, and fonts, only from copies vendored in the tree and checked by checksum

- **Status:** 🟢 accepted
- **Date:** 2026-11-10
- **Deciders:** Markus Brigl
- **Applies to:** `examples/web/`, its `vendor/` directory, every third-party script, stylesheet, and font the example loads, and `scripts/check-vendor.sh`
- **Supersedes:** [ADR-0021](0021-vendored-leaflet.md)

## Context

[ADR-0021](0021-vendored-leaflet.md) decided that the browser example loads every third-party
script and stylesheet from a copy under `examples/web/vendor/`, byte for byte as its release
published it and with its licence, checked against `examples/web/vendor/SHA256SUMS`, and named
Leaflet 1.9.4. Its reasons stand: no page load reaches a third-party origin, the copy is pinned and
reviewed, and the licence is permissive (Q-6).

Since 2026-11-07 the example draws labels, and the bindings take the fonts a mapfile names among
the files a page passes; the browser has no font file of its own to hand in. The example vendors
DejaVu Sans for them, `examples/web/vendor/dejavu/DejaVuSans.ttf` with its `LICENSE`, listed in
`SHA256SUMS` and checked by `scripts/check-vendor.sh`. ADR-0021 says nothing of a font, of where
the copy comes from, or which release, and the font's licence is not the BSD-2-Clause it weighed;
a reader of it would take the font for a mistake.

DejaVu Sans 2.37, released 2016-07-30, is the font family's last release. The copy in the tree is
`ttf/DejaVuSans.ttf` and `LICENSE` of `dejavu-fonts-ttf-2.37.tar.bz2`, from
`https://github.com/dejavu-fonts/dejavu-fonts/releases/download/version_2_37/dejavu-fonts-ttf-2.37.tar.bz2`.
The GitHub release records no checksum for the archive; the project's SourceForge release lists
SHA-1 `7fa15e7b9676fc3915338c41e76ad454c344fff5` and MD5 `d0efec10b9f110a32e9b8f796e21782c` for
it, which the archive downloaded from GitHub matches, and the two files extracted from it match
the SHA-256 in `SHA256SUMS` (read 2026-11-10). Its licence is Bitstream Vera's, a permissive font
licence that allows copying, modifying, and redistributing the fonts, a modified font under
another name; DejaVu's changes are in the public domain, and the glyphs from Arev under Tavmjong
Bah's licence of the same terms. The same font, as a subset, is the test font of the `label`
crate.

## Decision

We will load every third-party file of the browser example, its scripts, stylesheets, and fonts,
from a copy under `examples/web/vendor/`, byte for byte as its release published it and with its
licence, which is permissive (Q-6), whose SHA-256 checksums `scripts/check-vendor.sh` compares
against the checked-in `examples/web/vendor/SHA256SUMS` on every run; the copies are Leaflet 1.9.4
(`dist/leaflet.js`, `dist/leaflet.css`, and `LICENSE`, under `leaflet/`), which draws the
example's tile view, and DejaVu Sans 2.37 (`ttf/DejaVuSans.ttf` and `LICENSE` of
`dejavu-fonts-ttf-2.37.tar.bz2`, under `dejavu/`), in which it draws its labels.

Adding, replacing, or updating a vendored file follows this decision: a new copy with its
licence, its release and source named where it is recorded, and new checksums.

**Out of scope:** a JavaScript package manager or build step for the example; a base map from a
tile server, which waits for reprojection and geographic sample data; Leaflet or a font in the
published bindings, which stay library-agnostic and take the fonts a page passes; a subset of the
font, which would make the copy no longer the published bytes.

## Alternatives considered

- **Leaflet from a CDN with Subresource Integrity**, from ADR-0021 — no files in the tree, and the
  integrity attribute pins the content; but every page load contacts a third-party origin, and the
  example stops working offline and behind a proxy.
- **npm with a lock file**, from ADR-0021 — a toolchain and a `node_modules` tree for a few static
  files, against Simplicity.
- **OpenLayers**, **MapLibre GL JS**, **a pan-and-zoom of our own**, and **Leaflet 2.0**, from
  ADR-0021 — larger, drawing the map itself instead of Forager's tiles, rewriting tiling, or an
  alpha in an example readers copy from.
- **An ADR for the font beside ADR-0021** — two decisions of one rule, which a reader of either
  would have to put together; the maintainer chose one rule for every third-party file
  (2026-11-07).
- **The font from a web font service** — a third-party origin at page load, which the rule exists
  to keep out.
- **The `label` crate's subset of DejaVu Sans** — small, but not the published bytes, and without
  the glyphs a reader's own labels may need.
- **Another free font, such as Liberation Sans or Noto Sans** — under the SIL Open Font License,
  equally permissive; DejaVu Sans is the family the `label` crate's test font is cut from, so the
  example draws what the tests measure.

## Sources / Prior art

- Leaflet 1.9.4 on npm, <https://www.npmjs.com/package/leaflet>, checked 2026-10-24: BSD-2-Clause,
  tarball integrity
  `sha512-nxS1ynzJOmOlHp+iL3FyWqK89GtNL8U8rvlMOsQdTTssxZwCXh8N2NB3GDQOL+YR3XnWyZAxwQixURb+FA74PA==`
  (ADR-0021).
- DejaVu fonts 2.37, <https://github.com/dejavu-fonts/dejavu-fonts/releases/tag/version_2_37>, and
  its SourceForge release, <https://sourceforge.net/projects/dejavu/files/dejavu/2.37/>, with the
  SHA-1 and MD5 of each file, read 2026-11-10.
- The DejaVu fonts' licence, <https://dejavu-fonts.github.io/License.html>, and the `LICENSE` of the
  release.
- Subresource Integrity, <https://www.w3.org/TR/SRI/>.

## Consequences

- Positive: one rule for every file the example loads that the repository did not write, and the
  font's origin and licence on record.
- Negative / trade-offs: the tree carries about 740 KB of font beside Leaflet's 160 KB; updating a
  copy is a manual step with new checksums, and no advisory database watches it.
- Follow-ups: a base map once reprojection lands; Leaflet 2.0 once it is released as stable.

## Enforcement

`scripts/check-vendor.sh` runs `sha256sum --check` on `examples/web/vendor/SHA256SUMS`, which
lists every file under `examples/web/vendor/`, and fails on a mismatch or a file not listed;
`scripts/check-all.sh` and CI run it, and its self-test cites this ADR with `Verifies:`. The same
script fails when a `<script>` or `<link>` of `examples/web/index.html`, or an `import` of its
module script, names another origin (`http:`, `https:`, or `//`). A `url()` in the page's inline
style, and a font fetched by the page's script, are not read; a reviewer holds those, and the
release, source, and licence each copy names.
