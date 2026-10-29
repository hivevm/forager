# ADR-0031: The core crate `expr` evaluates the IR's expressions as MapServer's parser does, with patterns matched by the `regex` crate in byte mode

- **Status:** 🟡 proposed
- **Date:** 2026-10-29
- **Deciders:** Markus Brigl
- **Applies to:** `crates/expr`, the selection of classes and the per-feature properties of `crates/engine`, and every crate that evaluates an expression of the style IR

## Context

The style IR holds every class `EXPRESSION`, layer `FILTER`, and bound or computed property as a
CQL2 tree (ADR-0030); the engine evaluates none of them yet, so it draws every feature with the
first class. ADR-0009 reserves a core crate `expr` for evaluation and the pushdown planner,
synchronous, without I/O, depending on `style-ir` only. A feature's properties are the port's
typed values (ADR-0026).

MapServer at `rel-8-6-6` evaluates an expression in `msEvalExpression` (`src/maputil.c`) and,
for one in parentheses, in the grammar of `src/mapparser.y`:

- **Types.** A number is a C `double`; an attribute read as a number goes through `atof`. Strings
  compare with `strcmp`, byte by byte, and `=*` with `strcasecmp`. A logical value asked for as a
  string is `"true"` or `"false"`, a number `%g`. `AND`, `OR`, and `NOT` take a number as true
  when it is not 0. Times compare with `msTimeCompare`; a time `msParseTime` rejects is a parse
  error.
- **Lists.** `string IN "a,b"` compares the string to each comma-separated item with `strcmp`, no
  item trimmed; `number IN "1,2"` compares the number to `atof` of each item.
- **Arithmetic.** `+ - * /` on doubles; `/` by zero is a parse error; `%` is `(int)a % (int)b`,
  which C leaves undefined for a zero divisor; `^` is `pow`; a unary `-` returns its operand
  unchanged (`'-' math_exp %prec NEG { $$ = $2; }`).
- **Functions.** `length` is `strlen`, in bytes; `round(x)` is `MS_NINT(x)`, half to even, and
  `round(x, n)` is `MS_NINT(x / n) * n`; `upper`, `lower`, `initcap`, and `firstcap` change case
  byte by byte with C's `toupper` and `tolower`; `tostring(x, format)` is `msToString`, whose
  failure is a parse error; `commify` is `msCommifyString`.
- **Patterns.** `~` and `~*`, and a `/re/` expression, compile the pattern with `ms_regcomp`,
  through PCRE2's POSIX interface at the pin (ADR-0030), without `REG_UTF`: PCRE2 reads the
  pattern in byte mode, `.` one byte, `\d \w \s \b` and caseless matching ASCII only. A value
  that is empty never matches, before the pattern is compiled. A pattern that does not compile
  makes `~` false where it stands, as `mapparser.y` sets the term to false without an error, and
  makes a `/re/` class expression not match.
- **Failure.** A parse error makes the whole expression fail, and a failed expression does not
  match.

The `regex` crate (1.13.1, MIT OR Apache-2.0, updated 2026-07-15) has a byte-oriented matcher
(`regex::bytes`) whose Unicode features can be turned off, which makes classes, `.`, and case
folding ASCII and byte-wise; it bounds the size of a compiled pattern and builds for
`wasm32-unknown-unknown` without a `-sys` crate. Its syntax is close to PCRE2's, but it refuses
back-references, look-around, atomic groups, recursion and subroutine calls, `\Q…\E`, `\K`,
branch resets, comments `(?#…)`, `\h`, `\R`, `\e`, `\cX`, `\N`, octal escapes, and quantifiers
such as `a{,3}`; and it accepts a few patterns with another meaning than PCRE2's: possessive
quantifiers (`a*+`), `(?R)`, `\v`, and `$` before a final newline.

## Decision

We will evaluate the IR's expressions in the core crate `expr`, against a feature's properties the
engine hands it through a lookup of its own, as `msEvalExpression` and `mapparser.y` at the pin
evaluate them, and match `regexp_like` with the `regex` crate's byte matcher, its Unicode features
off, as PCRE2's POSIX interface matches in byte mode.

- **Values.** A number is an `f64`, a string bytes compared as `strcmp` compares them, `casei` as
  `strcasecmp` does in ASCII, a time as `msParseTime` reads it. `to_number` is `atof` of the
  property's text; `to_string` of a condition is `"true"` or `"false"`, of a number `%g`. A
  property that is absent or null reads as the empty text.
- **Operators and functions.** Each is ported from `mapparser.y`, `maputil.c`, and
  `mapstring.cpp` as the Context states it, with a test of its own: `in` against the list as the
  grammar compares, `%` as `(int)a % (int)b`, a unary minus that does not negate, `length` in
  bytes, `round` half to even, the case functions byte by byte in ASCII.
