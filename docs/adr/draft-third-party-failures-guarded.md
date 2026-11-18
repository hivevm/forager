# ADR-draft-third-party-failures-guarded: A third-party crate's failure on a user's input is kept out of a render by a guard Forager derives from the pinned version, and a guard that leaves a part undrawn warns

- **Status:** 🟡 proposed
- **Date:** 2026-11-18
- **Deciders:** Markus Brigl
- **Applies to:** every guard in Forager's code that keeps an input from a computation of a third-party crate where that crate panics, loops, overflows, or runs out of memory, today in `crates/render-agg` (agg-rust) and `crates/symbolize` (geo and the i_overlay it brings), and the versions `Cargo.lock` pins of those crates

## Context

Q-3 asks that errors of third-party crates become diagnostics, measured by a fuzz run of 5 minutes
per target that ends without a panic, a crash, a hang past 10 s, or more than 2 GiB. The render
target reaches agg-rust, geo, and i_overlay with any `f64`. Between 2026-11-16 and 2026-11-18,
each 300-second run of it, and two reviewers' runs of 45 and 60 seconds, found a new failure in a
third-party crate. Each was fixed by a guard in Forager's own code, read from the crate's source
at the version `Cargo.lock` pins:

- agg-rust 1.2.0's clipper overflowed on a glyph's outline of a huge width. `render-agg` now bounds
  that width as a stroke's (8bb2ed83). Nothing visible moves.
- i_overlay 4.5.2 squared an edge of some `1e-164` to 0 in geo's buffer, and the shape vanished.
  `symbolize` now scales a tiny shape by a power of two around the buffer and back (a13dc8bb).
  Nothing moves.
- i_overlay 4.5.2 looped or vanished on a line buffered by a distance near its grid's step.
  `symbolize` now raises a line's distance to a floor (cb56e3ba). The buffer is wider than GEOS's
  by at most the floor.
- geo 0.33.1's repair panicked on a polygon with a coordinate that is no number. `simplify` now
  fails the layer with a diagnostic (5a003cb0).
- agg-rust 1.2.0's image line multiplied past `i32` at a join far from its short segment.
  `render-agg` now leaves such a piece of a brushed line undrawn (8feb9c16, 44a91d4e). It warns
  nothing.

Three forces meet here. Q-3 forbids the crash. Q-4 asks for MapServer's drawing, so a guard that
moves a pixel is a departure. And a guard derived from a crate's internals holds only for the
version it was read at: the bound of 44a91d4e copies `line3_no_clip`'s halving and
`fix_degenerate_bisectrix` from agg-rust 1.2.0, and the floor of cb56e3ba is set from i_overlay
4.5.2's grid. Upgrading either crate in `Cargo.lock` can break a guard silently. ADR-0025 says
the renderer's memory safety rests on agg-rust's `unsafe` code and on the clipping, and ADR-0041
says `buffer` is geo's `buffer_with_style`. Neither says how a failure of the crate is kept out,
nor that a guard may refuse or change an input. ADR-0046 decides only the brushed start's
reductions, which move no pixel, and asks a refused tile to warn.

The guards answer the five findings in four ways: exact, refused with a diagnostic, refused
silently, and changed within a bound. A reader without this history would take the silent refusal
and the floor for mistakes.

This draft was written by the design revision of 2026-11-18 without the interrogation of the
`propose-adr` skill. Its questions for the maintainer are under *Follow-ups*.

## Decision

We will guard a render against a third-party crate's failure on a user's input in the crate of
Forager that calls it, derived from the version `Cargo.lock` pins.

- A guard must take one of three forms:
  - **exact:** it changes the input and changes the result back, and moves no pixel;
  - **refusal:** it leaves the input or its part undrawn;
  - **bounded change:** it changes the input within a bound it states.
- A refusal must report a diagnostic of the layer that draws the input.
- A bounded change must name its bound in the doc comment of the guard. The page of
  `docs/semantics/` of the function it changes must name the bound under **Not as MapServer**.
- Each guard must name the crate, its version, and the source lines it was derived from, in a
  marker comment `// Guard: <crate> <version>` above the guard.
- A change of `Cargo.lock` that moves a crate a guard names to another version must re-derive
  each guard that names it, in the same change.
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
- **Patch the crate in a fork or a vendored copy.** It fixes the cause, but the fork has to be kept
  in step with upstream, and ADR-0011 takes crates from crates.io only, so a Git fork is refused
  by `cargo deny`'s check of sources.
- **Report upstream and wait.** Q-3's weekly run stays red until a release, and a guard is
  needed meanwhile. Reporting stays open beside this decision.
- **Refuse silently, as 44a91d4e does.** It is the smallest code, but Q-3 asks for a diagnostic,
  and a part that vanishes without one looks like a renderer's bug to a user.
- **Keep the guards undecided, one bug fix each, as today.** Each fix chose its own form, and no
  record ties a guard to the version it was derived from.

## Sources / Prior art

- The merge messages of tickets 16, 18, 19, 20, and 21 of `docs/plan/revision-2026-11-16/`
  (2156170e, 53421100, aa34dc9f, d6691adf), with the lines of agg-rust 1.2.0 and i_overlay 4.5.2
  each read.
- `docs/backlog/line-buffer-vanishes-or-runs-long.md` and
  `docs/backlog/brushed-line-join-bound-loose-at-sharp-turns.md`, two guards' known limits.
- No external source was consulted for this draft.

## Consequences

- Positive: each guard is found by its marker, and an upgrade of agg-rust, geo, or i_overlay names
  the guards to re-derive. A user sees a diagnostic where a part of the map is left out.
- Negative / trade-offs: a refused piece of a brushed line warns where it is silent today, a
  change of the drawing's diagnostics. Each guard costs two tests and a marker. An update of
  the three crates in `Cargo.lock` becomes work of its own.
- Follow-ups, for the maintainer:
  - Does a refused piece of a brushed line warn `symbol-not-drawn`, as a refused tile does, or with
    a code of its own?
  - Is the floor of a line's buffer a bounded change Q-4 accepts, or does it wait for a fix
    upstream?
  - Does the guard's marker suffice, or is the crate's version pinned with `=` in `Cargo.toml`
    too?
  - Do the guards' upstream reports become a backlog entry?

## Enforcement

Tests cite this ADR with `Verifies:`:

- a test in `crates/render-agg` that a refused piece of a brushed line warns;
- the two tests of each guard, the failing input and the ordinary one beside it.

A check under `scripts/` lists each `// Guard: <crate> <version>` marker under `crates/`. It
fails where `Cargo.lock` holds the crate at another version, and where a marker names a crate
`Cargo.lock` does not hold.

**Not mechanically decidable:** that a guard is derived from the lines it names, and that a
bounded change's page names its bound. The review decides them.
