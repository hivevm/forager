# Ticket: Q-9's memory per cached label holds at any count of labels

## What to build

Q-9 says a label the label cache holds keeps at most 400 bytes apart from its text, and 32 more
for each style value bound to it. `crates/engine/tests/label_memory.rs` measures it at 20,000
labels, where a label without style holds about 394 bytes. The review of #164 measured 472 bytes
at 16,385 features, just past a power of two, on main as on that branch: the label cache's lists
double as they grow, and the measuring allocator counts the old and the new buffer at the peak.
The sentence of Q-9 holds at the test's count only.

Decide with the maintainer which of these Q-9 means, then make it true:

1. the bound holds at any count: the cache's lists are sized ahead of time, or grow by less than
   double, and the test measures at counts past a power of two;
2. the bound holds at a stated count: Q-9, or the test with a stated reason, says so, and names
   the peak past a power of two as outside it.

`docs/plan/revision-2026-10-31/03-label-texts-held-once.md` drops the per-point copies of a
label's text from the cache (`texts` in `cache`) and adds cases to `label_memory.rs`. That changes
the lists this ticket measures and the file it edits, so it lands first, and the numbers above are
measured again on it before the decision.

## Acceptance criteria

- [ ] Given option 1, when it lands, then `label_memory.rs` measures a label without style at
  16,385 and at 20,000 labels, and each holds within 400 bytes, as Q-9 states.
- [ ] Given option 2, when it lands, then Q-9 and `label_memory.rs` name the same count, the test
  measures at that count and holds within 400 bytes, and the sentence that states the bound says
  that a count just past a power of two peaks above it.

## Read first

- Q-9 in `docs/SPECIFICATION.md`; ADR-0035; `crates/engine/tests/label_memory.rs`; the review in
  #164; `docs/plan/revision-2026-10-31/03-label-texts-held-once.md`.

## Blocked by

- docs/plan/revision-2026-10-31/03-label-texts-held-once.md

## Mode

with the human: what Q-9 means is the maintainer's
([`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules), rule 7).
