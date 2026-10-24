# Attribute bindings `[name]`

**Where:** in `STYLE`, `COLOR`, `OUTLINECOLOR`, `SIZE`, `WIDTH`, `ANGLE`, `SYMBOL`, `OFFSET`,
`POLAROFFSET`, `OPACITY`, and `OUTLINEWIDTH`; in `LABEL`, `FONT`, `SIZE`, `COLOR`,
`OUTLINECOLOR`, `SHADOWCOLOR`, `SHADOWSIZE`, `POSITION`, `OFFSET`, `ANGLE`, `ALIGN`, and
`PRIORITY`. **IR:** the property's value is `{"binding": "name"}`, one of three forms a style or
label property takes beside a plain value and an expression.

**Meaning.** The property varies by feature: it takes the feature's property `name`, read as
`bindStyle` in `src/maputil.c` reads an attribute.

- A number with C's `atof` (`bindDoubleAttribute`). An empty value leaves the property at its
  default, the value it has without the binding.
- A colour (`bindColorAttribute`): `#rrggbb`, `#rrggbbaa`, or three integers separated by single
  spaces, each read with `atoi`. MapServer first sets the colour to none, so a value that reads as
  none of these, an empty one included, leaves no colour.

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
