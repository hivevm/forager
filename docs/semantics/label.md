# `LABEL`, `TEXT`, and `LABELITEM`

**Where:** `LABEL` blocks and `TEXT` in `CLASS`, `LABELITEM` in `LAYER`. **IR:** the class's
`labels`, each with its `text` and its `expression`.

**Text.** A label's text is its own `TEXT`, else the class's `TEXT`, else the layer's
`LABELITEM` as `to_string` of that property, as `msShapeGetLabelAnnotation` in `src/maputil.c`
picks it; the frontend resolves the choice at import. A label's own `TEXT ''` is its text, and
draws no label, without falling back. A `TEXT` is a string or an expression; `loadLabel` and
`loadClass` refuse a regular expression or a list, a class `TEXT` a later one replaces included.
A `TEXT` is evaluated as `msEvalTextExpression` does. A string has replaced by the property's value
each binding `msTokenizeExpression` finds when it reads the string with the expression lexer of
`src/maplexer.l`, every occurrence of it (`concat` of the parts): a `[name]`, or one quoted as
`'[name]'`, `"[name]"`, or `` `[name]` ``; `[shape]`, `[map_cellsize]`, and `[data_cellsize]` are
none. A quote that opens no such binding opens a string, which the same quote closes and in which
no binding is found; one that never closes holds the rest of the text, so `it's [name]` keeps
`[name]` as written. A carriage return, a newline, and a single `|` end the lexer's reading too.
An expression in parentheses is taken as text, a number printed with `%g`, a condition as `true`
or `false`, as `src/mapparser.y` gives a logical value where a string is asked for. A `TEXT` the
frontend cannot carry over, a time among them, is a warning and an empty text, which draws no
label.

**Condition.** A label's `EXPRESSION` decides whether it is drawn for a feature:
`msGetLabelStatus` in `src/maputil.c` evaluates it against the layer's `LABELITEM`, and a label
whose expression does not match is not drawn. The frontend reads it as a class's
[`EXPRESSION`](expression.md), with `LABELITEM` where a class's takes [`CLASSITEM`](classitem.md),
into the label's `expression`, one of the IR's conditions: one it cannot carry over is a warning
and a condition that never matches, and a reader of a newer minor keeps one it cannot read the
same way, since dropped it would draw the label for every feature.

**Parameters**, as `loadLabel` in `src/mapfile.c` reads them: `FONT`, a font of the
[font set](fontset.md) or an attribute, a comma-separated list naming fallbacks (`textlayout.c`
splits it); `SIZE`, a number above 0, an attribute, an expression, or a bitmap size `tiny`,
`small`, `medium`, `large`, `giant`, which MapServer stores as 5, 7, 10, 13, 16 (`MS_TINY` to
`MS_GIANT`); `MINSIZE`, `MAXSIZE`; `COLOR`, `OUTLINECOLOR`, `SHADOWCOLOR`, as
[colours](color.md); `OUTLINEWIDTH`; `SHADOWSIZE` and `OFFSET`, two numbers or attributes;
`POSITION`, one of `ul` … `lr` or `auto`, or an attribute; `ANGLE`, a number from −360 to 360,
an attribute, or `auto`, `auto2`, `follow`; `ALIGN`; `WRAP`, one character; `MAXLENGTH`,
`BUFFER`, `MINDISTANCE`, `REPEATDISTANCE`, `MINFEATURESIZE` (a number or `auto`),
`MAXOVERLAPANGLE` (0 to 360); `PARTIALS`; `FORCE` `on`, `off`, or `group`; `PRIORITY`, 1 to 10
(`MS_MAX_LABEL_PRIORITY`), an attribute, or an expression. MapServer reads the integer ones with
`getInteger`, which checks the range of the number as written and then casts it, and stores the
numbers of `OFFSET`, `SHADOWSIZE`, `PRIORITY`, and `MINFEATURESIZE` with a cast to `int`, so the
frontend truncates them all: `MINSIZE 0.5` is 0. `TYPE`, `truetype` or `bitmap`, and `ANTIALIAS`
are read and discarded, as `loadLabel` discards them.

**Repeated directives.** The last one holds, but `loadLabel` frees a property's binding only for
another binding, and `bindLabel` draws the binding over the value: `ANGLE [a] ANGLE 30` is bound
to `a`, and so are `ALIGN`, `COLOR`, `OUTLINECOLOR` (over `-1 -1 -1` too), `FONT`, `POSITION`,
and each number of `OFFSET` and `SHADOWSIZE`. `SIZE` frees its binding and its expression, so the
last `SIZE` holds; `PRIORITY` frees only its expression, so `PRIORITY [p] PRIORITY (e)` is the
expression, which `bindLabel` evaluates after the binding, and a later `PRIORITY 5` is bound to
`p` again. `ANGLE auto` sets the mode and leaves the angle.

**Styles.** A label's `STYLE` blocks are [styles](style-symbol.md) drawn with the label; one
without a `GEOMTRANSFORM` draws at the label's point, `labelpoint`, the default `loadLabel` gives
it, and `labelpoly` draws the box around the text.

**`TRUE` and `ON`.** Wherever MapServer takes `ON` and `OFF` it takes `TRUE` and `FALSE` too, and
the other way round: its lexer gives `MS_TRUE` and `MS_ON` the value 1, `MS_FALSE` and `MS_OFF`
0 (`src/mapserver.h`), and its parser compares the values. The frontend takes them alike.
