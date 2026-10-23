# Agent Guide

> **The single rule file of this project** ([ADR-0001](docs/adr/0001-agent-governance-model.md)).
> Every working rule lives here. Where another document repeats one, this wording governs. The
> file belongs to the template and is not edited per project. Project conventions go into
> [`docs/CONVENTIONS.md`](docs/CONVENTIONS.md), into checks, and into ADRs. A procedure is a skill
> under [`.agents/skills/`](.agents/skills/). It cites a rule here and states none of its own
> ([ADR-0003](docs/adr/0003-procedures-as-skills.md)).

## 1. Principles

The human decides *why* and *what*; the agent works out *how*. A how that binds future choices is
the human's too. It is recorded with its why in an ADR ([§3](#3-adr-rules)). Five principles win
over speed and cleverness:

- **Simplicity.** Build the simplest thing that satisfies the specification. Add a dependency or
  an abstraction only for a present need. Removing code is progress. Justify warranted complexity
  in an ADR.
- **Evolvability.** Decide the essential architecture at the start, and keep it open to change.
  To follow a technology, supersede the ADR that chose it; never work around it.
- **Proportionality.** Ceremony scales with how hard a change is to reverse. Say it out loud when
  you skip ceremony a reader would expect.
- **Reflection.** Weigh alternatives and consequences, and write the reasoning down.
- **Critical stance.** Take nothing at face value: not the human's framing, not your own earlier
  output, not the existing code. Verify against the specification, the ADRs, the code, and
  authoritative sources. Put disagreement, with its evidence, into the pull request. Put what you
  could not confirm there too; "none" is a claim the reviewer checks.

## 2. Start here

Before any non-trivial work, read:

1. [`docs/SPECIFICATION.md`](docs/SPECIFICATION.md): problem, goals, success criteria, in the
   vocabulary of [`docs/GLOSSARY.md`](docs/GLOSSARY.md).
2. [`docs/adr/README.md`](docs/adr/README.md): the ADR index. **Accepted ADRs are binding.** Read
   every one whose *Applies to* names what your change touches.
   [`scripts/sensor-binding-adrs.sh`](scripts/sensor-binding-adrs.sh) lists those that name one
   of its paths. An *Applies to* written in prose you match yourself. Of each binding ADR, read
   the header, the *Decision*, and the *Enforcement*. Read the rest when the change questions,
   revises, or supersedes it. A `proposed` ADR binds the work that implements it. Rejected and
   superseded ADRs explain *why*; they are not rules.
3. [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md): the system as it stands today. Where it and an
   accepted ADR disagree, the ADR is right.

Scaffold text in those documents is the first work: a placeholder title, an empty section, a
`TODO`. Say so, and elaborate it with the human before implementing.

## 3. ADR rules

Authority runs **specification → accepted ADRs → task**. The mechanics of the record are in
[`docs/adr/README.md`](docs/adr/README.md). Working rules originate here and nowhere else.
Another document cites a section by its anchor (`[AGENTS.md §3](AGENTS.md#3-adr-rules)`) and
restates nothing. A rule this file does not state is not a rule of this project.

1. **Create an ADR before any architecture-relevant decision.** Such a decision picks a
   dependency or framework, a public interface, a persistence or synchronization strategy, or a
   protocol or data format. In general, it is anything that constrains future technology choices.
   One decision per ADR. A cross-cutting ADR is only for an integration, where the decision *is*
   the interplay. The skill `propose-adr` carries a catalogue of concepts. When a change takes a
   decision the catalogue has an entry for and no ADR in force decides, propose the entry's
   fitting concepts before implementing, the simplest among them.
2. **Calibrate.** An ADR is for a decision that is *costly to reverse*, that *constrains future
   choices*, or that **a reader without this conversation would take for a mistake**. Record the
   last however cheap it is to reverse. No ADR is needed for implementing within an ADR in force,
   for bug fixes, interface-preserving refactorings, tests, docs, formatting, or dev-only tooling.
   When genuinely unsure, prefer a short `proposed` ADR over a silent decision.
