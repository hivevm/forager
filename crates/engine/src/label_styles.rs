// The styles of a label follow msDrawLabelCache, its labelpoly, labelpoint, and labelcenter
// styles, and computeMarkerBounds, src/mapdraw.c, msGetMarkerSize, src/mapsymbol.c, and
// msDrawLabelBounds and msDrawMarkerSymbol, src/maprendering.c, of MapServer at the release pinned
// in scripts/fetch-msautotest.sh, with the label cache of the label crate (ADR-0028).
// Copyright (c) 1996-2005 Regents of the University of Minnesota.
// Copyright (c) 1996-2011 Regents of the University of Minnesota.
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

use std::borrow::Cow;

use forager_label::{Bounds, Member, Placement, label_poly_at, marker_bounds};
use forager_render::{DisplayItem, Point, Renderer};
use forager_style_ir::{
    Bound, GeomTransform, Label, Map, Rgba, Style, Symbol, SymbolKind, SymbolRef,
};

use crate::classes::{self, scale_factor};
use crate::draw::{Canvas, shade};
use crate::extent::View;
use crate::symbol::{self, marker_size_of};

/// The kind of a label's style that is drawn: by its `GEOMTRANSFORM`, on the box around the text,
/// at the label's point, or at the centre of that box.
enum Kind {
    Poly,
    Point,
    Center,
}

/// The kind of a label's `style`, or why it is not drawn: a labelpoly with a symbol, a labelpoint
/// or labelcenter whose marker is not drawn, or a style of any other transform or none.
fn kind(style: &Style, symbols: &[Symbol]) -> Result<Kind, &'static str> {
    match style.geom_transform {
        Some(GeomTransform::LabelPoly)
            if !matches!(style.symbol, None | Some(Bound::Value(SymbolRef::Index(0)))) =>
        {
            Err("a STYLE of GEOMTRANSFORM labelpoly with a SYMBOL")
        }
        Some(GeomTransform::LabelPoly) => Ok(Kind::Poly),
        Some(GeomTransform::LabelPoint) if !marker_drawn(style, symbols) => Err(
            "a STYLE of GEOMTRANSFORM labelpnt whose SYMBOL is no vector or ellipse of the \
             symbol set",
        ),
        Some(GeomTransform::LabelPoint) => Ok(Kind::Point),
        Some(GeomTransform::LabelCenter) if !marker_drawn(style, symbols) => Err(
            "a STYLE of GEOMTRANSFORM labelcenter whose SYMBOL is no vector or ellipse of the \
             symbol set",
        ),
        Some(GeomTransform::LabelCenter) => Ok(Kind::Center),
        Some(
            GeomTransform::Start
            | GeomTransform::End
            | GeomTransform::Vertices
            | GeomTransform::Centroid,
        )
        | None => {
            Err("a STYLE other than one of GEOMTRANSFORM labelpoly, labelpnt, or labelcenter")
        }
    }
}

/// The first of a label's styles that is not drawn yet, named.
pub(crate) fn unsupported(label: &Label, symbols: &[Symbol]) -> Option<&'static str> {
    label.styles.iter().find_map(|s| kind(s, symbols).err())
}

/// Whether a label's `labelpoint` or `labelcenter` style is drawn: without a symbol, or with
/// symbol 0, which MapServer sizes as 1 by 1 and draws nothing of, or with a vector or an ellipse
/// of the set.
fn marker_drawn(style: &Style, symbols: &[Symbol]) -> bool {
    match &style.symbol {
        None | Some(Bound::Value(SymbolRef::Index(0))) => true,
        Some(Bound::Value(SymbolRef::Index(index))) => symbols
            .get(*index as usize - 1)
            .is_some_and(|s| matches!(s.kind, SymbolKind::Vector | SymbolKind::Ellipse)),
        Some(_) => false,
    }
}

/// What `computeMarkerBounds` takes of a label's first `labelpoint` style: the width and height
/// `msGetMarkerSize` gives its marker, its symbol's anchor point, its angle in degrees, and its
/// `OFFSET`, scaled.
#[derive(Clone, Copy)]
pub(crate) struct LabelMarker {
    size: (f64, f64),
    anchor_point: [f64; 2],
    angle: f64,
    offset: [f64; 2],
}

