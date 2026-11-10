# ADR-0035: The browser example loads every third-party file, its scripts, stylesheets, and fonts, only from copies vendored in the tree and checked by checksum

- **Status:** 🟢 accepted
- **Date:** 2026-11-10
- **Deciders:** Markus Brigl
- **Applies to:** `examples/web/`, its `vendor/` directory, every third-party script, stylesheet, and font the example loads, and `scripts/check-vendor.sh`

## Context

The browser example displays the image the WASM bindings render (G-5) in a tile view: a map
library asks Forager's `render` for each tile with the tile's extent, with no server in between,
which shows the use Forager is built for. The map library is the example's first JavaScript
dependency. Crate dependencies are governed by [ADR-0011](0011-crate-supply-chain.md); nothing
else governs the files the example loads. The example runs in the user's browser, and the project
gives security precedence over convenience: a file loaded from a third-party origin at page load is
code or data the repository neither reviews nor pins. The licence must be permissive (Q-6). The
library stays out of the WASM bundle and its size limit (Q-5), and out of the core (Q-1).

The example draws labels, and the bindings take the fonts a mapfile names among the files a page
passes; the browser has no font file of its own to hand in. So the example vendors a font beside
its map library.

The sample data is in plain map units (0..100 × 0..60), not geographic coordinates, and Forager
does not reproject geometry yet, so the example uses a planar coordinate reference system without
a base map.

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
only from a copy under `examples/web/vendor/` whose SHA-256 checksum `scripts/check-vendor.sh`
compares against the checked-in `examples/web/vendor/SHA256SUMS` on every run.

- **The copy.** Each copy must be byte for byte as its release published it, with its licence.
  The licence must be permissive (Q-6).
- **The copies today.**
  - Leaflet 1.9.4 (`dist/leaflet.js`, `dist/leaflet.css`, and `LICENSE`, under `leaflet/`), which
    draws the example's tile view;
  - DejaVu Sans 2.37 (`ttf/DejaVuSans.ttf` and `LICENSE` of `dejavu-fonts-ttf-2.37.tar.bz2`, under
    `dejavu/`), in which the example draws its labels.
- **A change.** Adding, replacing, or updating a vendored file follows this decision. It brings a
  new copy with its licence, its release and source named where it is recorded, and new
  checksums.

**Out of scope:** a JavaScript package manager or build step for the example; a base map from a
tile server, which waits for reprojection and geographic sample data; Leaflet or a font in the
published bindings, which stay library-agnostic and take the fonts a page passes; a subset of the
font, which would make the copy no longer the published bytes.

## Alternatives considered

- **The rule for scripts and stylesheets alone** — the first record of this rule, for Leaflet. It
  said nothing of a font, of where a copy comes from, or which release, and weighed only Leaflet's
  BSD-2-Clause. Once the example drew labels and vendored DejaVu Sans, a reader of it would have
  taken the font for a mistake; the maintainer chose one rule for every third-party file
  (2026-11-07).
- **An ADR for the font beside the rule for scripts** — two decisions of one rule, which a reader
  of either would have to put together.
- **Leaflet from a CDN with Subresource Integrity** — no files in the tree, and the integrity
  attribute pins the content; but every page load contacts a third-party origin, and the example
  stops working offline and behind a proxy.
- **npm with a lock file** — the usual route for JavaScript, with `npm audit`; a toolchain and a
  `node_modules` tree for a few static files, against Simplicity.
- **OpenLayers** — richer, with first-class projections; an ES module tree several times Leaflet's
  size that wants a bundler.
- **MapLibre GL JS** — vector-tile renderer with WebGL; it would draw the map itself instead of
  showing Forager's raster tiles, which is what the example exists to show.
- **A pan-and-zoom of our own on the canvas** — no dependency, but tiling, input handling, and
  zoom levels written again for an example.
- **Leaflet 2.0** — published only as `2.0.0-alpha.1`; an alpha in an example readers copy from.
- **The font from a web font service** — a third-party origin at page load, which the rule exists
  to keep out.
- **The `label` crate's subset of DejaVu Sans** — small, but not the published bytes, and without
  the glyphs a reader's own labels may need.
- **Another free font, such as Liberation Sans or Noto Sans** — under the SIL Open Font License,
  equally permissive; DejaVu Sans is the family the `label` crate's test font is cut from, so the
  example draws what the tests measure.

## Sources / Prior art

- Leaflet 1.9.4 on npm, <https://www.npmjs.com/package/leaflet>, checked 2026-10-24: `latest`
  dist-tag, BSD-2-Clause, published 2023-05-18; tarball integrity
  `sha512-nxS1ynzJOmOlHp+iL3FyWqK89GtNL8U8rvlMOsQdTTssxZwCXh8N2NB3GDQOL+YR3XnWyZAxwQixURb+FA74PA==`
  verified against the downloaded file.
- Leaflet `GridLayer.createTile` and `CRS.Simple`, <https://leafletjs.com/reference.html>.
- DejaVu fonts 2.37, <https://github.com/dejavu-fonts/dejavu-fonts/releases/tag/version_2_37>, and
  its SourceForge release, <https://sourceforge.net/projects/dejavu/files/dejavu/2.37/>, with the
  SHA-1 and MD5 of each file, read 2026-11-10.
- The DejaVu fonts' licence, <https://dejavu-fonts.github.io/License.html>, and the `LICENSE` of the
  release.
- Subresource Integrity, <https://www.w3.org/TR/SRI/>.

## Consequences

- Positive: the example pans and zooms, and each tile is a call of `render` with the tile's
  extent. No page load reaches a third-party origin, and a changed vendored file fails the checks.
- Positive: one rule for every file the example loads that the repository did not write, with
  each copy's origin and licence on record.
- Negative / trade-offs: the tree carries about 740 KB of font beside Leaflet's 160 KB of
  minified JavaScript and CSS; updating a copy is a manual step with new checksums, and no
  advisory database watches it, so the maintainer follows the releases.
- Follow-ups: a base map once reprojection lands; Leaflet 2.0 once it is released as stable.

## Enforcement

`scripts/check-vendor.sh` runs `sha256sum --check` on `examples/web/vendor/SHA256SUMS`, which
lists every file under `examples/web/vendor/`, and fails on a mismatch or a file not listed.
`scripts/check-all.sh` and CI run it, and its self-test, `scripts/test-check-vendor.sh`, cites this
ADR with `Verifies:`. The same script fails when a `<script>` or `<link>` of
`examples/web/index.html`, or an `import` of its module script, names another origin (`http:`,
`https:`, or `//`).

**Not mechanically decidable:** a `url()` in the page's inline style, and a font fetched by the
page's script, which the check does not read; and the release, source, and licence each copy
names. A reviewer holds them.
