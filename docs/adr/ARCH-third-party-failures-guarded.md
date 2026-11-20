# ARCH-third-party-failures-guarded: A third-party crate's failure on a user's input is kept out of a render by a guard Forager derives from the pinned version, and a guard that leaves a part undrawn warns

- **Status:** 🟢 accepted
- **Date:** 2026-11-18
- **Deciders:** Markus Brigl
- **Applies to:** every guard in Forager's code that keeps an input from a third-party crate's computation where the crate panics, loops, overflows, or runs out of memory: `crates/render/src/clip.rs`, `crates/render-agg/src/brush.rs`, `crates/render-agg/src/glyphs.rs`, `crates/render-agg/src/lib.rs`, `crates/symbolize/src/geometry.rs`, `crates/symbolize/src/layer_transform.rs`, the diagnostics of a refused part in `crates/engine`, and `Cargo.lock` for the crates the guards name

## Context

Q-3 asks that errors of third-party crates become diagnostics, measured by a fuzz run of 5 minutes
per target that ends without a panic, a crash, a hang past 10 s, or more than 2 GiB. The render
target reaches agg-rust, geo, and the i_overlay that geo brings with any `f64`.

Between 2026-11-16 and 2026-11-18, ticket 01 of revision-2026-11-16 ran the render target for 300
seconds five times. Two reviewers ran it for 45 and 60 seconds. Five of those runs found a failure
in a third-party crate, one found a timeout in Forager's own smoothsia (72c437a6), and the fifth
300-second run was clean (3befb4ec). Each third-party failure was fixed by a guard in Forager's
code, read from the crate's source at the version `Cargo.lock` pins:

- agg-rust 1.2.0's clipper overflowed on a glyph's outline of a huge width. `render-agg` now bounds
  that width as a stroke's (2f27e2bf, ticket 16). Nothing visible moves.
- i_overlay 4.5.2 squared an edge of some `1e-164` to 0 in i_float 1.16.0's `normalize`, and the
  shape vanished from geo's buffer. `symbolize` now scales a tiny shape by a power of two around
  the buffer and back (a13dc8bb in fa1dadcd, ticket 18). Nothing moves.
- i_overlay 4.5.2 looped or vanished on a line buffered by a distance near its grid's step.
  `symbolize` now raises a line's distance to a floor (cb56e3ba in d6691adf, ticket 19). The
  buffer is wider than GEOS's by at most the floor. The maintainer chose the floor; a first floor
  of the shape's size alone was rejected in review, since it missed lines far from the origin.
- geo 0.33.1's `lex_cmp` unwrapped `partial_cmp` of a NaN in the repair of a simplified polygon.
  `simplify` now fails the layer with a diagnostic (53421100, ticket 20).
- agg-rust 1.2.0's image line multiplied past `i32` at a join far from its short segment
  (`renderer_outline_image.rs:456`). `render-agg` now leaves such a piece of a brushed line
  undrawn (2156170e, ticket 21). It warns nothing.

Older guards of the same kind were written without that name:

- in `crates/render/src/clip.rs`, the clip of every path to the image grown by its margin
  ([ARCH-one-clip-to-image](ARCH-one-clip-to-image.md)), and `drawn_width` and `MAX_GLYPH_REACH`
  with `within_glyph_reach`, which keep a stroke's width and a glyph's points within agg-rust's
  fixed-point coordinates. The width moves no pixel. The clip moves no pixel of a path it keeps, but
  drops silently a point that is no finite number, a segment whose coordinate difference overflows,
  and a crossing that is not finite. A TrueType marker's glyph past its reach is not drawn,
  silently; a label's glyph past it is cut by `label` and warns `label-glyph-out-of-reach`;
- in `crates/render-agg/src/brush.rs`, the filter of points that are no finite number, silent,
  `line_coord_sat`, which holds a coordinate to agg-rust's `LINE_MAX_COORD`, `within_i32`, which
  bounds the pattern's place, and the start reductions of REN-symbol-line-brushed-by-tile's *No
  panic* clause.