/// The marker of the first `labelpoint` style of a label's `styles`, as computeMarkerBounds sizes
/// it at the label's scale factor `factor`; `None` without one; or why it is not drawn.
pub(crate) fn label_marker(
    styles: &[Style],
    map: &Map,
    factor: f64,
) -> Result<Option<LabelMarker>, String> {
    let Some(style) = styles
        .iter()
        .find(|s| s.geom_transform == Some(GeomTransform::LabelPoint))
    else {
        return Ok(None);
    };
    let size = marker_size_of(style, &map.symbols, factor)
        .map_err(|what| format!("its labelpoint style draws {what}"))?;
    let anchor_point = match &style.symbol {
        Some(Bound::Value(SymbolRef::Index(index))) if *index > 0 => map
            .symbols
            .get(*index as usize - 1)
            .and_then(|s| s.anchor_point)
            .unwrap_or([0.5, 0.5]),
        _ => [0.5, 0.5],
    };
    let value = |p: &Option<Bound<f64>>| p.as_ref().and_then(Bound::value).unwrap_or(0.0);
    let offset = style.offset.as_ref().map_or([0.0, 0.0], |[x, y]| {
        [x, y].map(|o| o.value().unwrap_or(0.0) * factor)
    });
    Ok(Some(LabelMarker {
        size,
        anchor_point,
        angle: value(&style.angle),
        offset,
    }))
}

/// A property of a label's style that varies by feature, as [`classes::varies`] finds it: one
/// whose written value is a binding or an expression, which `bindLabel` binds to each feature.
#[derive(Clone, Copy)]
enum Field {
    Color,
    OutlineColor,
    Opacity,
    Angle,
    Size,
    Width,
    OutlineWidth,
    Offset(usize),
    PolarOffset(usize),
    LineOffset,
    Symbol,
}

impl Field {
    /// The properties of `style` that vary by feature, in the order they are held.
    fn of(style: &Style) -> impl Iterator<Item = Field> {
        fn varies<T>(p: Option<&Bound<T>>) -> bool {
            p.is_some_and(|p| !matches!(p, Bound::Value(_)))
        }
        let pair = |p: &Option<[Bound<f64>; 2]>, i| varies(p.as_ref().map(|p| &p[i]));
        [
            (varies(style.color.as_ref()), Field::Color),
            (varies(style.outline_color.as_ref()), Field::OutlineColor),
            (varies(style.opacity.as_ref()), Field::Opacity),
            (varies(style.angle.as_ref()), Field::Angle),
            (varies(style.size.as_ref()), Field::Size),
            (varies(style.width.as_ref()), Field::Width),
            (varies(style.outline_width.as_ref()), Field::OutlineWidth),
            (pair(&style.offset, 0), Field::Offset(0)),
            (pair(&style.offset, 1), Field::Offset(1)),
            (pair(&style.polar_offset, 0), Field::PolarOffset(0)),
            (pair(&style.polar_offset, 1), Field::PolarOffset(1)),
            (
                varies(style.line_offset.as_ref().map(|l| &l.distance)),
                Field::LineOffset,
            ),
            (
                matches!(style.symbol, Some(Bound::Binding { .. })),
                Field::Symbol,
            ),
        ]
        .into_iter()
        .filter_map(|(varies, field)| varies.then_some(field))
    }

    /// The field's value in `style`, bound to a feature.
    fn value(self, style: &Style) -> Value {
        let number =
            |p: Option<&Bound<f64>>| Value::Number(p.and_then(Bound::value).unwrap_or(0.0));
        let color = |c: &Option<Bound<Rgba>>| Value::Color(c.as_ref().and_then(Bound::value));
        let pair = |p: &Option<[Bound<f64>; 2]>, i: usize| number(p.as_ref().map(|p| &p[i]));
        match self {
            Field::Color => color(&style.color),
            Field::OutlineColor => color(&style.outline_color),
            Field::Opacity => number(style.opacity.as_ref()),
            Field::Angle => number(style.angle.as_ref()),
            Field::Size => number(style.size.as_ref()),
            Field::Width => number(style.width.as_ref()),
            Field::OutlineWidth => number(style.outline_width.as_ref()),
            Field::Offset(i) => pair(&style.offset, i),
            Field::PolarOffset(i) => pair(&style.polar_offset, i),
            Field::LineOffset => number(style.line_offset.as_ref().map(|l| &l.distance)),
            Field::Symbol => Value::Symbol(match &style.symbol {
                Some(Bound::Value(SymbolRef::Index(index))) => *index,
                _ => 0,
            }),
        }
    }

