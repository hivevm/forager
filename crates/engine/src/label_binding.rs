// The binding of a label's properties to a feature follows bindLabel, src/maputil.c, the
// defaults of initLabel and loadLabel, src/mapfile.c, and the bounds of msAddLabelGroup,
// src/maplabel.c, of MapServer at the release pinned in scripts/fetch-msautotest.sh (ADR-0031).
// Copyright (c) 1996-2005 Regents of the University of Minnesota.
// Copyright (c) 1996-2011 Regents of the University of Minnesota.
// Copyright (c) 1996-2013 Regents of the University of Minnesota.
// Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
// associated documentation files (the "Software"), to deal in the Software without restriction,
// including without limitation the rights to use, copy, modify, merge, publish, distribute,
// sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions: The above copyright notice and this
// permission notice shall be included in all copies of this Software or works derived from this
// Software. THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED,
// INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE
// AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

//! What of a label is settled before its features, and its properties bound to each feature as
//! `bindLabel` binds them, its bound values read through the one port of `bind*Attribute` the
//! styles' binding reads them through (`classes.rs`).

use forager_label::{Align, Anchor, MAX_PRIORITY, Position};
use forager_ports::Properties;
use forager_style_ir::{
    Align as IrAlign, Bound, Expr, Force, Label, Map, Position as IrPosition, Rgba,
};

use crate::classes::{self, FeatureProperties, attribute, bind_double, bind_integer, color};
use crate::label_styles;

/// A label's size where none is given, MapServer's `MS_MEDIUM`.
const DEFAULT_SIZE: f64 = 10.0;
/// The bounds of a label's size where none are given, `MS_MINFONTSIZE` and `MS_MAXFONTSIZE`.
const DEFAULT_MIN_SIZE: f64 = 4.0;
const DEFAULT_MAX_SIZE: f64 = 256.0;
/// A label's colour where none is given.
const DEFAULT_COLOR: Rgba = Rgba::opaque(0, 0, 0);
/// A label's `OUTLINEWIDTH` where none is given.
const DEFAULT_OUTLINE_WIDTH: f64 = 1.0;

/// A property of a label as it is drawn: a value, or the feature's property it is bound to.
#[derive(Clone, Copy)]
pub(crate) enum Prop<'a, T> {
    Value(T),
    Binding(&'a str),
}

