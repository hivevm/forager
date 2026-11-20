# Spec: Take up NUC v0.11.1

## Problem

Forager carries NUC v0.8.1 on its `Template release` line. The template has released v0.9.0,
v0.9.1, v0.10.0, v0.11.0, v0.11.1, and v0.12.0 since. Every release the project does not take up
widens the distance a later take-up has to cross. v0.9.0 is a migration of the whole record that
every later release presumes. Until Forager crosses it, no template fix reaches it by replacing
files, and the template's findings channel, its keyed findings, and its linear `feat/`
integration stay out of reach.

## Solution

The maintainer finds Forager on NUC v0.11.1:

- The record is the fixed base of thirteen `NUC-` ADRs plus Forager's own ADRs, each named by a
  category and a slug. Each clause Forager had written into an inherited ADR is carried by an ADR
  of its own that extends or narrows the `NUC-` clause.
- The checks and sensors are those of v0.11.1, merged with Forager's own changes to them. Every
  finding ends in a key, and every script takes its root as `--root <dir>`.
- A `feat/` branch integrates linearly, and the design revision falls due after 50 changes.
- A later take-up, such as v0.12.0, replaces files instead of migrating.

It gets there in four steps. Each lands on `main` by a pull request of its own and leaves the
`Template release` line on a true release: v0.9.0, v0.9.1, v0.10.0, v0.11.1.

## Existing behaviour

- The record is 50 numbered ADRs, `0001` to `0050`. Forager's `0001` to `0005` restate the
  template's `0001`, `0002`, `0003`, `0005`, and `0006` of v0.8.1, with clauses of Forager's own
  added. Forager's `0009` restates the template's `0004` and adds the Rust toolchain in the base
  image and the GitHub CLI's login on the host. `0006` (harness), `0007` (toolchain), `0008`, and
  `0010` to `0050` are Forager's own. Some 1,800 references `ADR-NNNN` stand in 342 tracked files,
  among them code comments and tests that cite the decision they verify.
- `AGENTS.md` differs from v0.8.1 only in the numbers of the ADRs it cites.
- 38 of the template's files under `scripts/`, `.agents/`, `.githooks/`, and `.github/` carry
  changes of Forager's own. `checks.yml`, `check-all.sh`, `check-docs.sh`, and the self-tests of
  `check-shell.sh` and `check-docs.sh` carry the most. Forager adds about 46 scripts of its own:
  sensors, self-tests, installers, and the checks of the Rust workspace.
- The design revision is due after 20 changes outside `docs/`.
- What must stay: every decision of Forager's record, each with its tests; every check and sensor
  Forager added, with what it decides; the Rust workspace, its CI jobs, and its scheduled
  workflows. `scripts/check-all.sh` and the self-tests pin the tooling; the traceability check
  pins the citations of tests. Nothing of the product changes, so no test of a crate changes
  beyond the ADR identifiers it cites.
- How references move: the normalize skill's `remap.sh` moves each reference by the release's
  map and by Forager's map of its own ADRs. A reference to another repository's ADR keeps its
  number by hand.

## Goals served

None of the specification's criteria. The work is the take-up that ADR-0004 asks of a derived
project, the unit in which a template change reaches it. It changes no output of Forager.

## User stories

1. As the maintainer, I want Forager on a current template release, so that a template fix
   reaches it by replacing files and not by another migration.
2. As the maintainer, I want to review the map of Forager's ADRs to identifiers before any file
   moves, so that each name states its decision.
3. As the maintainer, I want each clause Forager wrote into an inherited ADR kept as an ADR of
   its own, so that no decision is lost when the inherited ADRs give way to the fixed base.
4. As an agent, I want each ADR named by its decision, so that a citation says what binds.
5. As the maintainer, I want findings of the checks to end in a key, so that a filter or a port
   of the checks does not depend on wording.
6. As an agent, I want every script to take its root as `--root <dir>`, so that a positional
   argument is never mistaken for one.
7. As the maintainer, I want the design revision due after 50 changes, so that a revision covers
   a body of work and not a handful of tickets.
