# Ticket: Draft G-11 as what its test now measures

## What to build

G-11 reads: "When a proj4 transformation used by a test case is evaluated, the core's reprojection
shall agree with PROJ, as MapServer at the pin calls it, within 1 mm." Its test,
`crates/proj/tests/reference.rs`, now holds three things within that tolerance:

- points against `msProjectPoint`;
- rectangles against `msProjectRect`;
- shapes against `msProjectShape`, cut at the horizon and wrapped at the dateline.

The last two are MapServer's own algorithms, not PROJ's. The criterion's text says less than the
tests decide. Draft a wording for the maintainer that says what is compared, with what, and within
what. For example: "When a point, a rectangle, or a shape is reprojected between the proj4
projections the test cases use, the core's reprojection shall agree with MapServer at the pin
within 1 mm."

The draft names the accepted ADRs whose basis it may move, for the maintainer to judge
([`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules), rule 7):

- ADR-0038 (what its records hold; see ticket 12);
- not ADR-0037. On 2026-11-15 the maintainer decided that G-11's change ("agree with PROJ as
  MapServer at the pin calls it, within 1 mm") and the named-datum rule do not move ADR-0037's
  basis. ADR-0037 leaves the reference and the tolerance to the measurement, and stays unchanged.
  The draft cites that decision and does not ask it again.

The agent drafts. Only the maintainer lands the change.

## Acceptance criteria

- [ ] Given the draft, when the maintainer reads it, then it states G-11's comparison, its
  reference, and its tolerance, lists ADR-0038 with the question on it, and cites the
  maintainer's decision of 2026-11-15 on ADR-0037.
- [ ] Given the maintainer's decision, when the change lands, then `scripts/check-traceability.sh`
  passes and every `Verifies: G-11` marker decides the whole criterion as reworded.

## Read first

- Touches: `docs/SPECIFICATION.md` (G-11)
- `docs/adr/0037-reprojection-ported-in-proj.md`, `docs/adr/0038-proj-references-from-mapserver.md`;
  `crates/proj/tests/reference.rs`; `docs/CONVENTIONS.md`, *A `Verifies:` marker claims every
  shall of its criterion*.

## Blocked by

- None (can start immediately)

## Mode

with the human: a change to the specification is the human's to land
([`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules), rule 7).
