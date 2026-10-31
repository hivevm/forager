# Conventions — Forager

> How this project writes code and tests **where no check decides it**: the judgement calls. What
> a module may know, what a test asserts and never does, the shape of an error, how a thing is
> named when the glossary has no word for it. Whoever settles a convention updates this file
> inline, an agent included, in the same change as the review or code that settled it. The
> reviewer always has it ([ADR-0001](adr/0001-agent-governance-model.md)).

An entry says in a sentence or two what is done and why; the why tells a convention from a habit.
A convention a check can decide is the check, not an entry. One that constrains future choices is
an ADR. What the golden path already shows needs none. Few entries that hold beat many that are
skimmed. The design revision drops what prevented nothing
([`AGENTS.md` §5](../AGENTS.md#5-quality-bar--definition-of-done)).

- **Diagnostic codes are kebab-case and stable** — tests and tools match on `code`, never on the
  message, so messages can be reworded freely and a code once used keeps its meaning.
- **A test that expects a refusal asserts its reason** — the diagnostic's code in a Rust test,
  the refusal's text in a script, because any other failure, a missing tool or a broken fixture,
  would otherwise pass for the refusal.
- **A `Verifies:` marker claims every *shall* of its criterion** — a test cites a goal, a quality
  goal, or an ADR only when it decides all the criterion asks for, or a clause of the ADR's
  *Decision*, never because a test beside it does, such as G-5's image *and* the HTML example
  that shows it; a test that decides a part cites nothing and the criterion stays pending, unless
  the criterion asks for a test per item itself, as Q-8 and Q-9 do. The
  traceability check counts a marker as met and cannot tell how much its test decides.
- **Behaviour ported from MapServer or PROJ is read in their source, and its branches are
  tested** — a rule that follows their code names the function it follows, read at the pinned
  release, never as a ticket or memory describes it, and each branch of that function the port
  takes over has a test of its own: the msautotest corpus passing says nothing of a branch no
  file of it takes, which is where such a port goes silently wrong. A spec or ticket that says
  what MapServer does names the function and lines it read at the pin, or asks instead of
  stating; and a port in the engine first reads the parameter's page under `docs/semantics/`,
  which says what part of the C the frontend has already carried into the IR, so that no part is
  ported twice.
- **A reader of untrusted bytes allocates nothing from a count or a size the bytes state before
  checking it against the bytes present** — a header's record count, a part count, a length
  field: each is compared with what the input holds, in checked arithmetic, before anything is
  reserved for it, so that memory follows the input's size and Q-9's limit on it means something.
  Its test states counts past the bytes present and bounds the peak allocation under a counting
  allocator, as the shapefile adapter's tests do; the same test, run against a crate's reader,
  shows whether that reader keeps to it ([ADR-0024](adr/0024-shapefile-reader.md)).
- **A reader that skips what it cannot read fails closed** — a condition it cannot read becomes
  one that never matches, never one that is absent, a value it cannot carry over leaves what it
  belongs to undrawn, never at the default an absent value takes, and what it skips is still held
  to the limits of Q-9; a lenient path is a second way in, and what it leaves out must not widen
  what a map draws or what an input may cost.
- **What a render comes to hold per label or per feature is measured where it starts to** — the
  change that makes a cached label or a feature hold something more, a style, a marker, a bound,
  adds the case that holds it to the memory test of Q-9, such as
  `crates/engine/tests/label_memory.rs`, because that test measures only the maps it builds and
  says nothing of a field none of them sets.
- **A script that points git at a repository runs git without git's repository variables** — a
  test, a self-test, or a script under `scripts/` that drives git on a repository it names, a
  fixture or a checkout, removes what `git rev-parse --local-env-vars` names from git's
  environment, because a caller that set `GIT_DIR`, such as a hook in a linked worktree, otherwise
  turns every such command on the real repository. `scripts/test-fixture-git-env.sh` checks the
  scripts under `scripts/`; a Rust test that drives git, such as `git_in` of
  `crates/style-ir/tests/schema.rs`, is held by this entry alone.
- **A ticket's pull request carries what its criteria ask for and nothing else** — a change of
  another behaviour met on the way, such as a renderer's blending found under a ticket for
  opacity, leaves for a ticket or a proposed ADR of its own and its own pull request, because
  folded into the ticket it is reviewed as the ticket's means and not as the decision it is.
- **A reference image of Forager's own is re-blessed only in the change that moved it, which says
  how far** — the commit that rewrites images with `FORAGER_BLESS` names the behaviour that moved
  them, the largest move per channel, and the conformance outcome of the same change, because an
  image blessed from the renderer under test holds nothing of that change and only the
  conformance suite still judges it.
- **A test holds a value it states, never one the code under test computes** — the expected value
  is a literal, read from MapServer's source or a reference, not the engine's own constant or
  function, and a relation between two outputs of the same code, one doubled at twice the
  resolution, one ordered before another, comes beside a pinned value and never in place of it,
  because a wrong implementation that both sides share passes the relation and the self-derived
  value alike.
- **A commit typed `refactor` changes nothing a test can see** — a behaviour it fixes or moves on
  the way, such as a bound that now measures what the renderer draws, is a commit of its own typed
  `fix` with its regression test, because the type tells the reviewer and the history which change
  to read as a decision.
