# OPS-container-reaches-no-engine: The Dev Container gives no access to any container engine, not even one inside it

- **Status:** 🟡 proposed
- **Date:** 2026-11-20
- **Deciders:** Markus Brigl
- **Applies to:** the `features`, `mounts`, and `runArgs` of `.devcontainer/devcontainer.json`
- **Narrows:** NUC-container-hides-host-daemon §2

## Context

[NUC-container-hides-host-daemon §2](NUC-container-hides-host-daemon.md) keeps the host's daemon
out of the container and leaves an engine inside it, such as the Docker-in-Docker Feature, to the
project. Nothing the specification's goals test needs a running service or a container, and the
harness ([OPS-parallel-sessions-capped-at-three](OPS-parallel-sessions-capped-at-three.md)) keeps
parallel sessions apart as worktrees in one repository, not as containers. The decision was a
clause of Forager's Dev Container ADR until the migration to NUC v0.9.0.

## Decision

We will give the container no engine at all:

1. **No engine.** The container must give no access to any container engine. The host's is ruled
   out by [NUC-container-hides-host-daemon §2](NUC-container-hides-host-daemon.md) already, and
   this clause rules out one inside the container too, such as the Docker-in-Docker Feature.

**Out of scope:** a container engine for an adapter whose tests need a database, decided with that
adapter; a container per session.

## Alternatives considered

- **Docker-in-Docker** — talks to no host engine, but a privileged container and an engine nothing
  in the project uses.

## Sources / Prior art

- The catalogue entry *Container access* of the `propose-adr` skill.
- Docker daemon attack surface —
  <https://docs.docker.com/engine/security/#docker-daemon-attack-surface>.

## Consequences

- Positive: an agent in the container controls no containers at all.
- Negative / trade-offs: a test that needs a service has to wait for the decision of its adapter.

## Enforcement

`scripts/check-devcontainer.sh` fails on a host socket and on the Feature that mounts one, under
[NUC-container-hides-host-daemon §2](NUC-container-hides-host-daemon.md).

**Not mechanically decidable:** a Feature that would run an engine inside the container, which the
review of a change to the Dev Container catches.
