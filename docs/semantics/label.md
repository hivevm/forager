# `LABEL`, `TEXT`, and `LABELITEM`

**Where:** `LABEL` blocks and `TEXT` in `CLASS`, `LABELITEM` in `LAYER`. **IR:** the class's
`labels`, each with its `text` and its `expression`.

**Text.** A label's text is its own `TEXT`, else the class's `TEXT`, else the layer's
`LABELITEM` as `to_string` of that property, as `msShapeGetLabelAnnotation` in `src/maputil.c`
picks it; the frontend resolves the choice at import. A label without any of the three takes
the `TEXT` of an inline feature, which the engine reads for each feature
([feature.md](feature.md)). A label's own `TEXT ''` is its text, and
draws no label, without falling back. A `TEXT` is a string or an expression; `loadLabel` and
`loadClass` refuse a regular expression or a list, a class `TEXT` a later one replaces included.
A `TEXT` is evaluated as `msEvalTextExpression` does. A string has replaced by the property's value
each binding `msTokenizeExpression` finds when it reads the string with the expression lexer of
`src/maplexer.l`, every occurrence of it (`concat` of the parts): a `[name]`, or one quoted as
`'[name]'`, `"[name]"`, or `` `[name]` ``; `[shape]`, `[map_cellsize]`, and `[data_cellsize]` are
none. Each binding replaces every occurrence of itself in what the earlier ones left, in the
lexer's order; a value holding a later binding's `[name]` is not searched again, as it is in
MapServer. A `'` or `"` that opens no such binding opens a string, which the same quote closes and
in which no binding is found; one that never closes holds the rest of the text, so `it's [name]`
keeps `[name]` as written. A pair of backticks is a time literal, a lone backtick one character. A
newline and a single `|` end the lexer's reading; a carriage return is white space to it. It stops
too at a token it cannot read: an empty time literal, which the frontend follows, and one
`msParseTime` rejects, or `fromtext` without a WKT string, which the frontend cannot judge, so a
text with one before a `[` is a warning and not carried over.
An expression in parentheses is taken as text, a number printed with `%g`, a condition as `true`
or `false`, as `src/mapparser.y` gives a logical value where a string is asked for. A `TEXT` the
frontend cannot carry over, a time among them, is a warning and an empty text, which draws no
label but its labelpnt styles (see `LABELCACHE` below).
The engine evaluates a string, a property as text, and their concatenation, reading a feature's
properties as a bound property reads them ([expression.md](expression.md), *Attributes*): a name
matches whatever its case, a boolean prints as `1` or `0`, and a value ends at its first NUL. A
text that would pass 16 MiB leaves the label out, with the diagnostic of that bound
(`text-too-long`, Q-9). Any other expression leaves the label out with `label-text-unsupported`.

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
[colours](color.md), all but `SHADOWCOLOR` also an attribute; `OUTLINEWIDTH`; `SHADOWSIZE` and
`OFFSET`, two numbers or attributes;
`POSITION`, one of `ul` … `lr` or `auto`, or an attribute; `ANGLE`, a number from −360 to 360,
an attribute, or `auto`, `auto2`, `follow`; `ALIGN`; `WRAP`, one character; `MAXLENGTH`,
`BUFFER`, `MINDISTANCE`, `REPEATDISTANCE`, `MINFEATURESIZE` (a number or `auto`),
`MAXOVERLAPANGLE` (0 to 360); `PARTIALS`; `FORCE` `on`, `off`, or `group`; `PRIORITY`, 1 to 10
(`MS_MAX_LABEL_PRIORITY`), an attribute, or an expression. MapServer reads the integer ones with
`getInteger`, which checks the range of the number as written and then casts it, and stores the
numbers of `OFFSET`, `SHADOWSIZE`, `PRIORITY`, and `MINFEATURESIZE` with a cast to `int`, so the
frontend truncates them all: `MINSIZE 0.5` is 0. `TYPE`, `truetype` or `bitmap`, and `ANTIALIAS`
are read and discarded, as `loadLabel` discards them.