3. **Develop the ADR with the human, critically.** Submit it with status `proposed` **in a
   pull request of ADRs only**, never mixed with implementation. ADRs the same human decides may
   travel together in one pull request, as the founding decisions of a new project do. Each stays
   one decision. One the human sends back leaves that pull request for one of its own. Skill:
   `propose-adr`. The normalization of rule 5 is the one exception: its pull request carries the
   restated ADRs and the references that move with them.
4. **After human review, implement while the ADR is still `proposed`.** Findings return as
   revisions, each in its own ADR-only pull request. **Only a human changes the status.** The
   flip to `accepted` may land with the implementation.
5. **Accepted ADRs are binding and immutable.** Never violate or work around one; change it only
   by superseding it. Outside a normalization, never rewrite or delete one. Two edits are
   permitted, both by a human. One flips the `Status` line. The other adds the project's decider
   to the `Deciders` line of an ADR inherited from the template, when the project accepts it as
   its own. ADR numbers are permanent between normalizations. The template repository corrects
   its own ADRs in place ([ADR-0005](docs/adr/0005-template-releases.md)). If the specification
   and an ADR conflict, the specification wins. Raise the conflict; never choose silently.

   **A normalization restates the record, only on a human's instruction.** The specification is
   restated first, under rule 7. The ADRs it invalidates are superseded next. Then the ADRs in
   force are rewritten as a new series in reading order, the inherited ones among them, and
   superseded and rejected ADRs are deleted. A normalization changes no decision. It is rare, at a
   milestone or a release, because it makes the numbers cited outside the tree harder to read.
   Skill: `normalize`.
6. **Use the vocabulary of [`docs/GLOSSARY.md`](docs/GLOSSARY.md)** in code, comments, and
   documentation. Add a term in the same change that settles it. A missing entry is a gap to
   close there, not a word to invent.
7. **A change to the specification is a decision, not an edit.** Only a human lands one. It names
   every accepted ADR whose basis it moves. An agent drafts specification text. It never lands
   the change and never judges alone that no ADR is affected.

## 4. Working style

- **Reply to the human in their own language.** Everything else is English: code, comments,
  commits, docs, ADRs, pull requests.
- **Prefer small, reviewable changes.** Plan and explore before a larger one. Commits and pull
  requests name the ADR they implement (`Implements ADR-NNNN`).
- **Cut work into vertical slices.** Each slice is a thin, tested path through every layer it
  touches. The first slice through new ground is the tracer bullet. The one exception is a wide
  mechanical refactor: expand, migrate in batches, contract, every step green.