/// A label's `PRIORITY` as it is drawn: a value from 1 to [`MAX_PRIORITY`], the feature's property
/// it is bound to, or the expression that computes it.
#[derive(Clone, Copy)]
pub(crate) enum Priority<'a> {
    Value(u8),
    Binding(&'a str),
    Expr(&'a Expr),
}

/// What of a label is settled before its features, once checked to be drawable.
pub(crate) struct Fixed<'a> {
    /// The label's text, or `None` where the label, its class, and its layer's `LABELITEM` give
    /// none and the feature's own `TEXT` is read in their place (`msShapeGetLabelAnnotation`).
    pub(crate) text: Option<&'a Expr>,
    /// The label's `FONT`, needed only where a text is laid out (`msComputeTextPath`).
    pub(crate) fonts: Option<&'a str>,
    pub(crate) wrap: Option<char>,
    /// `MAXLENGTH` as MapServer holds it, an `int`, 0 where the label has none.
    pub(crate) max_length: i32,
    pub(crate) size: Prop<'a, f64>,
    pub(crate) min_size: f64,
    pub(crate) max_size: f64,
    pub(crate) color: Prop<'a, Option<Rgba>>,
    pub(crate) outline_color: Prop<'a, Option<Rgba>>,
    pub(crate) outline_width: f64,
    pub(crate) offset: [Prop<'a, f64>; 2],
    pub(crate) angle: Prop<'a, f64>,
    pub(crate) position: Prop<'a, Position>,
    pub(crate) align: Prop<'a, Align>,
    pub(crate) priority: Priority<'a>,
    /// The layer's scale factor, which its labels inherit (msUpdateClassScaleFactor) and which
    /// scales a label's size, offset, buffer, outline, and styles.
    pub(crate) factor: f64,
    /// The image's resolution factor, which scales a label's `MINSIZE`, `MAXSIZE`, and
    /// `MINDISTANCE`.
    pub(crate) resolution: f64,
    pub(crate) buffer: f64,
    pub(crate) force: bool,
    pub(crate) partials: bool,
    pub(crate) min_distance: f64,
}

/// A label's properties for one feature, as `bindLabel` leaves them.
pub(crate) struct BoundLabel {
    pub(crate) size: f64,
    pub(crate) color: Option<Rgba>,
    pub(crate) outline_color: Option<Rgba>,
    pub(crate) offset: [i32; 2],
    pub(crate) angle: f64,
    /// `None` where a bound value names no position or alignment.
    pub(crate) position: Option<Position>,
    pub(crate) align: Option<Align>,
    /// From 1 to [`MAX_PRIORITY`].
    pub(crate) priority: u8,
}

/// A C `(int)` cast of a `double`, as the x86-64 build at the pin computes it: `cvttsd2si`
/// truncates toward zero, and gives `INT_MIN` for a value past an `int` or no number.
fn int_cast(x: f64) -> i32 {
    if x > -2_147_483_649.0 && x < 2_147_483_648.0 {
        x as i32
    } else {
        i32::MIN
    }
}

/// A label's properties for a feature with `properties`, as `bindLabel` binds them, with
/// `evaluate` giving the number an expression computes for the feature.
pub(crate) fn bind(
    fixed: &Fixed<'_>,
    properties: &Properties,
    evaluate: impl FnOnce(&Expr) -> f64,
) -> BoundLabel {
    let lookup = FeatureProperties(properties);
    let read = |binding: &str| attribute(&lookup, binding);
    let number = |prop: Prop<'_, f64>, unset: i32| match prop {
        Prop::Value(v) => v,
        Prop::Binding(b) => f64::from(bind_integer(&read(b), unset)),
    };
    let colour = |prop: Prop<'_, Option<Rgba>>| match prop {
        Prop::Value(c) => c,
        Prop::Binding(b) => color(&read(b)),
    };
    let position = match fixed.position {
        Prop::Value(p) => Some(p),
        Prop::Binding(b) => bound_position(&read(b)),
    };
    let align = match fixed.align {
        Prop::Value(a) => Some(a),
        Prop::Binding(b) => bound_align(&read(b)),
    };
    BoundLabel {
        size: number(fixed.size, 1),
        color: colour(fixed.color),
        outline_color: colour(fixed.outline_color),
        offset: fixed.offset.map(|o| number(o, 0) as i32),
        angle: match fixed.angle {
            Prop::Value(a) => a,
            // bindDoubleAttribute after the angle is set to 0.
            Prop::Binding(b) => bind_double(&read(b), 0.0),
        },
        position,
        align,
        // bindIntegerAttribute after the priority is set to 1, or msEvalDoubleExpression's
        // value assigned to the int, then msAddLabelGroup's bounds.
        priority: match fixed.priority {
            Priority::Value(p) => p,
            Priority::Binding(b) => bounded_priority(bind_integer(&read(b), 1)),
            Priority::Expr(expr) => bounded_priority(int_cast(evaluate(expr))),
        },
    }
}

/// A priority bounded to 1 to [`MAX_PRIORITY`], as `msAddLabelGroup` bounds it.
fn bounded_priority(priority: i32) -> u8 {
    priority.clamp(1, i32::from(MAX_PRIORITY)) as u8
}

/// A bound `POSITION` as `bindLabel` reads it: a number rounded to an integer, which where it is
/// not 0 is MapServer's position constant, unchecked; else two characters naming a position,
/// without regard to case. `None` where it names none.
fn bound_position(text: &[u8]) -> Option<Position> {
    let anchors = [
        Anchor::Ul,
        Anchor::Lr,
        Anchor::Ur,
        Anchor::Ll,
        Anchor::Cr,
        Anchor::Cl,
        Anchor::Uc,
        Anchor::Lc,
        Anchor::Cc,
    ];
    match bind_integer(text, 0) {
        0 => {
            let names = [
                b"ul", b"lr", b"ur", b"ll", b"cr", b"cl", b"uc", b"lc", b"cc",
            ];
            if text.len() != 2 {
                return None;
            }
            names
                .iter()
                .position(|name| text.eq_ignore_ascii_case(*name))
                .map(|i| Position::At(anchors[i]))
        }
        // MS_UL (101) to MS_CC (109), and MS_AUTO.
        n @ 101..=109 => Some(Position::At(anchors[(n - 101) as usize])),
        110 => Some(Position::Auto),
        _ => Some(Position::At(Anchor::Other)),
    }
}

