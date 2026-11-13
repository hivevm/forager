# ADR-0017: C's reading of numbers is ported once, in `style-ir`, for the frontend and the core alike

- **Status:** ⚪ superseded by ADR-0036
- **Date:** 2026-10-31
- **Deciders:** Markus Brigl
- **Applies to:** the ports of C's `atof`, `strtod`, `atoi`, and MapServer's `MS_NINT` in `crates/style-ir`, `crates/expr`, `crates/mapfile`, and `crates/engine`

## Context

MapServer reads numbers with the C runtime: `atof` and `strtod` for a literal, a bound value, and a
WKT coordinate, `atoi` for an integer binding, `MS_NINT` to round one. The frontend, the
evaluator, and the engine each need them, and [ADR-0008](0008-ports-and-adapters-crates.md) gives
the frontend no edge to `expr`. The design revision of 2026-10-31 found them ported several times
apart, as each crate first needed them:

- `atof` in full in `expr` (hexadecimal, `inf`, `nan`, correctly rounded), a second, decimal only,
  in the frontend's `expression.rs`, which reads a literal and folds a numeric `IN` list at load,
  and a third reading of where `strtod` differs from Rust's parser in the frontend's `wkt.rs`.
  `([n] IN "0x10")` read 0 in the frontend and 16 in `expr`.
- `MS_NINT` three times with three behaviours past the range of a C `long`: saturating in the
  engine's classes, keeping the low bits in its labels, and failing as undefined in `expr`.
- `expr` re-exported `atof` and `atoi` for the engine's bindings, which evaluate nothing, so the
  evaluator crate had become the C runtime of the engine without a decision saying so.

The convention that a port from MapServer is read in its source and each branch tested is defeated
by copies: a branch fixed in one copy stays wrong in the others. Forces: Q-4 (the same reading of a
number everywhere MapServer reads it the same); the edges of
[ADR-0008](0008-ports-and-adapters-crates.md); [ADR-0016](0016-expression-evaluation.md), which
places C's functions that `expr` needs in `expr`.

The maintainer settled three questions. The module lives in `style-ir`, with no new crate. The
frontend keeps folding a numeric `IN` list at load, with that module's `atof`: the pin reads each
item with `atof` as it evaluates (`math_exp IN string_exp` in `src/mapparser.y`), and `atof` of a
literal gives the same value every time; an item it reads as no finite number stays refused, as
JSON cannot carry it. `MS_NINT` is `lrint` in the pin's Linux build (`src/mapserver.h` with
`HAVE_LRINT`), which on x86-64 gives `LONG_MIN` past a `long` and for NaN. The basis is x86-64,
where the references are rendered ([ADR-0033](0033-noise-measurement.md)); glibc's `lrint` on
AArch64 saturates instead, which is not the reference's behaviour.

Today the module is `crates/style-ir/src/c.rs`, with `strtod`, `atof`, `atoi`, `lrint`, and
`ms_nint`; the frontend, `expr`, and the engine import from it, and `expr` keeps only `printf` and
`strptime`.

## Decision

We will port C's `atof`, `strtod`'s reading of a number's end, `atoi`, and MapServer's `MS_NINT`
once, in a module of `crates/style-ir`, and have the frontend, `expr`, and the engine call that
module instead of copies of their own.

- **`lrint`.** The module ports `lrint` to a 64-bit integer that gives `i64::MIN`, x86-64's
  `LONG_MIN`, past the range and for NaN.
- **`ms_nint`.** The module gives `MS_NINT` as the low 32 bits of that `lrint`, as GCC narrows it
  into an `int`.
- **`ROUND`.** `expr`'s `round` keeps the `long` of `lrint` as a double.
- **`IN`.** The frontend folds a numeric `IN` list at load with the module's `atof`. An item that
  reads as no finite number is refused.

**Out of scope:** `printf` and `strptime`, which only `expr` needs and which stay in `expr`
([ADR-0016](0016-expression-evaluation.md)); the lexer's reading of a number token, which reads
with Rust's parser and refuses what is not finite.

## Alternatives considered

- **A leaf crate `c-runtime`** — the cleanest responsibility, but a new crate and edge in the
  dependency rule of [ADR-0008](0008-ports-and-adapters-crates.md) for a module of a few hundred
  lines.
- **Keep the copies and test them against each other** — no move, and three ports to keep in step
  with each branch the pin takes.
- **Let the frontend depend on `expr`** — the frontend gains the evaluator and `regex` it never
  runs, against [ADR-0008](0008-ports-and-adapters-crates.md) and the rule of
  [ADR-0016](0016-expression-evaluation.md) that only `expr` uses `regex`.
- **Keep a numeric `IN` list as text and let `expr` read each item when it evaluates** — it would
  take the frontend's `atof` away without a new home, but the folded list means the same, since
  `atof` of a literal gives the same value every time.
- **`MS_NINT` saturating past a `long`** — what glibc's `lrint` does on AArch64, not on x86-64,
  where the references are rendered.

## Sources / Prior art

- glibc's `strtod` and `atof`, and MapServer's `MS_NINT` at the pin, as the ports cite them in
  their comments; read at `rel-8-6-6`: `bindIntegerAttribute` and `bindStyle` in `src/maputil.c`,
  `ROUND` and `IN` in `src/mapparser.y`, `MS_NINT` in `src/mapserver.h`, and `HAVE_LRINT` in
  `CMakeLists.txt` and `src/mapserver-config.h.in`; glibc's `lrint` checked on x86-64.
- The design revision of 2026-10-31, its survey of `crates/mapfile`, `crates/expr`, and
  `crates/engine`.

## Consequences

- Positive: one port per C function, each branch tested once; the frontend and the evaluator read
  `0x10` alike; `expr`'s public surface is evaluation.
- Negative / trade-offs: the IR crate carries behaviour that is not the data model; every crate
  depends on it already, so no edge is added.
- Follow-ups: none.

## Enforcement

The test `c_number_reading_is_defined_only_in_the_module_of_style_ir` in
`crates/arch-test/tests/c_numbers.rs` cites this ADR with `Verifies:`. It refuses a function named
`atof`, `atoi`, `strtod`, `ms_nint`, or `lrint`, or one whose name starts with `lrint_` or
`ms_nint_`, defined outside `crates/style-ir/src/c.rs`. The module's tests cover each branch of the
C functions it ports and cite this ADR with `Verifies:`. Tests in `crates/engine/tests/classes.rs`
and `crates/engine/tests/labels.rs` cite it for the rounding of bound values, and one in
`crates/cli/tests/render.rs` for a numeric `IN` list that reads `0x10` as 16.

**Not mechanically decidable:** a rounding written inline under another name escapes the test;
review catches it.
