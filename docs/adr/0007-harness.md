# ADR-0007: Tickets are worked in parallel by Claude Code sessions under a cap of three, claimed locally, each reviewed in a fresh session, with the threat-protected parts and public interfaces never worked unattended

- **Status:** 🟡 proposed
- **Date:** 2026-10-23
- **Deciders:** Markus Brigl
- **Applies to:** every session that implements or reviews a change, `docs/plan/`, the `work-tickets` and `review` skills, and the worktrees under `.scratch/`

## Context

Forager grows in phases over many crates (*Strategy* of the
[specification](../SPECIFICATION.md)); the work spans many sessions, and most of it — a mapfile
keyword, a symbol type, an adapter — is a narrow slice a test can decide. One maintainer decides
and reviews, on one machine. With agents the bottleneck moves from writing to reviewing, so the
amount of open agent work has to stay below what one human reads.

Some parts carry the specification's threats: the credential filter of the browser bindings
(Q-7), the resolution of resource paths against the base context (Q-8), and the resource limits
(Q-9). Others are public interfaces whose mistakes are costly to reverse: the port traits, the
style document format and its versioning (G-4), and the API of the browser bindings.

The catalogue entry *Harness* of the `propose-adr` skill offers three forms: one agent with the
human reviewing everything, implementer and reviewer apart, and parallel tickets under a cap. Its
*Does not fit* warns against unattended work without a cap and against an implementer reviewing
its own change.

## Decision

We will work tickets in parallel through the `work-tickets` skill under the following harness:

- **Implementer:** Claude Code on Opus 5.5, started without a human by
  `claude -p "<ticket path>"` in the ticket's own worktree under `.scratch/`.
- **Reviewer:** Claude Code on Opus 5.5 in a fresh session, started by
  `claude -p "/review <branch>"`, given only what the `review` skill names. The session that wrote
  a change never reviews it ([`AGENTS.md` §5](../../AGENTS.md#5-quality-bar--definition-of-done)).
- **Claims:** a ticket is claimed by creating its branch and worktree in the one local repository;
  no claim is made on a remote, and no agent pushes without an instruction from the maintainer.
- **Cap:** at most three agent branches open at once.
- **Never unattended,** however small the ticket, and reviewed by the maintainer as well: the
  credential filter of the browser bindings, the resolution of resource paths, the resource
  limits, the port traits, the style document format and its versioning, the public API of the
  browser bindings, `deny.toml` and the licences, the specification, and the ADRs.
- **Decider:** Markus Brigl decides every ADR and every change to the specification, for all
  parts of the system; the code owner of `docs/adr/` and the specification is `@hivevm`.

**Out of scope:** a second maintainer or machine and the remote claims they would need; a reviewer
of a different model or vendor; containers per session.

## Alternatives considered

- **One agent, the human reviews every change** — the catalogue's simplest form; it serializes the
  work behind one session and leaves the feature layer's tickets without their purpose.
- **Implementer and reviewer apart, without parallel tickets** — the same review, but no
  parallelism where tickets are independent (adapters, symbol types, mapfile keywords).
- **Claims on `origin`** — needed only when sessions dispatch from clones of their own; here they
  share one repository, and the claim push would be a standing grant to push without an
  instruction ([ADR-0006](0006-feature-layer.md)).
- **No cap** — the review backlog grows faster than one maintainer reads it.

## Sources / Prior art

- The catalogue entry *Harness* of the `propose-adr` skill and its sources: Birgitta Böckeler,
  *Harness engineering* (<https://martinfowler.com/articles/harness-engineering.html>); Addy
  Osmani, *Agentic autonomy levels* (<https://addyosmani.com/blog/agentic-autonomy-levels/>).

## Consequences

- Positive: independent slices proceed in parallel; every change gets a review in a context that
  did not write it.
- Negative / trade-offs: three open branches can collide in shared crates (`style-ir`, `ports`);
  the plan's blockers have to say so. The threat-protected parts and public interfaces always
  wait for the maintainer. Implementer and reviewer share a model, so they can share a blind spot.
- Follow-ups: whether a reviewer of another model raises review quality, once there is a body of
  reviews to compare; remote claims once a second machine dispatches.

## Enforcement

**Not mechanically decidable:** which session reviews a change, who decides an ADR, and whether a
ticket ran unattended are decided outside the tree. What the harness relies on mechanically is
already checked: the git hooks under `.githooks/`, the required checks, and the Code Owners review
of the repository settings;
[`scripts/sensor-plan-frontier.sh`](../../scripts/sensor-plan-frontier.sh) lists the tickets a
dispatching session may claim.