Three forces meet here. Q-3 forbids the crash. Q-4 asks for MapServer's drawing, so a guard that
moves a pixel is a departure. A guard derived from a crate's internals holds only for the version
it was read at: the bound of 44a91d4e copies `line3_no_clip`'s halving and
`fix_degenerate_bisectrix` from agg-rust 1.2.0, and the floor of cb56e3ba is set from i_overlay
4.5.2's grid. An update of `Cargo.lock` can break a guard and leave no failing test, since the
regression tests hold the inputs that failed at the old version.

REN-agg-rust-rasterizer says the renderer's memory safety rests on agg-rust's `unsafe` code and on
the clipping. GEO-simplify-buffer-through-geo says `buffer` is geo's `buffer_with_style`. Neither
says how a failure of the crate is kept out, nor that a guard may refuse or change an input.
REN-symbol-line-brushed-by-tile decides only the brushed start's reductions, which move no pixel,
and asks a refused tile to warn. A reader without this history would take the silent refusal and the
floor for mistakes.

**The maintainer's answers** (2026-11-18, the interrogation of `propose-adr`, held through the
session that ran the design revision):

- The decision binds every guard against a third-party crate, the older ones above included.
- A refused piece of a brushed line is reported as a warning on the layer that draws it.
- A guard is bound to its crate's version by a marker in the code and a check against `Cargo.lock`,
  not by an exact `=` requirement in `Cargo.toml`.
- The clip of every path to the image grown by its margin, which ARCH-one-clip-to-image decides, is
  a guard under this decision too. Its marker and tests leave ARCH-one-clip-to-image's text
  untouched, as for `drawn_width`, `MAX_GLYPH_REACH`, and REN-symbol-line-brushed-by-tile's
  reductions.
- Every refusal warns, the older silent ones included: the filter of a brushed line's points that
  are no finite number, a glyph past `within_glyph_reach`, and a stroke of no positive width if
  it guards agg-rust.
- Still open: whether the guards are reported upstream, and whether Q-4 accepts the floor of a
  line's buffer.

The forms, the warning, the marker, and the tests are stated as one decision. Each is a duty of the
same piece of code, a guard. A guard decided without its binding to a version, or without what a
user sees of a refusal, is half-decided, and the next fix would choose the rest again.

## Decision

We will guard a render against a third-party crate's failure on a user's input in the Forager
crate that calls it, by the version `Cargo.lock` pins.

- A guard must take one of three forms:
  - **exact:** it changes the input and changes the result back, and moves no pixel;
  - **refusal:** it leaves the input or its part undrawn;
  - **bounded change:** it changes the input within a bound it states.
- The layer that draws a refused input must report a warning.
- A bounded change must name its bound in the guard's doc comment. The page of `docs/semantics/`
  that describes the changed function must name the bound and how it differs from MapServer.
- A marker comment `// Guard: <crate> <version>` must stand above each guard. The guard's doc
  comment must name the source lines of that version it was derived from.
- A change of `Cargo.lock` that moves a named crate to another version must re-derive each guard
  that names it, in the same change.
- Each guard must ship two tests. In one, the failing input must not fail. In the other, an
  ordinary input beside it must draw as before the guard.

**Out of scope:**

- Forager's own code that loops or overflows, such as the smoothsia timeout of ad075a4c. It is a
  bug fix under Q-9, not a guard.
- Whether a guard is reported upstream, and whether Forager takes up a fixed version.
- The bound of each guard, which its own fix decides.

## Alternatives considered

- **Catch the panic at the crate's call.** It is recalled, not verified here, that
  `wasm32-unknown-unknown` builds abort on a panic, so `catch_unwind` does not return in the
  browser. It would not stop a loop, a timeout, or an allocation of gigabytes in any case.
- **Patch the crate.** It fixes the cause, but each form has a cost of its own:
  - a Git fork is refused by `cargo deny`'s check of sources, since SEC-crates-locked-and-denied
    takes crates from crates.io only;
  - a vendored copy in the tree, through `[patch]` with a path, is kept in step with upstream by
    hand, and its licence and notice become Forager's to carry;
  - a fork published on crates.io passes SEC-crates-locked-and-denied, but Forager maintains a crate
    of its own.
