# Attribute bindings `[name]`

**Where:** in `STYLE`, `COLOR`, `OUTLINECOLOR`, `SIZE`, `WIDTH`, `ANGLE`, `SYMBOL`, `OFFSET`,
`POLAROFFSET`, `OPACITY`, and `OUTLINEWIDTH`; in `LABEL`, `FONT`, `SIZE`, `COLOR`,
`OUTLINECOLOR`, `SHADOWCOLOR`, `SHADOWSIZE`, `POSITION`, `OFFSET`, `ANGLE`, `ALIGN`, and
`PRIORITY`. **IR:** the property's value is `{"binding": "name"}`, one of three forms a style or
label property takes beside a plain value and an expression.

**Meaning.** The property varies by feature: it takes the feature's property `name`, read as
`bindStyle` in `src/maputil.c` reads an attribute.

- A number with C's `atof`, `OPACITY` rounded to an integer (`bindDoubleAttribute`,
  `bindIntegerAttribute`). Before it reads the value, `bindStyle` sets the property to a value of
  its own, which an empty value leaves: 360 for `ANGLE`, 1 for `SIZE`, `WIDTH`, and
  `OUTLINEWIDTH`, 100 for `OPACITY`, 0 for `OFFSET` and `POLAROFFSET`. For `SIZE`,
  `OUTLINEWIDTH`, and `ANGLE` that is not the value the style has without the binding, −1 (the
  symbol's own size), 0, and 0, though an angle of 360 draws as one of 0. A value that is not
  empty but no number reads as 0.
- A symbol (`SYMBOL`) by its name, without regard to case, with `msGetSymbolIndex`, which reads
  a name the symbol set does not hold as an image file; where that fails too, the symbol is 0.
- A colour (`bindColorAttribute`): `#` and six or eight characters, each pair read with
  `msHexToInt`, the alpha 255 where it is not given; or three integers separated by spaces, each
  read with `atoi` and taken without a range check. `msStringSplit` runs several spaces together
  into one, but a leading or a trailing space adds an empty token, which `atoi` reads as 0: `" 1 2"`
  is the colour 0, 1, 2, and `" 1 2 3"` is four tokens and no colour. A tab is no separator.
  MapServer first sets the colour to none, so a value that reads as none of these, an empty one
  included, leaves no colour, and so does one with a channel of −1 (`MS_VALID_COLOR`).

A label's properties are read by `bindLabel` in the same file: `ANGLE` as a number, `SIZE`,
`PRIORITY`, `SHADOWSIZE`, and `OFFSET` as a number rounded to an integer (`bindIntegerAttribute`,
`MS_NINT(atof(…))`), and `FONT` as the value as written. `ALIGN` and `POSITION` take a nonzero
integer as it is, without a range check; where the value reads as 0, `ALIGN` takes a value of at
least four characters that begins with `left`, `cente`, or `right`, and `POSITION` one of exactly
two characters among `ul` … `lr` and `cc`, each compared without case. MapServer binds no
`SHADOWCOLOR`: `loadLabel` reads it with `loadColor` and no binding. The frontend carries a
`SHADOWCOLOR` binding all the same, and the style IR does not state the rounding.

A binding is not an expression: MapServer keeps the two apart (`bindings` and `exprBindings` of a
style), and a binding has the empty-value rule that an expression's `to_number` does not, which
reads an empty value as 0.
