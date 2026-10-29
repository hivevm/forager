# `EXPRESSION`

**Where:** `CLASS`, `LABEL`. **IR:** the class's or the label's `expression`, a CQL2 tree
([ADR-0030](../adr/0030-cql2-expressions.md)).

**Meaning.** Which features the class draws. A class without one draws every feature that reaches
it; MapServer tries a layer's classes in order and draws a feature with the first that matches. A
label's decides whether the label is drawn for a feature, and its string, regular expression, or
list compares `LABELITEM` where the table below shows `CLASSITEM` ([label.md](label.md)).

## The four kinds

MapServer decides the kind by the token that follows the keyword (`loadExpression` in
`src/mapfile.c`) and evaluates it in `msEvalExpression` (`src/maputil.c`):

| Mapfile | IR | MapServer |
|---|---|---|
| `"road"` or `road` | `=` of `to_string(CLASSITEM)` and `"road"` | `strcmp` |
| `"road"i` | `=` of `casei` of both | `strcasecmp` |
| `/re/`, `/re/i` | `regexp_like(to_string(CLASSITEM), "re", "" or "i")` | `regcomp` with `REG_EXTENDED`, `REG_ICASE` for `i`, through PCRE2's POSIX interface, which ignores `REG_EXTENDED` and reads in byte mode |
| `{a,b,c}` | `in(to_string(CLASSITEM), ["a","b","c"])` | split at commas, `strcmp` each, no trimming |
| `( … )` | the translated expression | `yyparse` of `src/mapparser.y` |

The first three need a `CLASSITEM`; without one MapServer never matches, and the frontend warns and
stores `false`. An empty one, `""`, `{}`, or `()`, matches every feature, item or not, since
`msEvalExpression` takes an empty expression as true; the frontend stores `true`. Any other token
after `EXPRESSION` — a number, a bare binding — is refused by MapServer's loader and is a load
error.

## The expression language

Shared by `EXPRESSION`, `FILTER`, and, in later parameters, `TEXT` and attribute bindings. The
tokens are those of `src/maplexer.l` in its expression state, which MapServer builds with `flex -i`,
so keywords match in any case. That state has a rule for each keyword and none for a word, so
`andnot` is `and` and `not`. A newline or a single `|` ends the expression: what follows is unread.
The outer parentheses are the mapfile token's, which runs to the last `)` on its line and of which
`loadExpression` keeps the inside: `([a] = 1) OR ([b] = 2)` parses as `[a] = 1) OR ([b] = 2`, which
fails. The grammar and its precedence are those of `src/mapparser.y`, loosest first: `OR` (`or`,
`||`); `AND` (`and`, `&&`); `NOT` (`not`, `!`); the comparisons `=` `==` `eq`, `!=` `<>` `ne`, `<`
`lt`, `>` `gt`, `<=` `le`, `>=` `ge`, `=*`, `~`, `~*`, `in`; `+` `-`; `*` `/` `%`; unary `-`; `^`,
the only right-associative one.

**Types come from the syntax.** `[a]` is the attribute as a number, read with C's `atof`: its
longest numeric prefix, or 0, so `"12abc"` is 12 and `"abc"` is 0. `"[a]"` and `'[a]'` are it as
a string, `` `[a]` `` as a time. The frontend writes these as `to_number`, `to_string`, and
`to_timestamp` around the property, so the IR means the same whatever type a source delivers. A
comparison takes two operands of one type; one of a number and a string is no production of the
grammar, so MapServer never matches it, and neither does the IR.

**Numbers in a condition.** `AND`, `OR`, and `NOT` take numbers as well, true when not 0: the IR
writes `x <> 0`. A whole expression that is a number is true when not 0; one that is a string is
always true, as MapServer tests only that the string exists.

| Mapfile | IR |
|---|---|
| `=`, `!=`, `<`, `>`, `<=`, `>=` on numbers or strings | `=`, `<>`, `<`, `>`, `<=`, `>=` |
| the same on times | `t_equals`, `not t_equals`, `t_before`, `t_after`, `not t_after`, `not t_before` |
| `a =* b` on strings | `=` of `casei(a)` and `casei(b)`; on numbers `=`; on times `t_equals` |
| `a ~ re`, `a ~* re` | `regexp_like(a, re, "")`, `regexp_like(a, re, "i")` |
| `a in 'x,y'` | `in(a, ["x","y"])`, split at load; for a number `a`, each part read with `atof` |
| `AND`, `OR` chains | one `and` or `or` with every operand |
| `"a" + "b"` | `concat("a", "b")` |
| `-x` | `x`, as the grammar's rule `$$ = $2` does not negate |
| `length`, `round`, `tostring`, `commify`, `upper`, `lower`, `initcap`, `firstcap` | the function of the same name |

A number token may start with `-`: `[a] -1` is two operands with no operator, and MapServer, like
the frontend, does not parse it. Strings take `\'`, `\"`, and `\\` as escapes. A binding runs to
the last `]` before its closing character that flex's longest match reaches, that character
doubled inside it standing for one: `"[a]b]"` and `[a]]b]` both name `a]b`.

**Times.** A time literal `` `2010-12-01` `` becomes `{"date": …}`, `` `2010-12-01T10:00:00` ``
with an optional `Z` a `{"timestamp": …}`; MapServer's other time formats are not yet carried
over.

**Not carried over.** `[shape]`, `[map_cellsize]`, `[data_cellsize]`, the geometry functions and
spatial predicates, `IN` against a computed string, a number past the range of a double, which JSON
cannot hold, and `%name%` runtime substitutions. Each is a warning, and the class keeps `false`,
never matching — what MapServer does with an expression it cannot parse, since it parses one only
when it evaluates it.

