# ADR-0017: C's and MapServer's small ports, the `(int)` cast among them, live once in `style-ir`

- **Status:** 🟢 accepted
- **Date:** 2026-11-13
- **Deciders:** Markus Brigl
- **Applies to:** the ports of C's `atof`, `strtod`, `atoi`, `(int)` cast of a `double`, and string end, and of MapServer's `MS_NINT`, `MS_MIN`, `MS_MAX`, `MS_DEG_TO_RAD`, `msRectOverlap`, and `msStringSplit`, in `crates/style-ir`, `crates/expr`, `crates/mapfile`, `crates/engine`, `crates/symbolize`, `crates/label`, and `crates/adapter-shapefile`; every conversion of a number that may truncate in `crates/engine`, `crates/symbolize`, and `crates/label`

## Context

MapServer reads numbers with the C runtime: `atof` and `strtod` for a literal, a bound value, and a
WKT coordinate, `atoi` for an integer binding, `MS_NINT` to round one. It converts a `double` to an
`int` with a C cast in many places Forager ports, and it uses the small macros and helpers of
`mapserver.h` throughout. The frontend, the evaluator, the engine, the symbolization, the label
layout, and the adapters each need some of them, and
[ADR-0008](0008-ports-and-adapters-crates.md) gives the frontend no edge to `expr`.

The design revisions found them ported several times apart, as each crate first needed them:

- **Reading numbers** (revision of 2026-10-31):
  - `atof` in full in `expr` (hexadecimal, `inf`, `nan`, correctly rounded), a second, decimal
    only, in the frontend's `expression.rs`, which reads a literal and folds a numeric `IN` list at
    load, and a third reading of where `strtod` differs from Rust's parser in the frontend's
    `wkt.rs`. `([n] IN "0x10")` read 0 in the frontend and 16 in `expr`.
  - `MS_NINT` three times with three behaviours past the range of a C `long`: saturating in the
    engine's classes, keeping the low bits in its labels, and failing as undefined in `expr`.
  - `expr` re-exported `atof` and `atoi` for the engine's bindings, which evaluate nothing, so the
    evaluator crate had become the C runtime of the engine without a decision saying so.
- **The `(int)` cast** (revisions of 2026-10-31 and 2026-11-07). The x86-64 build at the pin
  compiles it to `cvttsd2si`: it truncates toward zero and gives `INT_MIN` past an `int` and for
  NaN. Rust's `as` saturates instead, and gives 0 for NaN. #153 first held `MAXLENGTH` with `as`,
  and review caught it. The engine's `int_cast` (`crates/engine/src/label_binding.rs`) ported the
  cast right, but about a dozen sites in `engine`, `label`, `symbolize`, `expr`, and `mapfile`
  wrote `as i32` or `.trunc()` in its place: a label's `OFFSET` and `BUFFER`, a layer's `OPACITY`
  and `MAXFEATURES`, a line's length in a label's layout, a marker's size, a hatch's box, the
  padding of `GEOMTRANSFORM bbox`, and `expr`'s `modulo`. Each diverges from the pin only past an
  `int`, which no corpus case reaches, so only a check would see the next one.
- **The small ports.** `MS_MIN` and `MS_MAX` keep their second operand where a comparison fails
  (NaN). They were written three times as functions and five times inline. `MS_DEG_TO_RAD` was
  written twice. A C string's end at its first NUL was written five times, `msRectOverlap` four
  times, and `msStringSplit` twice.

The convention that a port from MapServer is read in its source and each branch tested is defeated
by copies: a branch fixed in one copy stays wrong in the others. 1eec7a6 moved the symbolization of
a style, with its calls of `ms_nint`, from `crates/engine` into `crates/symbolize`, so a change
there must be routed to this decision by its *Applies to*.

Forces: Q-4 (the same reading and conversion of a number everywhere MapServer reads or converts it
the same); the edges of [ADR-0008](0008-ports-and-adapters-crates.md), which permit `label` to
depend on `style-ir`; [ADR-0016](0016-expression-evaluation.md), which places C's functions that
only `expr` needs in `expr`; [ADR-0007](0007-rust-toolchain.md), which holds the workspace's lints.

The maintainer settled these questions:

