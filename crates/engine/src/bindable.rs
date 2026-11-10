//! The properties of a style a feature may bind, listed once: how each is read from a style as
//! written, and set on one. Whether a style varies by feature, its binding to a feature
//! (`classes.rs`), and the values a cached label holds of its bound styles (`label_styles.rs`) all
//! walk this list.

use forager_style_ir::{Bound, Rgba, Style, SymbolRef};

/// A property of a style a feature may bind: one whose written value may be a binding or an
/// expression, which `bindStyle` binds to each feature.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Field {
    Color,
    OutlineColor,
    Opacity,
    Angle,
    Size,
    Width,
    OutlineWidth,
    /// One coordinate of `OFFSET`.
    Offset(usize),
    /// The distance (0) or the angle (1) of `POLAROFFSET`.
    PolarOffset(usize),
    LineOffset,
    Symbol,
}

/// A property of a style as written, with how `bindStyle` binds it.
pub(crate) enum Written<'a> {
    /// A number, `unset` where a binding's value is empty, as bindDoubleAttribute reads it;
    /// computed by an expression where `computes`, else 0.
    Number {
        property: Option<&'a Bound<f64>>,
        unset: f64,
        computes: bool,
    },
    /// A colour, as bindColorAttribute reads it.
    Color(Option<&'a Bound<Rgba>>),
    /// A symbol, of which a binding names one; an expression is not bound.
    Symbol(Option<&'a Bound<SymbolRef>>),
}

/// A value of a style's property bound to a feature: a colour, `None` where it does not read, a
/// number, or a symbol's index.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Value {
    Color(Option<Rgba>),
    Number(f64),
    Symbol(u32),
}

impl Field {
    /// Every property a feature may bind, in the order `bindStyle` binds them.
    pub(crate) const ALL: [Field; 13] = [
        Field::Color,
        Field::OutlineColor,
        Field::Opacity,
        Field::Angle,
        Field::Size,
        Field::Width,
        Field::OutlineWidth,
        Field::Offset(0),
        Field::Offset(1),
        Field::PolarOffset(0),
        Field::PolarOffset(1),
        Field::LineOffset,
        Field::Symbol,
    ];

    /// The property of `style` as written.
    pub(crate) fn written<T>(self, style: &Style<T>) -> Written<'_> {
        fn number(property: Option<&Bound<f64>>, unset: f64) -> Written<'_> {
            Written::Number {
                property,
                unset,
                computes: true,
            }
        }
        fn pair(p: &Option<[Bound<f64>; 2]>, i: usize) -> Option<&Bound<f64>> {
            p.as_ref().map(|p| &p[i])
        }
        match self {
            Field::Color => Written::Color(style.color.as_ref()),
            Field::OutlineColor => Written::Color(style.outline_color.as_ref()),
            // bindStyle sets an OPACITY bound to an empty value to 100.
            Field::Opacity => number(style.opacity.as_ref(), 100.0),
            Field::Angle => number(style.angle.as_ref(), 360.0),
            Field::Size => number(style.size.as_ref(), 1.0),
            Field::Width => number(style.width.as_ref(), 1.0),
            // bindStyle computes no OUTLINEWIDTH from an expression: the style keeps its 0.
            Field::OutlineWidth => Written::Number {
                property: style.outline_width.as_ref(),
                unset: 1.0,
                computes: false,
            },
            Field::Offset(i) => number(pair(&style.offset, i), 0.0),
            Field::PolarOffset(i) => number(pair(&style.polar_offset, i), 0.0),
            Field::LineOffset => number(style.line_offset.as_ref().map(|l| &l.distance), 0.0),
            Field::Symbol => Written::Symbol(style.symbol.as_ref()),
        }
    }

    /// Sets the property of `style` to `value`, a value of its kind; a coordinate of a pair or a
    /// line offset the style does not have stays unset.
    pub(crate) fn set<T>(self, value: Value, style: &mut Style<T>) {
        let number = || match value {
            Value::Number(n) => Bound::Value(n),
            _ => unreachable!("a number field holds a number"),
        };
        let color = || match value {
            Value::Color(c) => c.map(Bound::Value),
            _ => unreachable!("a colour field holds a colour"),
        };
        match self {
            Field::Color => style.color = color(),
            Field::OutlineColor => style.outline_color = color(),
            Field::Opacity => style.opacity = Some(number()),
            Field::Angle => style.angle = Some(number()),
            Field::Size => style.size = Some(number()),
            Field::Width => style.width = Some(number()),
            Field::OutlineWidth => style.outline_width = Some(number()),
            Field::Offset(i) => {
                if let Some(pair) = &mut style.offset {
                    pair[i] = number();
                }
            }
            Field::PolarOffset(i) => {
                if let Some(pair) = &mut style.polar_offset {
                    pair[i] = number();
                }
            }
            Field::LineOffset => {
                if let Some(line) = &mut style.line_offset {
                    line.distance = number();
                }
            }
            Field::Symbol => {
                let Value::Symbol(index) = value else {
                    unreachable!("a symbol field holds a symbol")
                };
                style.symbol = Some(Bound::Value(SymbolRef::Index(index)));
            }
        }
    }

    /// Whether the property of `style` varies by feature: a binding or an expression, of a
    /// symbol a binding alone.
    pub(crate) fn varies<T>(self, style: &Style<T>) -> bool {
        fn bound<T>(property: Option<&Bound<T>>) -> bool {
            property.is_some_and(|p| !matches!(p, Bound::Value(_)))
        }
        match self.written(style) {
            Written::Number { property, .. } => bound(property),
            Written::Color(property) => bound(property),
            Written::Symbol(property) => matches!(property, Some(Bound::Binding { .. })),
        }
    }

    /// The properties of `style` that vary by feature, in the order of [`Field::ALL`].
    pub(crate) fn of<T>(style: &Style<T>) -> impl Iterator<Item = Field> + '_ {
        Field::ALL.into_iter().filter(|field| field.varies(style))
    }

    /// The property's value in `style`, bound to a feature: a number 0 and a symbol 0 where it
    /// is unset or still varies, a colour none.
    pub(crate) fn value<T>(self, style: &Style<T>) -> Value {
        self.written_value(style)
            .unwrap_or(match self.written(style) {
                Written::Number { .. } => Value::Number(0.0),
                Written::Color(_) => Value::Color(None),
                Written::Symbol(_) => Value::Symbol(0),
            })
    }

    /// The property's value as written in `style`; none where it is unset or varies by feature,
    /// or where a symbol is named, not indexed.
    pub(crate) fn written_value<T>(self, style: &Style<T>) -> Option<Value> {
        match self.written(style) {
            Written::Number { property, .. } => property.and_then(Bound::value).map(Value::Number),
            Written::Color(property) => property
                .and_then(Bound::value)
                .map(|c| Value::Color(Some(c))),
            // A bound symbol is an index: binding resolves a name (classes::symbol_index).
            Written::Symbol(Some(Bound::Value(SymbolRef::Index(index)))) => {
                Some(Value::Symbol(*index))
            }
            Written::Symbol(_) => None,
        }
    }
}