    /// Sets the field of `style` to `value`, as [`Field::value`] took it.
    fn set(self, value: Value, style: &mut Style) {
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
}

/// A value of a style's property bound to a feature: a colour, `None` where it does not read, a
/// number, or a symbol's index.
#[derive(Clone, Copy)]
enum Value {
    Color(Option<Rgba>),
    Number(f64),
    Symbol(u32),
}

/// The fields of a definition's styles that vary by feature, each with its style's index.
type Layout = Box<[(usize, Field)]>;

/// The labels of consecutive texts of one definition whose styles vary by feature: from text
/// `text`, `len` of them, their values held from `at` in the order of the fields of `layout`.
struct Run {
    text: u32,
    len: u32,
    layout: u32,
    at: usize,
}

/// The styles of the labels cached and the markers of their first `labelpoint` styles: of a label
/// whose styles vary by feature, the values bound to the feature only, by the index of its text,
/// held apart so that a label without costs nothing, its marker computed from them as the cache
/// asks for it; a label whose styles vary by none holds its marker by its definition's index, so
/// that no label holds bounds of its own (Q-9).
#[derive(Default)]
pub(crate) struct LabelStyles {
    /// The fields that vary by feature of each definition whose styles vary, each with its
    /// style's index, by the definition's index, in the order first held.
    layouts: Vec<(u32, Layout)>,
    /// The runs of the labels cached whose styles vary by feature, by their first text, ascending.
    runs: Vec<Run>,
    /// The values bound to the features of those labels, in the order of their texts.
    values: Vec<Value>,
    /// The marker of the first `labelpoint` style of each label whose styles vary by no feature,
    /// by its definition's index, ascending.
    def_markers: Vec<(u32, LabelMarker)>,
}

impl LabelStyles {
    /// Holds the styles of the feature's label of text `text` and definition `def`, written
    /// `written`, and their marker: of `styles`, bound to the feature where they vary by it, else
    /// empty, the values bound to it, from which its marker is computed; else the marker, held by
    /// the definition.
    pub(crate) fn hold(
        &mut self,
        def: u32,
        text: u32,
        written: &[Style],
        styles: &[Style],
        marker: Option<LabelMarker>,
    ) {
        if styles.is_empty() {
            let at = self.def_markers.binary_search_by_key(&def, |(d, _)| *d);
            match (at, marker) {
                (Ok(i), Some(m)) => self.def_markers[i].1 = m,
                (Ok(i), None) => {
                    self.def_markers.remove(i);
                }
                (Err(i), Some(m)) => self.def_markers.insert(i, (def, m)),
                (Err(_), None) => {}
            }
            return;
        }
        let layout = match self.layouts.iter().position(|(d, _)| *d == def) {
            Some(i) => i,
            None => {
                let fields = written
                    .iter()
                    .enumerate()
                    .flat_map(|(i, s)| Field::of(s).map(move |f| (i, f)))
                    .collect();
                self.layouts.push((def, fields));
                self.layouts.len() - 1
            }
        };
        match self.runs.last_mut() {
            Some(run) if run.layout as usize == layout && run.text + run.len == text => {
                run.len += 1;
            }
            _ => self.runs.push(Run {
                text,
                len: 1,
                layout: layout as u32,
                at: self.values.len(),
            }),
        }
        let fields = &self.layouts[layout].1;
        self.values
            .extend(fields.iter().map(|&(i, field)| field.value(&styles[i])));
    }

