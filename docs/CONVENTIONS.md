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
- **A `Verifies:` marker claims every *shall* of its criterion** — a test cites a goal or quality
  goal only when it decides all the criterion asks for, such as G-5's image *and* the HTML example
  that shows it; a test that decides a part cites nothing and the criterion stays pending, unless
  the criterion asks for a test per item itself, as Q-8 and Q-9 do. The
  traceability check counts a marker as met and cannot tell how much its test decides.
- **Logic ported from MapServer keeps MapServer's notice** — a file whose rules follow MapServer's
  C code names the source file and release and carries its copyright and permission notice at the
  top, as the specification's licence constraint requires.