**Limit.** An expression nests at most 24 deep (Q-9); a deeper one, or parentheses nested past
four times that, fail the load. A chain of `AND` or `OR` counts once.

## Evaluation

The core crate `expr` evaluates the tree as `msEvalExpression` (`src/maputil.c`) and the grammar of
`src/mapparser.y` evaluate MapServer's ([ADR-0031](../adr/0031-expression-evaluator.md)). The engine
tries a layer's `FILTER` first; a feature that fails it is not drawn. It then draws the feature
with the first class whose expression matches, a class without one matching every feature; a
feature no class matches is not drawn (`msShapeGetClass`).

**Attributes.** A name is found without regard to ASCII case, as MapServer finds an item in a
shapefile's `.dbf` (`msDBFGetItemIndex`, `strcasecmp`) and in an OGR source
(`OGR_FD_GetFieldIndex`): by the source's own lookup, then the first property whose name equals it
in ASCII case. A feature that has no value for it, or a null one, reads as the empty text, as a
shape whose source has the field but no value for it gives `""`. A boolean reads as `1` or `0`, as
OGR gives one; a number as the text its source gives (ADR-0026). A value ends at its first NUL
byte, as a C string does.

**A missing attribute.** MapServer asks the source for every attribute a layer's classes, styles,
labels, and filter read before it draws (`msLayerWhichItems`, which collects them with
`string2list`, again without regard to case), and a source without one fails the layer with an
error, and with it the map. Forager has no field list of a source, and skips the layer with the
warning `property-missing` when none of the features it reads holds a property its filter, its
class expressions, or its styles' bound and computed properties read; the other layers draw (G-8).
Labels' properties are checked when labels are drawn.

**Values.** A number is a C `double`; `to_number` is `atof` of the text — `strtod` in the C
locale, so `0x10` is 16 and `inf` and `nan` read as such. Strings compare as `strcmp`, by unsigned
bytes; `casei` lowers ASCII letters only, so `=` over `casei` is `strcasecmp`. `to_string` gives a
condition as `true` or `false` and a number as `%g`. A whole expression that is a number matches
when it is not 0, one that is a string always.

**Operators.** `in` compares the value to each item as it stands, untrimmed, by `strcmp` or as
doubles. `%` is `(int)a % (int)b`; `/` and `%` by zero, a `%` operand past an `int`, and `round`
past a `long` are undefined in C, and fail the expression. `^` is `pow`. `AND`, `OR`, and `NOT`
take conditions; the grammar evaluates every operand before an operator, so a failure anywhere in
the tree, `false AND 1/0 <> 0` included, fails the whole expression, which then does not match.

**Functions.** `length` counts bytes. `round(x)` is `MS_NINT`, `lrint` at the pin, half to even;
`round(x, n)` is `MS_NINT(x / n) * n`. `upper`, `lower`, `initcap`, and `firstcap` change ASCII
letters byte by byte, `initcap` starting a word after each space. `commify` places a comma before
every third character left of the decimal point as `msCommifyString` does, a sign counted as a
digit (`-123456` is `-,123,456`), and leaves a text with two points, or none before its point, as
it is. `tostring(x, format)` is `msToString`: one conversion `%e`, `%E`, `%f`, `%F`, `%g`, or
`%G` with the flags `+ - space ' 0`, a width up to 1024, and a precision, and `%%`; any other
format fails the expression, and the result is cut to the buffer `msToString` sizes.

**Times.** `to_timestamp`, a `date`, and a `timestamp` read as `msParseTime`: the first of its
formats whose pattern matches the start of the text, read by glibc's `strptime`, which stops at
the first field it cannot read and keeps the fields before it (`2010-13-01` is the year 2010
alone). Text no format matches, an empty one included, fails the expression. Times compare as
`msTimeCompare`, field by field from the year to the second, without time zones.

**Patterns.** `regexp_like` reads its pattern in PCRE2's syntax in byte mode
([ADR-0030](../adr/0030-cql2-expressions.md)) and matches it with the `regex` crate's byte matcher,
its Unicode features off: `.` is one byte, `\d \w \s \b` and the flag `i` are ASCII. An empty value
never matches, before the pattern is read. A pattern PCRE2 refuses makes the term false where it
stands. A brace is a quantifier only as `{n}`, `{n,}`, or `{n,m}`, up to 65535, as PCRE2 10.42
reads it; any other brace is a literal one (`a{,3}`). A pattern with a construct PCRE2 reads and
the crate cannot match as PCRE2 does — a back-reference, a look-around, an atomic group,
recursion or a subroutine call, `\Q…\E`, `\K`, a possessive quantifier, a branch reset, a
comment group, the option `x`, `\h`, `\v`, `\R`, `\e`, `\cX`, `\N`, `\p`, `\Z`, `\G`, an
octal escape, a verb `(*…)`, a range with a class as an end — and a pattern compiled past 1 MiB
(Q-9) make the whole expression not match, and the engine reports each once per render with the
class or filter that holds it (`pattern-unsupported`). `$` matches at the very end only, where
PCRE2's matches before a final newline too. Each pattern is compiled once per render.

**Bound and computed style properties.** For each feature the engine binds `COLOR`,
`OUTLINECOLOR`, `SIZE`, `WIDTH`, `OUTLINEWIDTH`, `ANGLE`, `OFFSET`, and a `SYMBOL` bound by name as
`bindStyle` does: a binding as [binding.md](binding.md) states; an expression's number as
`msEvalDoubleExpression` reads it, 0 where it fails; an expression's colour from its text as
`bindColorAttribute` reads it, none where it fails or gives no colour. Two differences: MapServer
binds into the style itself, so where a colour expression gives none it keeps the previous
feature's colour, and the cached second pass of a line layer draws every feature with the values
bound for the last; Forager binds each feature's own values each time.
