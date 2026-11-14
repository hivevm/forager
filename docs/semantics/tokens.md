# Tokens: bare words, keywords, and numbers

How the mapfile frontend reads a value's token, as MapServer's lexer (`src/maplexer.l`) and its
readers `getString`, `getSymbol`, `getToken`, and `getDouble` (`src/mapfile.c`) read it at the
pin.

**A bare word that is a keyword is no string.** Every word one of the lexer's rules names, such
as `circle`, `name`, `data`, or `on`, is a token of its own and never a string, without regard to
case. Where MapServer expects a string, `getString` refuses it and the load fails, so
`CLASSITEM name`, `SYMBOL circle`, and `SHAPEPATH data` are load errors (`invalid-value`); quoted,
`'name'` is a string. A bare word that no rule names, such as `roads` or `/data/x.shp`, is a
string. The table of the keywords and their token values is `crates/mapfile/src/keywords.rs`.
Where MapServer reads any token's text with `getToken`, an output format's `NAME` and `IMAGEMODE`,
a keyword is read as its text; a `PROJECTION` takes the keyword `auto` besides strings, as
`loadProjection` does.

**A keyword choice is compared by its token value.** `getSymbol` compares the lexer's numeric
token with the values of its choices, so every word of the same value stands for a choice:
`OFF`, `FALSE`, `INCHES`, and `POINT` are 0, which is `LINEJOIN none`, `TYPE point` of a layer,
and `TYPE truetype` of a symbol; `ON`, `TRUE`, `FEET`, and `LINE` are 1, `LINEJOIN bevel` and a
label's `ALIGN left`; `POLYGON`, `DEFAULT`, `MILES`, and `CENTER` are 2, `LINECAP butt`; `dd` is 5,
a label's `SIZE tiny`. This holds for every choice the frontend reads: a layer's `TYPE` and
`STATUS`, `UNITS` and `SIZEUNITS`, a style's `LINECAP` and `LINEJOIN`, a label's `ALIGN`, `SIZE`,
`ANGLE`, `FORCE`, `PARTIALS`, and `TYPE`, a symbol's `TYPE` and `FILLED`, and an output format's
`TRANSPARENT`. `loadSymbol` hands `getSymbol` a count of 8 for its seven types; the eighth value
it compares is undefined in C, and the frontend takes the seven alone. A layer's `CONNECTIONTYPE`
is read so too, `getSymbol(14, MS_OGR, …)` of `loadLayer`, and its source takes the name of the
choice in lower case as its kind: `small`, 7, is `wms`, and `postgresql`, 4, is `ogr`. At the pin
`graticule` lexes as the keyword `GRATICULE`, 1140, which is not `MS_GRATICULE`, 10, so MapServer
itself refuses `CONNECTIONTYPE graticule` (`invalid-value`); the word of 10 is `medium`.

**A statement keyword is matched by its token value.** Each loader switches on the lexer's token,
so a word stands for the statement or block of its block that has its value: `SIMPLE`
(`MS_SYMBOL_SIMPLE`, 1000) inside a `STYLE` or a `LABEL` is `ANGLE`, `POSTLABELCACHE` inside a
`LABEL` is `FORCE` (1098), `SVG` (1006) is `COLOR`, and inside a `LAYER` `large` (13) is
`PLUGIN`, whose case is `MS_PLUGIN`. A word of no statement of its block stays the warning
`unknown-keyword`.

**A `VALIDATION` or `METADATA` pair is two strings.** `loadHashTable` takes a key only as a string
token and reads its value with `getString`, failing the load on a bare keyword in either place;
the frontend leaves such a pair out with the warning `unsupported-value`, the block's other pairs
still holding.

**A number token is read with `atof`.** The lexer's rule for a number,
`-?[0-9]+|-?[0-9]+\.[0-9]*|-?\.[0-9]*` with an optional exponent, is the same in a mapfile and in
an expression, and one function measures it for both (`number_len` in
`crates/mapfile/src/lexer.rs`). `.` and `-.` match it and read as 0, so `EXTENT . 0 1 1` loads as
`([a] = .)` does. A number `atof` reads as infinite, such as `1e999`, is refused
(`invalid-number`), since the style document's JSON cannot carry it.