8. As the maintainer, I want a `feat/` branch to integrate linearly, so that its pull request
   into `main` reads as one commit per topic.
9. As the maintainer, I want each step to land on `main` on its own, so that each review covers
   one release and every state of `main` is a true release.

## Implementation decisions

- **Order.** v0.9.0, then v0.9.1, then v0.10.0, then v0.11.0 and v0.11.1 together. Each step
  takes up its release by its notes, *Taking it up*, and moves the release line in the same
  change. v0.11.1 is a patch of wording and fixes, so it lands with v0.11.0.
- **Not v0.12.0.** v0.12.0 hands the git hooks to the `nuc` binary of `hivevm/nuc-tools`. The
  maintainer stops at v0.11.1, so that no commit depends on a binary outside the repository.
- **One pull request per step** (the maintainer, 2026-11-20). There is no `feat/` branch.
- **The migration (v0.9.0)** follows section 8 of the normalize skill of v0.9.0, "Migrate a
  numbered record", with one difference. Forager's inherited ADRs are not `0001` to `0006` but
  `0001` to `0005` and `0009`, the result of Forager's own normalizations. The diff of what
  Forager added runs each of them against the template file of v0.8.1 it restates, not against
  the file of the same number. `0006` and `0007` are Forager's own and take the `Role:` lines
  harness and toolchain.
- **The project map.** One line per numbered ADR of Forager's own, 44 in all. Each takes a
  category from the fixed ones or from the list in the index, and a slug that states the
  decision. The maintainer reviews the map before anything moves. The pull request carries it
  under the line `ADR map:`.
- **The added clauses** become ADRs of Forager's own through `propose-adr`, each with
  `Extends:` or `Narrows:` naming the `NUC-` clause. A clause that contradicts the base is a
  fork, which the maintainer decides. No fork is expected, since `AGENTS.md` differs only in
  numbers.
- **Merging.** Where Forager changed a template file, the step applies the template's change
  between the two tags to Forager's file and keeps Forager's change. A conflict that changes
  what one of Forager's checks decides goes to the maintainer.
- **The revision line** moves from 20 to 50 changes in the v0.10.0 step (the maintainer,
  2026-11-20). v0.10.0 keeps a project's own number until the project edits it.
- **The template findings** of NUC-template-learns-from-projects arrive with v0.10.0. Forager's
  setup is closed, so it does not take up the issue template. Drafts under `.scratch/` are sent
  only by the maintainer.

## Testing decisions

- The seam is `scripts/check-all.sh` with the self-tests it runs. Each step ends with it green,
  and with the CI jobs of `checks.yml` green on the pull request.
- The template's self-tests and fixture tables come with each release. A row Forager added to a
  table moves to the table's new form.
- The migration's own proof: no numbered ADR file is left, every link resolves, the blob list
  matches, and a search for `ADR-[0-9]{4}` finds only references to other repositories.

## Out of scope

- NUC v0.12.0 and the `nuc` binary.
- The ten backlog entries; none of them is touched.
- Sending a template finding to `hivevm/nuc`.
- CI jobs on macOS or Windows runners.
- A normalization of Forager's record beyond identifiers: no ADR is restated, merged, or
  dropped, and the specification stays as it reads.
- The design revision, which runs after the last step and closes this spec.

## ADR impact

- ADR-0004 decides the take-up. Its clauses move to NUC-releases-are-semver-tags with the
  migration.
- The migration replaces ADR-0001 to ADR-0005 and the template's part of ADR-0009 with the
  `NUC-` base. Each clause Forager added becomes a `proposed` ADR in the v0.9.0 step, accepted by
  the maintainer before the step merges.
- ADR-0006, the harness ADR, keeps every ADR with the maintainer, so no step runs unattended.
- The v0.10.0 step checks each ADR that names a check of `check-docs.sh` by number, or cites
  clause 7 or 9 of NUC-checks-are-bash or clause 3 of NUC-design-is-revised. An ADR that no
  longer holds against the wider clause is superseded.
