# Attribute bindings `[name]`

**Where:** the style properties `COLOR`, `OUTLINECOLOR`, `SIZE`, `WIDTH`, `ANGLE` (and, with
later parameters, `OFFSET`, `OPACITY`, `OUTLINEWIDTH`, `SYMBOL`, `POLAROFFSET`). **IR:** the
property's value is `{"binding": "name"}`, one of three forms a style property takes beside a
plain value and an expression.

**Meaning.** The property varies by feature: it takes the feature's property `name`, read as
`bindStyle` in `src/maputil.c` reads an attribute.

- A number with C's `atof` (`bindDoubleAttribute`). An empty value leaves the property at its
  default, the value it has without the binding.
- A colour (`bindColorAttribute`): `#rrggbb`, `#rrggbbaa`, or three integers separated by single
  spaces, each read with `atoi`. MapServer first sets the colour to none, so a value that reads as
  none of these, an empty one included, leaves no colour.

A binding is not an expression: MapServer keeps the two apart (`bindings` and `exprBindings` of a
style), and a binding has the empty-value rule that an expression's `to_number` does not, which
reads an empty value as 0.
