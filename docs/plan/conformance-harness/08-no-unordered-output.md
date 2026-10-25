# Ticket: Keep hash-map order, clocks, and randomness out of the core

## What to build

A check that the core crates use no hash map or hash set whose order could reach the output, no
clock, and no random source (Q-2), by lint configuration of the workspace or a script, whichever
the existing checks already favour, with a self-test where it is a script.

## Acceptance criteria

- [ ] Given a core crate that iterates a `HashMap`, reads `SystemTime` or `Instant`, or depends on
      a random-number crate, when the checks run, then they fail and name the crate and the use.
- [ ] Given the current tree, when the checks run, then they pass, or each existing use is replaced
      by an ordered one in the same change.
- [ ] A test cites Q-2 with a `Verifies:` marker.

## Read first

- Q-2; ADR-0008 (clippy and its configuration); ADR-0009 (which crates are the core).

## Blocked by

- None (can start immediately)

## Mode

unattended: the checks decide every criterion; a lint configuration or one script, undone by one
revert; nothing ADR-0007 keeps with the maintainer. Stop when a hash map in the core cannot be
replaced without changing a port or the style IR, and hand off.
