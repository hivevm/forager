# `LABEL`, `TEXT`, and `LABELITEM`

**Where:** `LABEL` blocks and `TEXT` in `CLASS`, `LABELITEM` in `LAYER`. **IR:** the class's
`labels`.

**Text.** A label's text is its own `TEXT`, else the class's `TEXT`, else the layer's
`LABELITEM` as `to_string` of that property, as `msShapeGetLabelAnnotation` in `src/maputil.c`
picks it; the frontend resolves the choice at import. A `TEXT` is evaluated as
`msEvalTextExpression` does: a string has each `[name]` in it replaced by the property's value
(`concat` of the parts); an expression in parentheses is taken as text, a number printed with
`%g`. A `TEXT` the frontend cannot carry over is a warning and an empty text, which draws no
label.

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
`getInteger`, so the frontend truncates them. `TYPE` and `ANTIALIAS` are read and discarded, as
`loadLabel` discards them.

**Styles.** A label's `STYLE` blocks are [styles](style-symbol.md) drawn with the label; one
without a `GEOMTRANSFORM` draws at the label's point, `labelpoint`, the default `loadLabel` gives
it, and `labelpoly` draws the box around the text.

**`TRUE` and `ON`.** Wherever MapServer takes `ON` and `OFF` it takes `TRUE` and `FALSE` too, and
the other way round: its lexer gives `MS_TRUE` and `MS_ON` the value 1, `MS_FALSE` and `MS_OFF`
0 (`src/mapserver.h`), and its parser compares the values. The frontend takes them alike.