    /// The bounds of the marker of the label of text `text` and definition `def`, written
    /// `written`, at its point `at`, as computeMarkerBounds computes them: of its first
    /// `labelpoint` style bound to its feature where its styles vary by it, else its definition's
    /// marker; `None` where it has none.
    pub(crate) fn marker_bounds(
        &self,
        map: &Map,
        def: u32,
        text: u32,
        written: &[Style],
        at: Point,
    ) -> Option<Bounds> {
        let marker = if self.run(text).is_some() {
            let i = written
                .iter()
                .position(|s| s.geom_transform == Some(GeomTransform::LabelPoint))?;
            let style = self.style(text, i, &written[i]);
            // The label was cached with this marker, which it computed from the same values.
            label_marker(std::slice::from_ref(&*style), map, scale_factor(map)).ok()??
        } else {
            let i = self
                .def_markers
                .binary_search_by_key(&def, |(d, _)| *d)
                .ok()?;
            self.def_markers[i].1
        };
        Some(marker_bounds(
            at,
            marker.size,
            marker.anchor_point,
            marker.angle,
            marker.offset,
        ))
    }

    /// The run that holds the values of the label of text `text`, where its styles vary by
    /// feature.
    fn run(&self, text: u32) -> Option<&Run> {
        let run = self.runs.partition_point(|r| r.text <= text);
        run.checked_sub(1)
            .map(|r| &self.runs[r])
            .filter(|r| text < r.text + r.len)
    }

    /// The style `i` of the label of text `text`, written `style`, bound to its feature where its
    /// styles vary by it, as `bindLabel` binds it, else as written.
    fn style<'s>(&self, text: u32, i: usize, style: &'s Style) -> Cow<'s, Style> {
        let Some(run) = self.run(text) else {
            return classes::unbound(style);
        };
        let fields = &self.layouts[run.layout as usize].1;
        let at = run.at + (text - run.text) as usize * fields.len();
        let mut bound = None;
        for (&(s, field), &value) in fields.iter().zip(&self.values[at..]) {
            if s == i {
                field.set(value, bound.get_or_insert_with(|| style.clone()));
            }
        }
        match bound {
            Some(bound) => classes::with_opacity(Cow::Owned(bound)),
            None => classes::unbound(style),
        }
    }
}

/// What the styles of the labels drawn are drawn with.
pub(crate) struct Styled<'a> {
    view: &'a View,
    map: &'a Map,
    /// The labels' scale factor.
    factor: f64,
    styles: &'a LabelStyles,
    canvas: Canvas<'a>,
}

impl<'a> Styled<'a> {
    /// What the styles of the labels drawn in an image `width` by `height` draw with.
    pub(crate) fn new(
        map: &'a Map,
        view: &'a View,
        styles: &'a LabelStyles,
        width: u32,
        height: u32,
    ) -> Self {
        // The labels' scale factor: `fixed` refuses SYMBOLSCALEDENOM and a SIZEUNITS other than
        // pixels, so every label's factor is the map's, as each label's `Fixed` holds it.
        let factor = scale_factor(map);
        Self {
            view,
            map,
            factor,
            styles,
            canvas: Canvas::new(&map.symbols, width, height, factor),
        }
    }

    /// Draws the styles of the label of text `text` of feature `feature_id`, `member` placed at
    /// `placement`, in their order, bound to its feature where they vary by it, else `written`, as
    /// msDrawLabelCache draws them before the text (mapdraw.c:4056-4127): a labelpoint style's
    /// marker at its point, a labelpoly style on the labelpoly it computes at `placement`
    /// (mapdraw.c:3799-3860), and a labelcenter style's marker at the centre of the labelpoly's
    /// box, the box of the turned polygon where the label is turned; it never tests the labelpoly
    /// or the labelcenter marker for collisions (maplabel.c:781). Each style is drawn by the kind
    /// of its written style, which `fixed` has checked; one of no kind is not drawn.
    pub(crate) fn draw<R: Renderer>(
        &mut self,
        renderer: &mut R,
        member: &Member,
        placement: &Placement,
        feature_id: u64,
        text: u32,
        written: &[Style],
    ) {
        let point_label = member.label.expect("a drawn label has a box");
        let poly = label_poly_at(member.point, &point_label, placement.anchor);
        for (i, style) in written.iter().enumerate() {
            let Ok(kind) = kind(style, &self.map.symbols) else {
                continue;
            };
            let style = self.styles.style(text, i, style);
            match kind {
                Kind::Point => {
                    draw_label_marker(
                        renderer,
                        member.point,
                        &style,
                        self.map,
                        self.factor,
                        feature_id,
                    );
                }
                Kind::Center => {
                    let [min_x, min_y, max_x, max_y] = poly.bbox;
                    let center = Point {
                        x: (max_x + min_x) / 2.0,
                        y: (max_y + min_y) / 2.0,
                    };
                    draw_label_marker(renderer, center, &style, self.map, self.factor, feature_id);
                }
                Kind::Poly => {
                    draw_label_poly(
                        renderer,
                        &poly,
                        &style,
                        self.view,
                        &mut self.canvas,
                        feature_id,
                    );
                }
            }
        }
    }
}