- **Report upstream and wait.** Q-3's weekly run stays red until a release, and a guard is
  needed meanwhile. Reporting stays open beside this decision.
- **Pin each guarded crate with `=` in `Cargo.toml`.** Rejected by the maintainer. A pin stops an
  update, but it does not say which guard to re-derive when the pin moves, and `Cargo.lock`
  already holds the version.
- **No binding to a version.** Rejected by the maintainer. An update can break a guard silently,
  and no record says which guard to re-check.
- **Refuse silently, as 44a91d4e does.** Rejected by the maintainer. It is the smallest code, but
  Q-3 asks for a diagnostic, and a part that vanishes without one looks like a renderer's bug.
- **Keep the guards undecided, one bug fix each, as before.** Each fix chose its own form, and no
  record ties a guard to the version it was derived from.

## Sources / Prior art

- The merges of tickets 16 (2f27e2bf in c2d97aaf, #204), 18 (a13dc8bb in fa1dadcd, #210),
  19 (cb56e3ba in d6691adf, #211, a merge without a body), 20 (53421100), and 21 (2156170e), and
  the plan commits of tickets 17 and 18 (b350ec1a, aa34dc9f).
- The pinned crates' sources in the local cargo registry, read for this draft:
  - agg-rust 1.2.0: `renderer_outline_image.rs:452-456`, the products of `dist_start` and
    `dist_end`; `line_aa_basics.rs:15-16`, `LINE_MAX_COORD` and `LINE_MAX_LENGTH`;
    `renderer_outline_aa.rs:671`, `MAX_HALF_WIDTH`;
  - geo 0.33.1: `utils.rs:92-97`, `lex_cmp`, which unwraps `partial_cmp`;
  - i_float 1.16.0, which i_overlay 4.5.2 takes: `float/vector.rs:34-37`, `normalize`, which
    divides by the length without a check.
- `docs/backlog/line-buffer-vanishes-or-runs-long.md` and
  `docs/backlog/brushed-line-join-bound-loose-at-sharp-turns.md`, two guards' known limits.
- Not consulted: the crates' issue trackers and later releases, since no network was used.
  Whether a later release fixes a failure is unknown.

## Consequences

- Positive: each guard is found by its marker, and an update of agg-rust, geo, i_overlay, or
  i_float names the guards to re-derive. A user sees a warning where a part of the map is left
  out.
- Negative / trade-offs:
  - the map's diagnostics change where a refusal is silent today. A layer warns for:
    - a refused piece of a brushed line;
    - points of a brushed line that are no finite number;
    - what the clip drops of a stroked or filled path: a point that is no finite number, a
      segment whose coordinate difference overflows, and a crossing that is not finite;
    - a TrueType marker's glyph past `within_glyph_reach`;
    - a stroke of no positive width, and the other silent skips of `render-agg`, where the
      refusal guards agg-rust.

    A label's glyph past the reach keeps its one warning, `label-glyph-out-of-reach`;
  - `render-agg` needs a way to report to the layer, which the renderer trait of `crates/render`
    lacks today;
  - each guard costs two tests and a marker, the older guards included;
  - an update of the guarded crates in `Cargo.lock` becomes work of its own.
- Follow-ups:
  - whether the guards' failures are reported upstream, and whether that becomes a backlog
    entry;
  - whether Q-4 accepts the floor of a line's buffer, or the floor waits for a fix upstream;
  - Q-3's wording, which says "become diagnostics" and does not name a guard's refusal.

## Enforcement

Tests cite this ADR with `Verifies:`:

- a test in `crates/engine` for each refusal that the layer carries its warning once:
  - a refused piece of a brushed line;
  - points of a brushed line that are no finite number;
  - each drop of the clip;
  - a TrueType marker's glyph past `within_glyph_reach`;
- the two tests of each guard, the failing input and the ordinary one beside it.

A check under `scripts/` lists each `// Guard: <crate> <version>` marker under `crates/`. It
fails where `Cargo.lock` holds the crate at no version the marker names, and where a marker names
a crate `Cargo.lock` does not hold.

**Not mechanically decidable:** that a guard is derived from the lines it names, and that every
guard carries a marker. The same holds for a bounded change's page naming its bound. The review
decides them.
