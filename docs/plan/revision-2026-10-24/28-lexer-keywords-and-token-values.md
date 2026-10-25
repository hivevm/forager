# Ticket: Read bare keywords and keyword choices as MapServer's lexer gives them

## What to build

Found by areas 1, 3, 4, and 5 of the port review (revision ticket 23, #66–#70). Two rules of
MapServer's lexer and `getSymbol` (`src/maplexer.l`, `src/mapfile.c` at `rel-8-6-6`) that the
frontend does not follow:

- **A bare word that is a mapfile keyword is no string.** `SYMBOL circle`, `CLASSITEM name`,
  `EXPRESSION line` lex as keyword tokens, and `getString`/`getSymbol` refuse them
  (`mapfile.c:264-273`): the load fails. `Token::string()` takes any bare word.
- **A keyword choice is compared by its token value.** `getSymbol` compares the lexer's numeric
  token, so tokens of equal value stand for each other: for `LINEJOIN`, `OFF`/`FALSE`/`INCHES`/
  `POINT` are `none` and `TRUE`/`ON`/`FEET`/`LINE` are `bevel`; for `LINECAP`, `POLYGON`/
  `DEFAULT`/`MILES`/`CENTER` are `butt`; for a label, `ALIGN on` is left and `SIZE dd` is tiny;
  in `SYMBOL`, `TYPE FALSE`/`OFF` is TrueType (`MS_TRUETYPE` is 0, `mapserver.h:583`). The
  frontend's `choice` aliases only `TRUE`/`ON` and `FALSE`/`OFF`.

The lexer knows which bare words are keywords and their token values; a string read refuses a
keyword, and a choice accepts every token of the chosen value, as MapServer does. Where
MapServer's own call is undefined (`getSymbol(8, …)` with seven choices for `SYMBOL TYPE`), the
ticket follows what it yields and says so.

## Acceptance criteria

- [ ] Given `CLASSITEM name` and `SYMBOL circle` unquoted, when each loads, then it is the
  load error MapServer's loader gives, and quoted they load as before.
- [ ] Given `LINEJOIN off`, `LINECAP polygon`, `ALIGN on`, when each loads, then it reads as
  MapServer reads it, a test per token value.
- [ ] Given the msautotest corpus, when G-1 runs, then every mapfile still loads.

## Read first

- `crates/mapfile/src/lexer.rs`, `crates/mapfile/src/translate/mod.rs` (`choice`, `string`); the
  convention on behaviour ported from MapServer in `docs/CONVENTIONS.md`.
- MapServer `rel-8-6-6`: `src/maplexer.l` (the keyword rules), `getSymbol`, `getString`, the
  enums of `src/mapserver.h`.

## Blocked by

- None (can start immediately)

## Mode

with the human: what a mapfile may say changes ([ADR-0007](../../adr/0007-harness.md)).