/// A bound `ALIGN` as `bindLabel` reads it: a number rounded to an integer, which where it is not
/// 0 is MapServer's alignment constant, `MS_ALIGN_CENTER` 2 and `MS_ALIGN_RIGHT` 3 and any other
/// the left of a line read left to right; else a value of at least four characters that begins
/// with `cente`, `left`, or `right`, without regard to case. `None` where it names none.
fn bound_align(text: &[u8]) -> Option<Align> {
    match bind_integer(text, 0) {
        0 => {
            let starts = |prefix: &[u8]| {
                text.len() >= prefix.len() && text[..prefix.len()].eq_ignore_ascii_case(prefix)
            };
            if text.len() < 4 {
                None
            } else if starts(b"cente") {
                Some(Align::Center)
            } else if starts(b"left") {
                Some(Align::Left)
            } else if starts(b"right") {
                Some(Align::Right)
            } else {
                None
            }
        }
        2 => Some(Align::Center),
        3 => Some(Align::Right),
        _ => Some(Align::Left),
    }
}

/// A property that is a value or bound to a feature's property, or why it is not drawn: one
/// computed by an expression is not evaluated yet.
fn prop<'a, T, U>(
    property: Option<&'a Bound<T>>,
    what: &str,
    default: U,
    value: impl Fn(&T) -> U,
) -> Result<Prop<'a, U>, String> {
    match property {
        None => Ok(Prop::Value(default)),
        Some(Bound::Value(v)) => Ok(Prop::Value(value(v))),
        Some(Bound::Binding { binding }) => Ok(Prop::Binding(binding)),
        Some(Bound::Expr(_)) => Err(format!(
            "its {what} is computed by an expression, which is not drawn yet"
        )),
    }
}

/// What of `label` is settled before its features, or why the label is not drawn: a property
/// computed by an expression, or one whose meaning is not drawn yet, is not guessed at.
pub(crate) fn fixed<'a>(
    map: &Map,
    label: &'a Label,
    text: Option<&'a Expr>,
    factor: f64,
) -> Result<Fixed<'a>, String> {
    let fonts = match &label.font {
        Some(Bound::Value(fonts)) => Some(fonts.as_str()),
        Some(_) => return Err("its FONT varies by feature, which is not drawn yet".to_owned()),
        None => None,
    };
    let unsupported = [
        (
            label.angle_mode.is_some(),
            "an ANGLE AUTO, AUTO2, or FOLLOW",
        ),
        (label.shadow_color.is_some(), "a SHADOWCOLOR"),
        label_styles::unsupported(label, &map.symbols).map_or((false, ""), |what| (true, what)),
        (
            label.min_scale_denom.is_some() || label.max_scale_denom.is_some(),
            "a scale range",
        ),
    ];
    if let Some((_, what)) = unsupported.iter().find(|(has, _)| *has) {
        return Err(format!("it has {what}, which is not drawn yet"));
    }
    let offset = match &label.offset {
        None => [Prop::Value(0.0), Prop::Value(0.0)],
        Some([x, y]) => [
            prop(Some(x), "OFFSET", 0.0, |v| *v)?,
            prop(Some(y), "OFFSET", 0.0, |v| *v)?,
        ],
    };
    Ok(Fixed {
        text,
        fonts,
        // WRAP is the first character of its string.
        wrap: label.wrap.as_deref().and_then(|w| w.chars().next()),
        // labelObj's maxlength starts at 0 (initLabel); getInteger checks the double above 0,
        // then stores it as `(int)` (mapfile.c:312-318).
        max_length: label.max_length.map_or(0, int_cast),
        size: prop(label.size.as_ref(), "SIZE", DEFAULT_SIZE, |v| *v)?,
        min_size: label.min_size.unwrap_or(DEFAULT_MIN_SIZE),
        max_size: label.max_size.unwrap_or(DEFAULT_MAX_SIZE),
        color: prop(label.color.as_ref(), "COLOR", Some(DEFAULT_COLOR), |c| {
            Some(*c)
        })?,
        outline_color: prop(label.outline_color.as_ref(), "OUTLINECOLOR", None, |c| {
            Some(*c)
        })?,
        outline_width: label.outline_width.unwrap_or(DEFAULT_OUTLINE_WIDTH),
        offset,
        angle: prop(label.angle.as_ref(), "ANGLE", 0.0, |v| *v)?,
        position: prop(
            label.position.as_ref(),
            "POSITION",
            Position::At(Anchor::Cc),
            |p| position(*p),
        )?,
        align: prop(label.align.as_ref(), "ALIGN", Align::Left, |a| match a {
            IrAlign::Left => Align::Left,
            IrAlign::Center => Align::Center,
            IrAlign::Right => Align::Right,
        })?,
        priority: match &label.priority {
            None => Priority::Value(1),
            // A value held as an int and bounded as msAddLabelGroup bounds it, such as the 0 the
            // frontend gives a condition, which MapServer evaluates to `true` or `false` and
            // `atof` reads as 0.
            Some(Bound::Value(p)) => Priority::Value(bounded_priority(int_cast(*p))),
            Some(Bound::Binding { binding }) => Priority::Binding(binding),
            Some(Bound::Expr(expr)) => Priority::Expr(expr),
        },
        factor,
        resolution: classes::resolution_factor(map),
        buffer: label.buffer.unwrap_or(0.0),
        force: label.force == Some(Force::On),
        partials: label.partials.unwrap_or(false),
        min_distance: label.min_distance.unwrap_or(-1.0),
    })
}

