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

**Open for the evaluator.** MapServer finds an attribute by name without regard to case, and an
attribute the layer does not have is an error when it is drawn; the IR keeps the name as written.
`src/mapparser.y` also decides what the operators mean when evaluated: `%` truncates both
operands to integers; `/` by 0 fails the expression, which then never matches; `~`, `~*`, and a
`/re/` expression never match an empty value, nor with a pattern `regcomp` refuses; `round`
rounds half away from zero (`MS_NINT`), with two arguments to a multiple of the second.
