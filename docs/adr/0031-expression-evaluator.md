# ADR-0031: The core crate `expr` evaluates the IR's expressions as MapServer's parser does, with patterns matched by the `regex` crate

- **Status:** 🟡 proposed
- **Date:** 2026-10-30
- **Deciders:** Markus Brigl
- **Applies to:** `crates/expr`, the selection of classes and the per-feature properties of `crates/engine`, and every crate that evaluates an expression of the style IR

## Context

The style IR holds every class `EXPRESSION`, layer `FILTER`, label `EXPRESSION`, and bound or
computed property as a CQL2 tree (ADR-0030); the engine does not evaluate any of them yet, so it
draws every feature with the first class. Of the 58 geometry cases in the harness's scope, about
nine fail on that alone. ADR-0009 reserves a core crate `expr` for evaluation and the pushdown
planner, synchronous, without I/O, depending on `style-ir` only. A feature's properties are the
port's typed values (ADR-0026).

MapServer at `rel-8-6-6` evaluates an expression in `msEvalExpression` (`src/maputil.c`) and,
for one in parentheses, in the grammar of `src/mapparser.y`:

- **Types.** A number is a C `double`; an attribute read as a number goes through `atof`. Strings
  compare with `strcmp`, byte by byte, and `=*` with `strcasecmp`. A logical value asked for as a
  string is `"true"` or `"false"`.
- **Lists.** `string IN "a,b"` compares the string to each comma-separated item with `strcmp`, no
  item trimmed; `number IN "1,2"` compares the number to `atof` of each item.
- **Arithmetic.** `+ - * /` on doubles; `/` by zero is a parse error, so the expression fails;
  `%` is `(int)a % (int)b`, which C leaves undefined for a zero divisor; `^` is `pow`.
- **Patterns.** `~` and `~*`, and a `/re/` expression, compile the pattern with `ms_regcomp`,
  through PCRE2's POSIX interface at the pin (ADR-0030), `REG_ICASE` for the `i` forms, and match
  with `ms_regexec`; a pattern that does not compile is an error, and the expression fails.
- **Failure.** An expression that fails to evaluate does not match.

The `regex` crate (1.13.1, MIT OR Apache-2.0, updated 2026-07-15) reads a syntax close to PCRE2's,
without back-references, look-around, or atomic groups, and bounds the size of a compiled
pattern; it builds for `wasm32-unknown-unknown` without a `-sys` crate.

## Decision

We will evaluate the IR's expressions in the core crate `expr`, against a feature's properties the
engine hands it through a lookup of its own, as `msEvalExpression` and `mapparser.y` at the pin
evaluate them, and match `regexp_like` with the `regex` crate as PCRE2's POSIX interface matches.

- **Values.** A number is an `f64`, a string bytes compared as `strcmp` compares them, `casei` as
  `strcasecmp` does in ASCII, a time as `msParseTime` reads it. `to_number` is `atof` of the
  property's text, a property that is absent or null reading as the empty text; `to_string` of a
  condition is `"true"` or `"false"`.
- **Operators.** Each operator and function of ADR-0030 is ported from `mapparser.y` or
  `maputil.c` with a test of its own, `in` against a list as the grammar compares, `%` as
  `(int)a % (int)b`.
- **Failure.** Where MapServer's evaluation fails, or where C leaves the result undefined, the
  expression does not match: a division or a `%` by zero, a pattern the `regex` crate refuses or
  reads otherwise than PCRE2 (back-references, look-around, atomic groups), a time `msParseTime`
  rejects. A pattern that cannot match is reported once per render with the class, filter, or
  label that carries it.
- **Patterns.** A pattern is compiled once per render, with the flag `i` as case-insensitive
  matching, and bounded by the `regex` crate's size limit at 1 MiB of compiled program (Q-9);
  only whether it matches is asked, as `REG_NOSUB` asks.
- **The interface.** `expr` reads properties through a trait it defines, which the engine
  implements over the port's properties, so `expr` depends on `style-ir` alone (ADR-0009).
- **The engine.** It draws a feature with the first class whose expression matches, after the
  layer's filter, and evaluates each bound or computed style property per feature.
- **Crates.** `regex` is used by `expr` alone, under ADR-0012.

**Out of scope:** pushdown of a filter to a source (G-7); geometry functions and spatial
predicates; label expressions, which the label cache evaluates with the labels spec.

## Alternatives considered

- **Evaluation inside the engine** — fewer crates, but tied to the asynchronous engine, and no
  place for the pushdown planner, which ADR-0009 gives `expr`.
- **A POSIX regular-expression engine** — `regcomp`'s syntax, but not the syntax MapServer reads
  at the pin, which is PCRE2's (ADR-0030).
- **A PCRE2 port or binding** — the exact syntax, but a binding links C, which the core may not
  (ADR-0009), and no maintained pure-Rust port covers it.
- **`fancy-regex`** — adds back-references and look-around over `regex`, with backtracking that
  can run long on a pattern from a mapfile, against Q-9.
- **Matching what `regex` cannot read anyway, by approximation** — a class drawn where MapServer
  would not, without a word.

## Sources / Prior art

- MapServer at `rel-8-6-6`: `src/maputil.c` (`msEvalExpression`, `msEvalRegex`),
  `src/mapparser.y` (comparison, `IN`, arithmetic, `%`, `/` by zero, `^`, the logical value as a
  string), `src/mapregex.c` (`ms_regcomp`, `ms_regexec`, `USE_PCRE2`); read 2026-10-30.
- crates.io, checked 2026-10-30: `regex` 1.13.1 (MIT OR Apache-2.0, updated 2026-07-15); it
  builds for `wasm32-unknown-unknown` with the pinned toolchain and no `-sys` crate.
- The `regex` crate's documentation of its syntax and of `RegexBuilder::size_limit`.

## Consequences

- Positive: classes, filters, and bound properties draw as MapServer draws them, so the cases
  that wait on them can pass, and the pushdown planner of G-7 has its crate.
- Positive: a pattern from a mapfile runs in time linear in its input, under a bounded size.
- Negative: a pattern with a back-reference, a look-around, or an atomic group never matches,
  where MapServer's PCRE2 would match; the report names it.
- Negative: `%` and `/` by zero never match, where MapServer may crash or fail.
- Follow-ups: pushdown (G-7); geometry functions when a case needs them.

## Enforcement

A test in `crates/arch-test` cites this ADR with `Verifies:` and fails when a crate other than
`expr` depends on `regex`, or when `expr` depends on a workspace crate other than `style-ir`.
Tests in `expr` cite it: each operator and function against values worked from MapServer's
source, `atof` and `strcmp` at their edges, `in` with untrimmed items, `%` and `/` by zero, a
pattern with the flag `i`, one past the size limit, and one with a back-reference. A test in
`engine` cites it: the first matching class, the filter, and a bound size per feature.