- The module lives in `style-ir`, with no new crate.
- The frontend keeps folding a numeric `IN` list at load, with that module's `atof`. The pin reads
  each item with `atof` as it evaluates (`math_exp IN string_exp` in `src/mapparser.y`), and `atof`
  of a literal gives the same value every time. An item it reads as no finite number stays
  refused, as JSON cannot carry it.
- `MS_NINT` is `lrint` in the pin's Linux build (`src/mapserver.h` with `HAVE_LRINT`), which on
  x86-64 gives `LONG_MIN` past a `long` and for NaN. The basis is x86-64, where the references are
  rendered ([ADR-0033](0033-noise-measurement.md)). glibc's `lrint` on AArch64 saturates instead,
  which is not the reference's behaviour.
- On 2026-11-13: the `(int)` cast, the macros, and the helpers join the same module, and `label`
  gains the edge to `style-ir` for them.

Today the module is `crates/style-ir/src/c.rs`. The frontend, `expr`, the engine, `symbolize`,
`label`, `render-agg`, and the adapters import from it, and `expr` keeps only `printf` and
`strptime`.

## Decision

We will port C's and MapServer's small functions and macros once, in the module `c` of
`crates/style-ir`, and call them there from every crate.

- **Reading numbers.** The module ports `atof`, `strtod`'s reading of a number's end, `atoi`,
  `lrint`, and `MS_NINT`:
  - `lrint` gives a 64-bit integer, and `i64::MIN`, x86-64's `LONG_MIN`, past the range and for
    NaN.
  - `ms_nint` gives the low 32 bits of that `lrint`, as GCC narrows it into an `int`.
  - `expr`'s `ROUND` keeps the `long` of `lrint` as a double.
  - The frontend folds a numeric `IN` list at load with the module's `atof`. It refuses a member
    of the list that reads as no finite number.
- **The `(int)` cast.** The module ports C's `(int)` cast of a `double` as `int_cast`. It
  truncates toward zero, and gives `i32::MIN`, x86-64's `INT_MIN`, past an `int` and for NaN.
  Every site that ports a C `(int)` cast, or a C assignment of a `double` to an `int`, must call
  it. `expr` keeps an operand of `%` past an `int` undefined, as
  [ADR-0016](0016-expression-evaluation.md) decides for C's undefined behaviour, and casts it
  with `int_cast` within an `int`.
- **The macros.** The module ports `MS_MIN` and `MS_MAX` as `ms_min` and `ms_max`, each keeping
  its second operand where the comparison fails, and `MS_DEG_TO_RAD` as one constant.
- **The helpers.** The module ports `msRectOverlap` over a rectangle's four bounds,
  `msStringSplit` over a separator, and a C string's end at its first NUL.
- **The edge.** `crates/label` depends on `crates/style-ir` for these ports.
- **The lint.** `crates/engine`, `crates/symbolize`, and `crates/label` deny clippy's
  `cast_possible_truncation` in their library code. A conversion between integers must use
  `try_from`, or carry an `#[expect]` with its reason where its bound is held on purpose (Q-9).

**Out of scope:**

- `printf` and `strptime`, which only `expr` needs and which stay in `expr`
  ([ADR-0016](0016-expression-evaluation.md));
- the lexer's reading of a number token, which reads with Rust's parser and refuses what is not
  finite;
- C's conversions of a `double` to unsigned or 64-bit types, such as AGG's `uround`: each site
  writes its own, with its reason;
- the lint in crates other than the three above.

## Alternatives considered

- **A leaf crate `c-runtime`** — the cleanest responsibility, but a new crate and edge in the
  dependency rule of [ADR-0008](0008-ports-and-adapters-crates.md) for a module of a few hundred
  lines. The cast, the macros, and the helpers are as small as the readings of numbers.
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
- **The readings of numbers alone in the module, as decided on 2026-10-31** — `atof`, `strtod`,
  `atoi`, `lrint`, and `MS_NINT`, for the frontend, `expr`, and the engine. It named neither the
  `(int)` cast nor the macros and helpers, which stayed copied, and its *Applies to* missed
  `crates/symbolize` once the symbolization moved there.
- **A second record for the `(int)` cast and the macros, beside the one for reading numbers** —
  two records for one module and one rule, which a reader has to put together again.
- **A module beside `c` in `style-ir`, `mapserver`, for the macros and helpers of `mapserver.h`**
  — it separates what C gives from what MapServer adds. It doubles the places a session looks for
  a port, and `MS_NINT`, a macro of `mapserver.h`, already lives in `c`.