**Lines and priority.** A label's text is broken into lines as `msLayoutTextSymbol` in
`src/textlayout.c` breaks it: with a `WRAP` and no `MAXLENGTH`, at every `WRAP` character; with a
positive `MAXLENGTH`, a text of more characters than it is broken at a `WRAP` character or U+200B
met once the line holds `MAXLENGTH` characters, and a text no longer than `MAXLENGTH` keeps its
`WRAP` characters.
`MAXLENGTH` is held as MapServer's `int`. A `PRIORITY` expression is evaluated for each feature
as `msEvalDoubleExpression` evaluates it, its value printed with `%g` and read back with `atof`;
that value, a literal, and a bound value alike are converted to an `int` and held at 1 to 10, as
`msAddLabelGroup` holds them, so a condition, which the frontend gives as 0, draws at 1.

**Repeated directives.** The last one holds, but `loadLabel` frees a property's binding only for
another binding, and `bindLabel` draws the binding over the value: `ANGLE [a] ANGLE 30` is bound
to `a`, and so are `ALIGN`, `COLOR`, `OUTLINECOLOR` (over `-1 -1 -1` too), `FONT`, `POSITION`,
and each number of `OFFSET` and `SHADOWSIZE`. `SIZE` frees its binding and its expression, so the
last `SIZE` holds; `PRIORITY` frees only its expression, so `PRIORITY [p] PRIORITY (e)` is the
expression, which `bindLabel` evaluates after the binding, and a later `PRIORITY 5` is bound to
`p` again. `ANGLE auto` sets the mode and leaves the angle.

**Styles.** A label's `STYLE` blocks are [styles](style-symbol.md) drawn with the label; one
without a `GEOMTRANSFORM` draws at the label's point, `labelpoint`, the default `loadLabel` gives
it, `labelpoly` draws the box around the text, and `labelcenter` draws its marker at the centre
of that box, the box of the turned polygon where the label is turned. They are drawn in their
order, before the text; the marker of the first `labelpoint` style takes room from other labels,
a `labelpoly` and a `labelcenter` marker take none. A `labelpoly` is drawn by `msDrawShadeSymbol`
in `src/maprendering.c`, as a polygon's style is: filled without a symbol, and hatched with a hatch
symbol, its lines counted from the map's origin, or from the corner of a layer's own image for
the labels of a layer of `LABELCACHE OFF` drawn there ([symbol.md](symbol.md)); a hatch past its
bounds is not drawn, and the label's layer warns with `hatch-not-drawn`, as for a polygon's (Q-9).
A `labelpoly` of any other symbol is not drawn yet, so its label is left out with
`label-not-drawn`, as is a label with a style that reaches the IR without a transform (the
diagnostic says so). `ANGLE AUTO` on a label's style changes nothing: `loadStyle` sets only the
style's `autoangle`, which none of the functions drawing a label's styles reads, so the style turns
by its `ANGLE` alone.

**The marker under a label.** The label cache holds beneath each label of a point layer the box
of the marker its class's first style draws at the point, as `msAddLabelGroup` and `msAddLabel`
in `src/maplabel.c` hold the box of `msGetMarkerSize` in `src/mapsymbol.c`: `w` by `h` about the
point, its far corner `w - 1` and `h - 1` from its near one. A label at any position but `CC` moves
off it by half that span, cut to whole pixels, and other labels are tested against it. A vector or
ellipse symbol is sized by its extent, MapServer's default symbol at 1 by 1, and a pixmap by its
image, loaded before the layer's labels are collected, at any scale, as `msGetMarkerSize` loads
it: a 32 × 32 image is 32 × 32 without a `SIZE` and 55 × 55 at `SIZE 55`, whatever its `ANGLE`
([symbol.md](symbol.md)). An SVG or a TrueType symbol has no size yet, so its label is left out
with `label-not-drawn`. The label of a pixmap that does not load is left out too, with
`label-not-drawn` naming the symbol and its image, and the rest of the map drawn; in MapServer
`msGetMarkerSize` fails, and with it `msAddLabelGroup`, the layer, and the map. A class whose first
style varies by no feature sizes every label's marker alike, held once for the label, not for
each feature's (Q-9). A layer of `LABELCACHE OFF` holds no marker.

