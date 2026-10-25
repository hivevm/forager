# Ticket: Keep hash maps, clocks, and randomness out of the core

## What to build

The lint configuration of the workspace bans, in the core crates, the hash-map and hash-set types
of the standard library, the clocks `SystemTime` and `Instant`, and random sources (Q-2), through
clippy's `disallowed-types` and `disallowed-methods`. `deny.toml` is not touched.

## Acceptance criteria

- [ ] Given a core crate that uses `HashMap`, `HashSet`, `SystemTime`, or `Instant`, when clippy
      runs, then it fails and names the use.
- [ ] Given the current tree, when the checks run, then they pass, or each existing use is replaced
      by an ordered one in the same change.
- [ ] Given a host or adapter crate, then the ban does not apply to it.
- [ ] A test cites Q-2 with a `Verifies:` marker.

## Read first

- Q-2; ADR-0008 (clippy and its configuration); ADR-0009 (which crates are the core).

## Blocked by

- None (can start immediately)

## Mode

unattended: clippy decides every criterion; the lint configuration and any replaced map, undone by
one revert; it touches neither `deny.toml` nor anything else ADR-0007 keeps with the maintainer. It
counts against the cap of three open agent branches of [ADR-0007](../../adr/0007-harness.md), and is
dispatched only while that cap allows. Stop when a hash map in the core cannot be replaced without
changing a port or the style IR, and hand off.