- **The macros where their users are, each crate with one copy** — no new edge for `label`, but
  three copies of `ms_min` stay, with their NaN case tested three times.
- **A test in `crates/arch-test` that refuses `as i32` by text** — it cannot tell a `f64` from an
  integer operand, so it either refuses the index casts too or lets a float cast through under a
  different spelling. The lint knows the types.
- **The lint in the whole workspace** — about 30 more sites in the renderers, the encoder, and
  the IR, most of them bounded pixel and index casts of Q-3 and Q-9, for no port of a C cast.
  Each crate may take the lint up later.
- **Saturate, as Rust's `as` does** — the conversion of glibc on AArch64 and of no reference
  image, and the behaviour #153 was caught with.

## Sources / Prior art

- glibc's `strtod` and `atof`, and MapServer's `MS_NINT` at the pin, as the ports cite them in
  their comments; read at `rel-8-6-6`: `bindIntegerAttribute` and `bindStyle` in `src/maputil.c`,
  `ROUND` and `IN` in `src/mapparser.y`, `MS_NINT` in `src/mapserver.h`, and `HAVE_LRINT` in
  `CMakeLists.txt` and `src/mapserver-config.h.in`; glibc's `lrint` checked on x86-64.
- MapServer at `rel-8-6-6`: `MS_MIN`, `MS_MAX`, `MS_DEG_TO_RAD`, and `MS_NINT` in
  `src/mapserver.h`; `msRectOverlap` and `msStringSplit` where the ports cite them.
- Intel's *Software Developer's Manual*, `CVTTSD2SI`: the "integer indefinite" value,
  `0x80000000`, for a source that is NaN or out of range. GCC compiles a C `(int)` cast of a
  `double` on x86-64 to that instruction.
- The Rust Reference, *Numeric cast*: a cast from a float to an integer rounds toward zero,
  saturates past the range, and gives 0 for NaN.
- Clippy's `cast_possible_truncation`.
- The design revisions of 2026-10-31 and 2026-11-07, their surveys of `crates/mapfile`,
  `crates/expr`, `crates/engine`, `crates/symbolize`, and `crates/label`.

## Consequences

- Positive: one port per C function and macro, each branch tested once; the frontend and the
  evaluator read `0x10` alike; `expr`'s public surface is evaluation.
- Positive: a new `(int)` cast that is written as `as` fails the lint; a change in `symbolize` is
  routed to this ADR.
- Negative / trade-offs: the IR crate carries behaviour that is not the data model. Every crate
  but `label` depended on it already, and `label` gains an edge to `style-ir`. About 30 integer
  conversions in the three crates are rewritten with `try_from` or carry an `#[expect]`.
- Follow-ups: whether `expr` and `mapfile` take up the lint as well.

## Enforcement

- The test `c_number_reading_is_defined_only_in_the_module_of_style_ir` in
  `crates/arch-test/tests/c_numbers.rs` cites this ADR with `Verifies:`. It refuses a function
  named `atof`, `atoi`, `strtod`, `ms_nint`, `lrint`, `int_cast`, `ms_min`, `ms_max`,
  `rect_overlap`, `ms_string_split`, `c_bytes`, or `c_str`, or one whose name starts with `lrint_`
  or `ms_nint_`, defined outside `crates/style-ir/src/c.rs`. It refuses `PI / 180` in the
  workspace's code outside that file.
- The module's tests cover each branch of the ports and cite this ADR with `Verifies:`. Among
  them are the NaN case of `ms_min` and `ms_max`, and the cast past an `int` and of NaN.
- Tests in `crates/engine/tests/classes.rs` and `crates/engine/tests/labels.rs` cite this ADR for
  the rounding of bound values, and one in `crates/cli/tests/render.rs` for a numeric `IN` list
  that reads `0x10` as 16.
- The crate root `src/lib.rs` of `crates/engine`, `crates/symbolize`, and `crates/label` denies
  `clippy::cast_possible_truncation`. Their `Cargo.toml` inherits the workspace's lints, beside
  which Cargo takes no lint of a crate's own. `scripts/check-all.sh` runs clippy on them.

**Not mechanically decidable:** a port written inline under another name escapes the test. A
rounding may stand for `MS_NINT`, a comparison for `MS_MAX`, or a loop for a split. Review
catches it.