**`LABELCACHE`.** A layer's `LABELCACHE`, `ON` or `OFF` as `loadLayer` in `src/mapfile.c` reads
it and `ON` without one (`initLayer`), is the IR's `label_cache`, true where it is absent. A point
layer of `LABELCACHE OFF` draws its labels at once, as `pointLayerDrawShape` in `src/mapdraw.c`
does, into the layer's image: each label `msGetLabelStatus` in `src/maputil.c` passes is drawn by
`msDrawLabel` with its styles before its text, by its position and its `OFFSET` alone, moved off
no marker, with no test against other labels, markers, or the image's edge. They never enter the
label cache, so the cached labels are not tested against them, and `Rendered.labels` does not list
them. A text that is empty or of line breaks only draws the label's labelpnt styles alone; with a
labelpoly or labelcenter style, on which `msDrawLabel` fails the layer's draw or draws on a box of
no size, the label is left out with a warning. Through the label cache, a label whose text is of
line breaks only and that has styles is left out with `label-not-drawn`: `msDrawLabelCache` tests
and draws its labelpnt marker whatever the text (`src/mapdraw.c:3607-3646, 4056-4065`), but only
once the text's box, which `msLayoutTextSymbol` leaves at `FLT_MAX` and `-FLT_MAX`, has passed its
tests (3789, 3898), and that box is not ported. A label without text, its text empty, which
`msShapeGetLabelAnnotation` in `src/maputil.c` reads as none, or not set at all, is kept by the
label cache for a labelpnt style alone, as `msAddLabelGroup` keeps it (`src/maplabel.c:267-277`):
its marker is tested unless the label is forced, takes room from the labels after it, and is its
box in `Rendered.labels`; its labelpnt styles are drawn, and it is measured from by no
`MINDISTANCE` (`src/mapdraw.c:3384-3385, 3607-3650, 3994-4069`). A labelpoly or labelcenter style
beside them fails `msDrawLabelCache` and the map, as it is drawn only with a text
(`src/mapdraw.c:4056-4133, 540, 593`): such a label is left out with `label-not-drawn`. Without a
labelpnt style it draws nothing and nothing is reported. MapServer needs no `FONT` for a label
without text, as it resolves one only to lay out a text (`msComputeTextPath`, which
`msDrawLabelCache` and `msDrawLabel` call only for a text, `src/mapdraw.c:3607-3650, 2710-2727`):
such a label without a `FONT` is drawn as one with a `FONT`, through the label cache and under
`LABELCACHE OFF`. A font is settled feature by feature: of a label without a `FONT` whose text is
empty only for some features, an empty `LABELITEM` value, those features draw their labelpnt
markers, and the others, whose text MapServer lays out in its built-in font, which is not bundled,
are left out with `label-not-drawn` ("it names no FONT"). A text of line breaks only lays out no
glyph and so needs no font either.

The engine draws a feature point by point, as `pointLayerDrawShape` does
(`src/mapdraw.c:2062-2105`): at each point inside the map's extent, every class style's marker,
whatever its `GEOMTRANSFORM`, then the labels there, then the markers at its next point; so a
marker at a later point of a multipoint covers a label at an earlier one. A point outside the
extent draws neither. MapServer's line
and polygon layers draw their labels at once too, which the engine does not draw yet: it leaves
them out with `label-not-drawn`. MapServer labels no such feature that its simplification to
pixels ([line.md](line.md), **Vertices**) leaves without a part: `msDrawShape` in `src/mapdraw.c`
returns before `lineLayerDrawShape` or `polygonLayerDrawShape`, which draw the labels, once
`msTransformShape` has left the shape no line (`src/mapdraw.c:2505-2507, 2577-2580`). The engine,
which draws none of those labels, does not tell such a feature from the others yet.

**`TRUE` and `ON`.** Wherever MapServer takes `ON` and `OFF` it takes `TRUE` and `FALSE` too, and
the other way round: its lexer gives `MS_TRUE` and `MS_ON` the value 1, `MS_FALSE` and `MS_OFF`
0 (`src/mapserver.h`), and its parser compares the values. The frontend takes them alike.
