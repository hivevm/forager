# `SIZE` (style)

**Where:** `STYLE`. **IR:** the style's `size`, a number in pixels, a [binding](binding.md), or an
expression.

**Meaning.** The size of the style's symbol. `loadStyle` in `src/mapfile.c` takes a number
greater than 0, a binding, or an expression in parentheses; 0 or less is a load error.

**Expression.** MapServer evaluates `SIZE ( … )` with `msEvalDoubleExpression` in
`src/maputil.c`: as text, a number printed with `%g` — six significant digits — and that text
read back with `atof`. The IR states this: `to_number(tostring(e, "%g"))` for a numeric `e`,
`to_number(e)` for a string. An expression MapServer cannot parse yields 0; the frontend warns
and stores 0. A condition or a time is not a number, and is the same warning.

How a size is scaled and bounded is [`scale.md`](scale.md); that a vector symbol's size is its
height comes with the symbols.
