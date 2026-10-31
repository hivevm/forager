# Ticket: Q-9's memory per cached label holds at any count of labels

## What to build

Q-9 says a label the label cache holds keeps at most 400 bytes apart from its text, and 32 more
for each style value bound to it. `crates/engine/tests/label_memory.rs` measures it at 20,000
labels, where a label without style holds about 394 bytes. The review of #164 measured 472 bytes
at 16,385 features, just past a power of two, on main as on that branch: the label cache's lists
double as they grow, and the measuring allocator counts the old and the new buffer at the peak.
The sentence of Q-9 holds at the test's count only.

Decide with the maintainer which of these Q-9 means, then make it true:

- the bound holds at any count: the cache's lists are sized ahead of time, or grow by less than
  double, and the test measures at counts past a power of two;
- the bound holds on average at a stated count: Q-9, or the test with a stated reason, says so.

## Acceptance criteria

- [ ] Given the maintainer's decision, when it lands, then Q-9 and `label_memory.rs` say the same
  thing, and the test measures it at 16,385 labels as well as at 20,000.

## Read first

- Q-9 in `docs/SPECIFICATION.md`; ADR-0035; `crates/engine/tests/label_memory.rs`; the review in
  #164.

## Blocked by

- None (can start immediately)

## Mode

with the human: what Q-9 means is the maintainer's
([`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules), rule 7).
