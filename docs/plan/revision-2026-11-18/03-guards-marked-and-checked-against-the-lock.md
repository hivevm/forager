# Ticket: Every guard against a third-party crate carries its marker and tests, and a check holds the markers to `Cargo.lock`

## What to build

ADR-draft-third-party-failures-guarded binds every guard against a third-party crate. The guards
are derived from the versions `Cargo.lock` pins: agg-rust 1.2.0, geo 0.33.1, and i_overlay 4.5.2
with i_float 1.16.0 under geo. Nothing ties them to those versions, and the older ones were never
named guards.

Each guard below gets the marker `// Guard: <crate> <version>`, the source lines it was derived
from in its doc comment, and the two tests the ADR asks for where they are missing. A marker on a
guard an accepted ADR decides, ADR-0027's clip or ADR-0046's reductions, leaves that ADR's text
untouched. Thirteen guards, eight older and five from tickets 16 to 21 of revision-2026-11-16:

- `crates/render/src/clip.rs`:
  1. the clip of every path to the image grown by its margin (ADR-0027);
  2. `drawn_width`, a stroke's width within agg-rust's fixed-point coordinates;
  3. `MAX_GLYPH_REACH` with `within_glyph_reach`, a glyph's points within them;
- `crates/render-agg/src/brush.rs`:
  4. the filter of points that are no finite number, before the subpixels;
  5. `line_coord_sat`, a coordinate held to agg-rust's `LINE_MAX_COORD`;
  6. `within_i32`, the pattern's place, unless ticket 02 has merged it into the probe;
  7. the start reductions of `Carried` and of a piece's start, ADR-0046's *No panic*;
  8. `JoinProbe` with `joins_within_i32`, the joins (ticket 21), as ticket 02 leaves it;
- `crates/render-agg/src/glyphs.rs`:
  9. the glyph outline's width bounded as a stroke's (ticket 16);
- `crates/symbolize/src/geometry.rs`:
  10. a tiny shape scaled by a power of two around the buffer (ticket 18);
  11. a line's buffer distance raised to its floor (ticket 19);
- `crates/symbolize/src/layer_transform.rs`:
  12. `simplify`'s refusal of a polygon with a coordinate that is no finite number (ticket 20);
  13. `buffer`'s refusal of a shape with one.

The session classifies `render-agg`'s refusal of a stroke of no positive width in `stroke`
(`lib.rs`). As a guard of agg-rust it takes a marker here and a warning in ticket 11; as a rule of
the display list it takes neither. The warnings of the older silent refusals are ticket 11's.

A check under `scripts/` lists the markers under `crates/`. It fails where `Cargo.lock` holds the
crate at no version the marker names, or does not hold it. A crate held at two versions, as
i_overlay is (4.5.2 under geo, 9.1.0 in `render-agg`), matches where one of them is the marker's.
`scripts/check-all.sh` runs it, and CI runs it where it runs the other checks of `scripts/`.

## Acceptance criteria

- [ ] Given a fixture whose marker names the locked version, when the self-test runs the check,
  then it passes.
- [ ] Given a fixture whose marker names another version, when the self-test runs the check,
  then it fails and names the file, the crate, and both versions.
- [ ] Given the tree after this ticket, when the check runs, then it passes and lists one marker
  for each guard of the list above that still exists, at least twelve.
- [ ] Given each guard of the list, when its tests are read, then one holds the failing input and
  one an ordinary input beside it, each citing the ADR. A guard whose failing input cannot be
  built in a test says so in the hand-off.

## Read first

- Touches: `scripts/check-guards.sh` (new), its self-test `scripts/test-check-guards.sh`,
  `scripts/check-all.sh`, `.github/workflows/checks.yml`, `CONTRIBUTING.md` (the scripts list),
  the five files above and their tests
- `docs/adr/draft-third-party-failures-guarded.md`; ADR-0007, ADR-0011, ADR-0027, ADR-0046;
  `scripts/lib/check.sh`, `scripts/lib/selftest.sh`.

## Blocked by

- docs/plan/revision-2026-11-18/02-brushed-piece-refused-once-and-reported.md (brush.rs's guards)
- docs/plan/revision-2026-11-18/04-render-agg-stroke-hands-a-bounded-pen.md (the width's guard)

## Mode

with the human: it implements ADR-draft-third-party-failures-guarded, which waits for the
maintainer's decision, and the guards of `layer_transform.rs` and `geometry.rs` charge or refuse
under Q-9's budget, which ADR-0006 keeps from unattended work. Stop when a guard's failing input
cannot be rebuilt from its commit's test; hand off with the list of guards done.