/// A label's `GEOMTRANSFORM labelpoly` style drawn on its polygon `poly`, as `msDrawLabelBounds`
/// draws it through msDrawShadeSymbol (see [`shade`]): the polygon where the label is turned,
/// else the ring of its box from its lower left corner up, at the canvas's factor: the map's
/// scale factor, which is every label's while `fixed` refuses SYMBOLSCALEDENOM and a SIZEUNITS
/// other than pixels.
fn draw_label_poly<R: Renderer>(
    renderer: &mut R,
    poly: &Bounds,
    style: &Style,
    view: &View,
    canvas: &mut Canvas<'_>,
    feature_id: u64,
) {
    let ring = match &poly.poly {
        Some(points) => points.to_vec(),
        None => {
            let [min_x, min_y, max_x, max_y] = poly.bbox;
            let p = |x, y| Point { x, y };
            vec![
                p(min_x, min_y),
                p(min_x, max_y),
                p(max_x, max_y),
                p(max_x, min_y),
                p(min_x, min_y),
            ]
        }
    };
    let mut ops = Vec::new();
    shade(&mut ops, vec![ring], style, view, canvas);
    for op in ops {
        renderer.draw(&DisplayItem {
            op,
            feature_id: Some(feature_id),
        });
    }
}

/// A label's `GEOMTRANSFORM labelpoint` style drawn at the label's point `at`, or its
/// `labelcenter` style at the centre of its labelpoly's box, as msDrawMarkerSymbol draws it at
/// the label's scale factor `factor`: its vector or ellipse symbol, moved by msAdjustMarkerPos;
/// nothing for symbol 0.
fn draw_label_marker<R: Renderer>(
    renderer: &mut R,
    at: Point,
    style: &Style,
    map: &Map,
    factor: f64,
    feature_id: u64,
) {
    let symbol::Mark::Symbol(s) = symbol::mark(style, &map.symbols) else {
        return;
    };
    let mut ops = Vec::new();
    symbol::symbol_ops(&mut ops, at, style, s, factor);
    for op in ops {
        renderer.draw(&DisplayItem {
            op,
            feature_id: Some(feature_id),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classes::{Reports, Selector};
    use forager_expr::Evaluator;
    use forager_ports::{Coord, Feature, Geometry, Properties, Stored};
    use forager_style_ir::{Layer, LineOffset};

    fn bound<T>(name: &str) -> Bound<T> {
        Bound::Binding {
            binding: name.to_owned(),
        }
    }

    /// A definition's three styles: the first and the third with every property
    /// `classes::varies` checks bound between them, beside one that varies by none; a second
    /// definition of one bound width; and the symbols they name.
    fn written() -> (Vec<Style>, Vec<Style>, Vec<Symbol>) {
        let first = vec![
            Style {
                color: Some(bound("c")),
                size: Some(bound("s")),
                offset: Some([Bound::Value(1.0), bound("y")]),
                opacity: Some(Bound::Value(50.0)),
                geom_transform: Some(GeomTransform::LabelPoint),
                ..Style::default()
            },
            Style {
                color: Some(Bound::Value(Rgba::opaque(1, 2, 3))),
                opacity: Some(Bound::Value(30.0)),
                ..Style::default()
            },
            Style {
                color: Some(Bound::Value(Rgba::opaque(4, 5, 6))),
                outline_color: Some(bound("o")),
                opacity: Some(bound("op")),
                angle: Some(bound("a")),
                width: Some(bound("w")),
                outline_width: Some(bound("ow")),
                offset: Some([bound("x"), Bound::Value(2.0)]),
                polar_offset: Some([bound("pd"), bound("pa")]),
                line_offset: Some(LineOffset {
                    distance: bound("d"),
                    both_sides: true,
                }),
                symbol: Some(bound("sym")),
                ..Style::default()
            },
        ];
        let second = vec![Style {
            width: Some(bound("w")),
            ..Style::default()
        }];
        let symbols: Vec<Symbol> = ["dot", "square"]
            .map(|name| Symbol {
                name: Some(name.to_owned()),
                kind: SymbolKind::Ellipse,
                paths: Vec::new(),
                extent: None,
                filled: true,
                anchor_point: None,
                character: None,
                font: None,
                image: None,
                transparent: None,
                span: None,
            })
            .into();
        (first, second, symbols)
    }

    /// The properties of the feature of text `text`, each distinct.
    fn properties(text: u32) -> Properties {
        let k = f64::from(text);
        // A colour that reads, one of an alpha, one empty, and one that does not read.
        let color = match text % 4 {
            0 => format!("{} 10 20", text % 256),
            1 => format!("#0a0b{:02x}80", text % 256),
            2 => String::new(),
            _ => "red".to_owned(),
        };
        let number = |n: f64| Stored::Number(n.to_string().into());
        Properties::list(vec![
            ("c".into(), Stored::String(color.into())),
            (
                "o".into(),
                Stored::String(format!("1 {} 3", text % 200).into()),
            ),
            // An opacity below 100 and above it.
            ("op".into(), number(40.0 + f64::from(text % 120))),
            ("a".into(), number(k / 3.0)),
            ("s".into(), number(k + 0.5)),
            ("w".into(), number(f64::from(text % 9))),
            ("ow".into(), number(k * 2.0)),
            ("x".into(), number(-k)),
            ("y".into(), number(k / 7.0)),
            ("pd".into(), number(k + 1.25)),
            ("pa".into(), number(k - 1.25)),
            ("d".into(), number(k * 0.1)),
            (
                "sym".into(),
                Stored::String(["dot", "square", "none"][text as usize % 3].into()),
            ),
        ])
    }

    #[test]
    fn a_style_held_for_a_feature_is_the_style_bound_to_it() {
        // Every property `classes::varies` checks bound, in the first of three styles of one
        // definition and the third, beside a style that varies by none; and a second definition
        // of one bound width, its labels interleaved with the first's.
        let (first, second, symbols) = written();
        let layer = Layer::default();
        let mut evaluator = Evaluator::new();
        let mut reports = Reports::default();
        let mut diagnostics = Vec::new();
        let mut selector = Selector {
            layer: &layer,
            layer_index: 0,
            scale: 1.0,
            evaluator: &mut evaluator,
            reports: &mut reports,
            diagnostics: &mut diagnostics,
        };
        let mut styles = LabelStyles::default();
        let mut held = Vec::new();
        for text in 0..3000u32 {
            let properties = properties(text);
            let feature = Feature {
                id: u64::from(text),
                geometry: Geometry::Point(Coord { x: 0.0, y: 0.0 }),
                properties,
            };
            let (def, written) = if text % 5 == 2 || text % 7 == 3 {
                (1, &second)
            } else {
                (0, &first)
            };
            let bound: Vec<Style> = written
                .iter()
                .map(|s| {
                    selector
                        .bind_label_style(s, &feature, &symbols, (0, 0))
                        .into_owned()
                })
                .collect();
            styles.hold(def, text, written, &bound, None);
            held.push((written, bound));
        }
        assert!(diagnostics.is_empty());
        for (text, (written, bound)) in held.iter().enumerate() {
            for (i, style) in written.iter().enumerate() {
                assert_eq!(
                    styles.style(text as u32, i, style).as_ref(),
                    &bound[i],
                    "text {text}, style {i}"
                );
            }
        }
    }
}
