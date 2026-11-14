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
  *Decision*, never because a test beside it does; a test that decides a part cites nothing and
  the criterion stays pending, unless the criterion asks for a test per item itself, as Q-8 and
  Q-9 do, and, by the maintainer's ruling of 2026-11-08, G-13 per category and G-10 per case: a
  marker such as `Verifies: G-13, for the category text` decides only the item it names. The
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
  shows whether that reader keeps to it ([ADR-0021](adr/0021-shapefile-adapter.md)).
- **A change that gives a target of Q-3 a new reader or a new path brings the fuzz seed that
  reaches it** — a decoder, a parser, or a path of the renderer is fuzzed from the change that
  adds it, not from a ticket after it, because a fuzz target reaches only what its harness builds
  and its seeds lead to: the pixmap decoder of 1eec7a6 panicked in the png crate on a ragged
  `PLTE` until its target came in a later ticket, and the render target drew no pixmap through
  two changes that added resampling and tiled fills, until c39b845.
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
- **A count held to a bound of Q-9 stops at the bound** — a load or a render charges nothing more
  to a count once it has passed its bound, because a sum of `usize` wraps on `wasm32` and lets
  through what it bounds: the defaults of `VALIDATION` did so until 94610e9.
- **A bound of Q-9 reports once** — the first excess is the one diagnostic, because a check made
  per item repeats its error for each: the defaults of `VALIDATION` did so until 94610e9.
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
- **A commit typed `refactor` changes nothing a test can see** — a behaviour it moves on the way
  is a `fix` of its own, because the type tells the reviewer and the history which change to read
  as a decision. `scripts/sensor-refactor-tests.sh` lists a `refactor` commit that changes the
  line of an assertion or a `#[test]`. An expected value it changes elsewhere is the reviewer's to
  find: on a continuation line of a wrapped assertion, in a `let expected`, a fixture, or an image.
- **A comparison fails when there is nothing to compare** — a test that holds two results equal,
  two renders, a rendering and its reference, a record and a reader, first asserts that each side
  holds what it is meant to compare, a layer's features, an image, a label, because two empty
  results agree: a text case marked `labels-hold` that rendered nothing held its mark until
  b1f2c29, and a shapefile that did not read left both sides of G-9 empty until f3a9c8f.
- **A port cuts a value where C's declared type cuts it** — a value MapServer keeps in an `int`, a
  label's `SIZE`, its `MAXLENGTH`, a line's pen, is cut, narrowed, or bounded at the step C does,
  an implicit assignment to an `int` field included, and an overflow that ADR-0036 and ADR-0016 do
  not decide is decided and named beside the code. No check sees such a step in a port that writes
  no cast: `msComputeTextPath` assigns `glyph_size`, an `int` (`mapserver.h:2385`), three times
  (`maplabel.c:71-75`), and the work of ef639d7 first rounded it once, so `SIZE 23` at a scale
  factor of 0.5 drew at 12 where MapServer draws at 11, until its review found it.
  [ADR-0036](adr/0036-c-ports-in-style-ir.md) decides how a port carries out a C cast, a C
  assignment of a `double` to an `int`, and C's rounding, with the `INT_MIN` of the `(int)` cast
  and the low 32 bits of `MS_NINT`; [ADR-0016](adr/0016-expression-evaluation.md) decides the
  undefined behaviour of `expr`. Neither places the step where a value is cut, narrowed, or
  bounded.
- **A minimum or maximum over values that may be NaN decides the NaN first** — `f64::min` and
  `f64::max` return the other operand, so a fold or a bound drops a NaN without a trace: the label
  cache filed a box with a NaN edge at its point until f8fa86d, and the label comparison of the
  harness was found on 2026-11-10 holding such a box within its tolerance.
- **An ADR that decides how code is written is tried against the workspace's lints before it is
  proposed** — an attribute, an export, or a pattern it names is compiled once under
  `[workspace.lints]` and the ADRs that bind the crate, because ADR-0034 first decided exports of
  the C ABI, which need the `unsafe` that ADR-0025 denies outside `render-agg`, and was found
  unworkable only by its implementation.
- **A rule changes everywhere it is stated** — a change to a rule, a step, or a name searches
  the tree for its old wording and for every citation of its anchor, and corrects them in the
  same change. A link check sees the anchor, never a sentence that restates it.
- **A self-test asserts what it builds** — it takes its fixtures from the tree it copies or writes
  them itself, never from the template's own ADRs, scripts, or numbers, so it holds in a derived
  project and after a normalization.