- **Write to [`docs/CONVENTIONS.md`](docs/CONVENTIONS.md).** Settle a new judgement call there,
  in the same change that raised it. A judgement call a check can decide becomes the check
  ([§5](#5-quality-bar--definition-of-done)). One that constrains future choices becomes an ADR.
- **Plan work larger than one session in the tree, under `docs/plan/`.** A feature spec is cut
  into tracer-bullet tickets. The change that lands a ticket's work deletes its file
  ([ADR-0006](docs/adr/0006-feature-layer.md)). Skill: `plan-feature`. The skill `work-tickets`
  works the tickets, one at a time or in parallel.
- **Work with the human:** small steps, reasoning surfaced, feedback sought early. A task runs
  unattended only when its ticket answers for all of the following:
  - a test decides its criteria;
  - its scope is isolated, and a wrong result costs one revert;
  - it touches nothing the harness ADR keeps with the human, and it fits under that ADR's cap on
    open agent work;
  - it says how a wrong result is noticed and undone, and what proves it right;
  - it names the stop past which the session hands off instead of trying on.

  The harness ADR is the project's founding decision on who decides, which agents work, and how
  much agent work may be open at once.
- **Keep the context lean.** Subagents return conclusions, not file dumps. Alignment,
  implementation, and review each start in a fresh session with only what they need. Never start
  one by compacting the old session. The implementing session loads what its ticket's *Read
  first* names. A degraded session hands off.
- **Session artifacts are not tracked.** Session plans, handoff notes, and scratch files live under
  `.scratch/` (gitignored) or outside the tree. What has to outlive the session becomes a ticket,
  a `proposed` ADR, or a change to one of the six documents of ADR-0001.
- **Research before a decision that is costly to reverse,** external sources included. Cite them
  in the ADR. Below that bar, research what you are actually unsure of. Never pass off recalled
  interfaces, versions, or defaults as verified.
- **Settle scope and intent with the human first when they are still open.** The trigger is
  disagreement about *why* and *what*, not size. **When in doubt, ask the human before
  implementing.**

## 5. Quality bar & Definition of Done

- **A change is done only when it builds, its tests pass, and linters and formatters are clean,**
  locally and in CI. Never hand off or propose merging red.
- **Run [`scripts/check-all.sh`](scripts/check-all.sh) before pushing.** It runs the repository's
  checks and the build, test, and lint commands of the README's **Build, Test & Run** section.
  CI runs the same on every pull request.
- **New behaviour ships with tests; a bug fix ships with a regression test** that fails before the
  fix and passes after. Genuinely untestable: say so and why.
- **The suite is the contract an implementation is held to.** So it tests through interfaces the
  implementation can change behind. Before changing what exists, pin the behaviour that must
  stay, tests first where none exist. A rewrite or migration is done when the old suite passes
  against the new implementation.
- **Every accepted ADR, and every specification criterion (`G-n`, `Q-n`) a change meets, is
  cited by a test** with a `Verifies: <id>` marker
  ([ADR-0002](docs/adr/0002-decisions-verified-by-tests.md)). An ADR no test can decide says so
  in its *Enforcement* section. A criterion no test cites yet is pending work.
- **Never weaken the suite to make it pass. A flaky test is a defect, not noise:** no retries, no
  re-running until green.
- **Review in a fresh context before handing off.** The session that wrote a change never reviews
  it. The reviewer gets the diff, the task, the doubts, and the standards, never the reasoning.
  It reports on two axes: does the change do what was asked, and does it meet this bar,
  simplicity first. Findings are fixed before the pull request opens. Every existing test the
  diff changes or removes needs a reason. Skill: `review`.
- **A feature spec closes only after a design revision.** A fresh session revisits the modules
  its tickets touched, the ADRs that bind them, and the criteria they serve. It files what it
  finds as tickets, checks, a `proposed` ADR, or a drafted specification change. With no spec
  open, a revision runs when the revision sensor says it is due
  ([ADR-0006](docs/adr/0006-feature-layer.md)). A sensor is a script that reports to the reviewer
  and fails nothing. Skill: `design-revision`.
- **A mistake a check could have caught becomes a check,** shipped with the fix. A rule in prose
  is the fallback for what no check can decide. The design revision drops what prevented nothing.
- **Keep the diff releasable.** No commented-out code, stray debug output, or `TODO` standing in
  for a decision. Unfinished work is a ticket or a `proposed` ADR, not hidden in the code.
- **A change to the structure updates [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)** in the same
  change.

## 6. Project rules

- **Commit freely on a branch, never on `main`. A human decides what leaves the machine.**
  Pushing, `gh`, and anything that rewrites or discards history (`reset`, `rebase`, `merge`,
  `restore`, `clean`) wait for an instruction. The instruction names the action and implies no
  next one. A standing instruction may cover one push only: the claim on a ticket across clones,
  where the harness ADR grants it. A claim reserves a ticket by creating its branch, with an
  empty commit and nothing else ([ADR-0006](docs/adr/0006-feature-layer.md)). Its push skips the
  pre-push hook. The git hooks under [`.githooks/`](.githooks/) refuse a commit on `main`. They
  refuse a push while the checks of [§5](#5-quality-bar--definition-of-done) are red.
- **Authenticate `gh` through its web flow;** a human enters the one-time code. Never request,
  store, or hard-code personal access tokens.
- **Changes reach `main` through a pull request.** What no file can enforce is listed under
  [Repository settings](README.md#repository-settings).

## 7. Secrets

- **Never write a secret into a tracked file, commit message, ADR, log, or CI output.** Secrets live
  in environment variables, gitignored `.env*` files, or GitHub Actions secrets. A placeholder in a
  tracked `.env.example` is not a secret.
- **A secret that reaches git is compromised.** Rotation comes first and is the human's to perform.
  Removing it from the branch tip is cleanup, not remediation. Tell the human immediately.
- No check enforces either rule. [`SECURITY.md`](SECURITY.md) says what the repository's
  configuration does about secrets and where it stops.
