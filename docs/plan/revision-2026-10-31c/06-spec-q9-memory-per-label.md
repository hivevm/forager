# Ticket: State in Q-9 the memory a cached label may hold (drafted specification change)

## What to build

The spec `labels-2` served Q-9 as "a label's styles add to the cost of a label; the label cache's
bound holds". Q-9 bounds the labels one render's label cache holds (100,000), not what each of them
holds. `crates/engine/tests/label_memory.rs` cites Q-9 and holds a cached label under 400 bytes, a
number no criterion and no ADR states, and twice in the spec it decided the work (#155, #156): a
change of the number, or the test's removal, would be judged against nothing. Draft for the
maintainer, beside the label bound of Q-9:

> … the labels one render's label cache holds (100,000), each holding a bounded size apart from
> its text (400 bytes) …

or, should the number be the test's and not the specification's, the test's own statement of why
400 and what moves it. It moves the basis of
[ADR-0035](../../adr/0035-label-cache-grid-and-bound.md), whose bound counts labels and whose
measurements take a label's memory as given; the maintainer judges that and any other, not the
agent ([`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules), rule 7). The drafted Q-9 changes of
`docs/plan/revision-2026-10-24/06-q9-feature-and-source-limits.md` and
`docs/plan/revision-2026-10-31/07-spec-q9-markers-by-any-path.md` change the same sentence and may
land with it.

## Acceptance criteria

- [ ] Given the maintainer's decision on the wording, when it lands, then Q-9, or the test with a
  stated reason, says what bounds a cached label's memory, and `label_memory.rs` cites it.

## Read first

- Q-9 in `docs/SPECIFICATION.md`; ADR-0035; `crates/engine/tests/label_memory.rs`; the two
  tickets named above.

## Blocked by

- None (can start immediately)

## Mode

with the human: a change of the specification is the maintainer's
([`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules), rule 7).
