# Ticket: Migrate the record to NUC v0.9.0

## What to build

Forager takes up NUC v0.9.0, the tracer bullet of the take-up. The inherited ADRs give way to
the twelve `NUC-` ADRs of the fixed base. Each of Forager's own ADRs is renamed to
`<CAT>-<slug>`, by a map the maintainer has reviewed. Each clause Forager wrote into an inherited
ADR is kept in an ADR of its own. The checks, sensors, hooks, skills, and documents of v0.9.0
are taken up, merged with Forager's changes. The `Template release` line reads v0.9.0, and
`scripts/check-all.sh` passes.

Follow section 8 of the normalize skill of v0.9.0, "Migrate a numbered record", with these
differences of Forager's:

- Forager's inherited ADRs are `0001` to `0005` and `0009`, not `0001` to `0006`. Step 1 diffs
  each against the template file of v0.8.1 it restates: `0001`↔`0001`, `0002`↔`0002`,
  `0003`↔`0003`, `0004`↔`0005`, `0005`↔`0006`, and `0009`↔`0004`. Do not diff by number.
- `0009` carries decisions of Forager's own beyond the template's `0004`: Rust on the Debian
  base image, and the GitHub CLI's login kept on the host. They become ADRs of Forager's own
  that extend NUC-container-hides-host-daemon, and they keep their tests.
- `0006` (harness) and `0007` (toolchain) are Forager's own. They take the `Role:` lines.
- Give the maintainer the map of the 44 own ADRs before anything moves, and wait for the answer.
- References in code comments, tests, and scripts move with the record. A reference that names
  another repository's ADR, such as a message quoted from NUC, keeps its number by hand.

## Acceptance criteria

- [ ] Given the map of Forager's 44 own ADRs, when the maintainer has reviewed it, then the
      files move by that map, and the pull request carries it under the line `ADR map:`.
- [ ] Given the change, when `docs/adr/` is listed, then it holds the `NUC-` files of v0.9.0 as
      `docs/adr/NUC.blobs` lists them, and each ADR of Forager's own as `<CAT>-<slug>.md`, and no
      numbered ADR file.
- [ ] Given each clause Forager added to an inherited ADR or to `AGENTS.md`, when the record is
      read, then an accepted ADR of Forager's own carries it with `Extends:` or `Narrows:`, or
      the pull request names the maintainer's decision to drop it.
- [ ] Given the harness ADR and the toolchain ADR, when `scripts/check-docs.sh` runs, then each
      carries its `Role:` line and the check passes.
- [ ] Given the tree, when `git grep -nE 'ADR-[0-9]{4}'` runs, then each hit names another
      repository's ADR, and the pull request lists them.
- [ ] Given the change, when `scripts/check-all.sh` runs and the CI of the pull request runs,
      then both pass, and the `Template release` line of `README.md` reads v0.9.0.

## Read first

- Touches: `docs/adr/`, `AGENTS.md`, `README.md`, `CONTRIBUTING.md`, `scripts/`, `.agents/skills/`,
  `.githooks/`, `.github/workflows/checks.yml`, and every file that cites an ADR by number
- The release notes of v0.9.0: `gh release view v0.9.0 -R hivevm/nuc`, section *Taking it up*
- The template at v0.9.0 and at v0.8.1, cloned outside the tree; in v0.9.0,
  `.agents/skills/normalize/SKILL.md` sections 7 and 8, and `map-v0.9.0.txt` next to it
- The spec, sections *Existing behaviour* and *Implementation decisions*
- `docs/adr/README.md` (the index and its mechanics), and the propose-adr skill for the added
  clauses
- The take-up of v0.8.1, PR #173, as the golden path of merging the template's changes into
  Forager's files

## Blocked by

- None (can start immediately)

## Mode

with the human: it replaces the inherited ADRs, renames every ADR of Forager's, and proposes new
ones, and ADR-0006 keeps every ADR with the maintainer. The maintainer reviews the map before the
files move, and accepts each new ADR before the pull request merges.