/// A literal `POSITION` as the label crate places it.
fn position(p: IrPosition) -> Position {
    let anchor = match p {
        IrPosition::Ul => Anchor::Ul,
        IrPosition::Uc => Anchor::Uc,
        IrPosition::Ur => Anchor::Ur,
        IrPosition::Cl => Anchor::Cl,
        IrPosition::Cc => Anchor::Cc,
        IrPosition::Cr => Anchor::Cr,
        IrPosition::Ll => Anchor::Ll,
        IrPosition::Lc => Anchor::Lc,
        IrPosition::Lr => Anchor::Lr,
        IrPosition::Auto => return Position::Auto,
    };
    Position::At(anchor)
}

#[cfg(test)]
mod tests {
    use forager_expr::Evaluator;
    use forager_ports::{Coord, Feature, Geometry, Stored};
    use forager_style_ir::{Layer, Style};

    use super::*;
    use crate::classes::Reports;

    #[test]
    fn a_label_and_a_style_bound_to_one_property_read_it_through_one_port() {
        // bindStyle and bindLabel read a bound value by the same bind*Attribute: " 12abc" is 12
        // to `atof`, a style's SIZE and a label's ANGLE by bindDoubleAttribute, a label's SIZE
        // by bindIntegerAttribute.
        let properties = Properties::list(vec![("p".into(), Stored::String(" 12abc".into()))]);
        let read = attribute(&FeatureProperties(&properties), "p");
        assert_eq!(read, b" 12abc");
        let p = || Bound::Binding {
            binding: "p".to_owned(),
        };
        let style: Style = Style {
            size: Some(p()),
            ..Style::default()
        };
        let feature = Feature {
            id: 0,
            geometry: Geometry::Point(Coord { x: 0.0, y: 0.0 }),
            properties,
        };
        let layer = Layer::default();
        let mut evaluator = Evaluator::new();
        let mut reports = Reports::default();
        let mut diagnostics = Vec::new();
        let mut binder = classes::Binder {
            layer: &layer,
            layer_index: 0,
            evaluator: &mut evaluator,
            reports: &mut reports,
            diagnostics: &mut diagnostics,
        };
        let bound_style = binder.bind_label_style(&style, &feature, &[], (0, 0));
        assert_eq!(
            bound_style.size,
            Some(Bound::Value(bind_double(&read, 1.0)))
        );
        assert_eq!(bound_style.size, Some(Bound::Value(12.0)));
        let label = Label {
            size: Some(p()),
            angle: Some(p()),
            ..Label::default()
        };
        let fixed = fixed(&Map::default(), &label, None, 1.0).unwrap_or_else(|why| panic!("{why}"));
        let bound_label = bind(&fixed, &feature.properties, |_| unreachable!());
        assert_eq!(bound_label.size, f64::from(bind_integer(&read, 1)));
        assert_eq!(bound_label.size, 12.0);
        assert_eq!(bound_label.angle, bind_double(&read, 0.0));
        assert_eq!(bound_label.angle, 12.0);
    }
}