- **Patterns.** A pattern is compiled once per render with `regex::bytes::RegexBuilder`, its
  Unicode features off, the flag `i` as ASCII case-insensitive matching, and a size limit of
  1 MiB of compiled program (Q-9); only whether it matches is asked, as `REG_NOSUB` asks. An empty
  value does not match. A pattern the `regex` crate refuses, where it is no construct PCRE2 reads,
  is taken as one PCRE2 refuses too and makes the term false where it stands. A pattern that uses
  a construct PCRE2 reads and the crate refuses or reads otherwise (the lists of the Context, but
  `$`) makes the whole expression not match, and is reported once per render with the class or
  filter that carries it.
- **Failure.** Where MapServer's evaluation fails with a parse error, or where C leaves the result
  undefined, the whole expression does not match: a division or a `%` by zero, a `tostring` that
  fails, a time `msParseTime` rejects.
- **The interface.** `expr` reads properties through a trait it defines, which the engine
  implements over the port's properties.
- **The engine.** It draws a feature with the first class whose expression matches, after the
  layer's filter, and evaluates each bound or computed style property per feature.
- **Crates.** `regex` is used by `expr` alone, under ADR-0012.

**Out of scope:** pushdown of a filter to a source (G-7); geometry functions and spatial
predicates; a label's expression, which the label cache evaluates with the labels spec; whether an
attribute's name matches without regard to case and whether a missing attribute is an error, which
the engine's lookup decides with ticket 02 of the geometry spec.

## Alternatives considered

- **Evaluation inside the engine** — fewer crates, but tied to the asynchronous engine, and no
  place for the pushdown planner, which ADR-0009 gives `expr`.
- **The crate's Unicode mode** — its default, but `\d`, `.`, and case folding would match beyond
  ASCII where PCRE2 in byte mode does not, and `\w{100}` would pass the size limit.
- **A POSIX regular-expression engine** — `regcomp`'s syntax, but not the syntax MapServer reads
  at the pin, which is PCRE2's (ADR-0030).
- **A PCRE2 port or binding** — the exact syntax, but a binding links C, which the core may not
  (ADR-0009), and no maintained pure-Rust port covers it.
- **`fancy-regex`** — adds back-references and look-around over `regex`, with backtracking that
  can run long on a pattern from a mapfile, against Q-9.
- **Matching what the crate reads otherwise anyway** — a class drawn where MapServer would not,
  without a word.

## Sources / Prior art

- MapServer at `rel-8-6-6`: `src/maputil.c` (`msEvalExpression`, `msEvalRegex`, the empty value),
  `src/mapparser.y` (comparison, `IN`, arithmetic, `%`, `/` by zero, `^`, the unary minus, the
  functions, `~` and `~*` without `yyerror`, the logical value and the number as a string),
  `src/mapstring.cpp` (the case functions, `msCommifyString`), `src/mapregex.c` (`ms_regcomp`,
  `ms_regexec`, `USE_PCRE2`); PCRE2 10.42, `pcre2posix.h` (`REG_EXTENDED` 0, no `REG_UTF`); read
  2026-10-29.
- crates.io, checked 2026-10-29: `regex` 1.13.1 (MIT OR Apache-2.0, updated 2026-07-15); it
  builds for `wasm32-unknown-unknown` with the pinned toolchain and no `-sys` crate.
- The `regex` crate's documentation of its syntax, of `regex::bytes`, and of
  `RegexBuilder::size_limit`.

## Consequences

- Positive: classes, filters, and bound properties draw as MapServer draws them, so the cases
  that wait on them can pass, and the pushdown planner of G-7 has its crate.
- Positive: a pattern from a mapfile runs in time linear in its input, under a bounded size.
- Negative: a pattern with a construct the crate cannot match as PCRE2 does never matches, where
  MapServer's would; the report names it.
- Negative: `$` before a value's final newline does not match, where PCRE2's does; a value ending
  in a newline is rare in a feature's attributes.
- Negative: `%` and `/` by zero never match, where MapServer may crash or fail.
- Follow-ups: pushdown (G-7); geometry functions when a case needs them.

## Enforcement

A test in `crates/arch-test` cites this ADR with `Verifies:` and fails when a crate other than
`expr` depends on `regex`; the edges of `expr` are ADR-0009's test. Tests in `expr` cite it: each
operator and function against values worked from MapServer's source, `atof` and `strcmp` at their
edges, `in` with untrimmed items, the unary minus, `%` and `/` by zero, an empty value against a
pattern, `not(regexp_like(x, "("))`, a pattern with the flag `i` and with `\d` against non-ASCII
bytes, one past the size limit, a possessive quantifier and a back-reference reported once per
render for two features. A test in `engine` cites it: the first matching class, the filter, and a
bound size per feature.
