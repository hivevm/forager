// The styles of a label follow msDrawLabelCache, its labelpoly, labelpoint, and labelcenter
// styles, and computeMarkerBounds, src/mapdraw.c, msGetMarkerSize, src/mapsymbol.c, and
// msDrawLabelBounds and msDrawMarkerSymbol, src/maprendering.c, of MapServer at the release pinned
// in scripts/fetch-msautotest.sh, with the label cache of the label crate (ADR-0031).
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

use forager_label::{Anchor, Bounds, PointLabel, label_poly_at, marker_bounds};
use forager_render::{Draw, Point};
use forager_style_ir::{Bound, LabelMarker, LabelTransform, Map, Style, SymbolRef};
use forager_symbolize::{
    Canvas, Factors, LabelGeometry, MarkerValues, Pixmaps, Refusals, View, label_style_marker,
    label_style_ops,
};

use crate::bindable::{Field, Value};
use crate::classes;
use crate::labels::{LabelAt, index};

/// The marker of the first `labelpoint` style of a label's `styles`, as computeMarkerBounds sizes
/// it at the label's scale factor `factor`, a pixmap's by its image among `pixmaps`; `None`
/// without one; or why it is not drawn.
pub(crate) fn label_marker(
    styles: &[Style<LabelTransform>],
    map: &Map,
    pixmaps: &Pixmaps,
    factor: f64,
) -> Result<Option<LabelMarker>, String> {
    let Some(style) = styles
        .iter()
        .find(|s| s.geom_transform == LabelTransform::Point)
    else {
        return Ok(None);
    };
    let values = marker_values(|field| field.written_value(style));
    label_style_marker(style, values, &style.symbol, &map.symbols, pixmaps, factor).map(Some)
}

/// The values of a `labelpoint` style's marker that `value` gives by the properties of
/// [`Field::ALL`], each that gives none taken as unset.
fn marker_values(value: impl Fn(Field) -> Option<Value>) -> MarkerValues {
    let number = |field| match value(field) {
        Some(Value::Number(n)) => Some(n),
        _ => None,
    };
    MarkerValues {
        size: number(Field::Size),
        angle: number(Field::Angle).unwrap_or(0.0),
        offset: [0, 1].map(|i| number(Field::Offset(i)).unwrap_or(0.0)),
    }
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
        written: &[Style<LabelTransform>],
        styles: &[Style<LabelTransform>],
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
            _ => crate::labels::push(
                &mut self.runs,
                Run {
                    text,
                    len: 1,
                    layout: index(layout),
                    at: self.values.len(),
                },
            ),
        }
        let fields = &self.layouts[layout].1;
        crate::labels::make_room(&mut self.values, fields.len());
        self.values
            .extend(fields.iter().map(|&(i, field)| field.value(&styles[i])));
    }

    /// The bounds of the marker of the label of text `text` and definition `def`, written
    /// `written`, at its point `at`, as computeMarkerBounds computes them: of its first
    /// `labelpoint` style bound to its feature where its styles vary by it, a pixmap's sized by its
    /// image among `pixmaps`, else its definition's marker; `None` where it has none.
    pub(crate) fn marker_bounds(
        &self,
        (map, pixmaps): (&Map, &Pixmaps),
        def: u32,
        text: u32,
        written: &[Style<LabelTransform>],
        at: Point,
        factor: f64,
    ) -> Option<Bounds> {
        let marker = if self.run(text).is_some() {
            let i = written
                .iter()
                .position(|s| s.geom_transform == LabelTransform::Point)?;
            self.bound_marker(text, i, &written[i], map, pixmaps, factor)?
        } else {
            let i = self
                .def_markers
                .binary_search_by_key(&def, |(d, _)| *d)
                .ok()?;
            self.def_markers[i].1
        };
        Some(marker_bounds(at, &marker))
    }

    /// The marker of the `labelpoint` style `i`, written `style`, of the label of text `text`
    /// whose styles vary by feature, of the values bound to its feature, without the style
    /// rebuilt: the cache asks for it as often as it tests the label.
    fn bound_marker(
        &self,
        text: u32,
        i: usize,
        style: &Style<LabelTransform>,
        map: &Map,
        pixmaps: &Pixmaps,
        factor: f64,
    ) -> Option<LabelMarker> {
        let run = self.run(text)?;
        let fields = &self.layouts[run.layout as usize].1;
        let at = run.at + (text - run.text) as usize * fields.len();
        let held = |field| {
            fields
                .iter()
                .zip(&self.values[at..])
                .find(|&(&(s, f), _)| s == i && f == field)
                .map(|(_, &value)| value)
        };
        let values = marker_values(|field| held(field).or_else(|| field.written_value(style)));
        let symbol = match held(Field::Symbol) {
            Some(Value::Symbol(index)) => &Some(Bound::Value(SymbolRef::Index(index))),
            _ => &style.symbol,
        };
        // The label was cached with this marker, which it computed from the same values.
        label_style_marker(style, values, symbol, &map.symbols, pixmaps, factor).ok()
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
    fn style<'s>(
        &self,
        text: u32,
        i: usize,
        style: &'s Style<LabelTransform>,
    ) -> Cow<'s, Style<LabelTransform>> {
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

/// Where a label's styles bound to its feature are found: held for the label cache by the index
/// of its text, or bound for a label drawn at once, none where they vary by no feature.
pub(crate) enum StylesOf<'a> {
    Held { styles: &'a LabelStyles, text: u32 },
    Bound(&'a [Style<LabelTransform>]),
}

impl StylesOf<'_> {
    /// The style `i`, written `style`, bound to the label's feature where its styles vary by it,
    /// as `bindLabel` binds it, else as written.
    fn style<'s>(
        &'s self,
        i: usize,
        style: &'s Style<LabelTransform>,
    ) -> Cow<'s, Style<LabelTransform>> {
        match self {
            Self::Held { styles, text } => styles.style(*text, i, style),
            Self::Bound([]) => classes::unbound(style),
            Self::Bound(bound) => Cow::Borrowed(&bound[i]),
        }
    }
}

/// A label drawn: of definition `at`, at `point`, its text's `label` placed at its anchor where it
/// has text, its styles bound to its feature as `styles` gives them.
pub(crate) struct Placed<'l> {
    pub(crate) at: LabelAt,
    pub(crate) point: Point,
    pub(crate) label: Option<(PointLabel, Anchor)>,
    pub(crate) styles: &'l StylesOf<'l>,
}

/// What the styles of the labels drawn are drawn with.
pub(crate) struct Styled<'a> {
    view: &'a View,
    map: &'a Map,
    /// The factors of each layer's labels, by the layer's index.
    factors: &'a [Factors],
    canvas: Canvas<'a>,
}

impl<'a> Styled<'a> {
    /// What the styles of the labels drawn in an image `width` by `height` draw with, at
    /// `factors`, each layer's by its index, a pixmap with its image among `pixmaps`.
    pub(crate) fn new(
        map: &'a Map,
        view: &'a View,
        factors: &'a [Factors],
        pixmaps: &'a Pixmaps,
        width: u32,
        height: u32,
    ) -> Self {
        Self {
            view,
            map,
            factors,
            // A label's styles inherit its factors, which are its layer's
            // (msUpdateClassScaleFactor), as each label's `Fixed` holds them: each draw sets its
            // layer's on the canvas before a style is drawn.
            canvas: Canvas::new(&map.symbols, width, height, Factors::new(1.0, 1.0))
                .with_pixmaps(pixmaps),
        }
    }

    /// Marks the labels as drawn into a layer's image of its own, from whose corner a hatch
    /// counts its lines (see [`Canvas::in_own_image`]).
    pub(crate) fn in_own_image(&mut self) {
        self.canvas.in_own_image();
    }

    /// What the styles of the labels drawn left undrawn since this was last asked: the hatches of
    /// labelpoly styles refused (Q-9), as a polygon's are refused, and the pixmap symbols of
    /// labelpoint and labelcenter styles whose image is refused.
    pub(crate) fn take_refusals(&mut self) -> Refusals {
        self.canvas.take_refusals()
    }

    /// Draws the styles of `label` of feature `feature_id`, written in its definition, in their
    /// order, as its `styles` gives them bound to its feature where they vary by it, else as
    /// written, as msDrawLabelCache draws them before the text (see [`label_style_ops`]): on the
    /// labelpoly it computes at its anchor (mapdraw.c:3799-3860), which it never tests for
    /// collisions (maplabel.c:781). No style is refused here: `fixed` refuses the whole label
    /// where `label_style_refused` refuses any of its styles as written.
    pub(crate) fn draw<R: Draw>(&mut self, renderer: &mut R, label: &Placed<'_>, feature_id: u64) {
        let written = &label.at.label(self.map).styles;
        if written.is_empty() {
            return;
        }
        // A label without text draws its labelpoint styles alone (mapdraw.c:4056-4069); one with a
        // labelpoly or labelcenter style is refused before it is drawn.
        let poly = label
            .label
            .map(|(text, anchor)| label_poly_at(label.point, &text, anchor));
        let at = LabelGeometry {
            point: label.point,
            poly: poly
                .as_ref()
                .map(|poly| (poly.bbox, poly.poly.as_ref().map(|p| &p[..]))),
        };
        self.canvas.set_factors(self.factors[label.at.0]);
        let mut ops = Vec::new();
        for (i, style) in written.iter().enumerate() {
            let style = label.styles.style(i, style);
            label_style_ops(&mut ops, &style, &at, self.view, &mut self.canvas);
        }
        crate::draw_items(renderer, feature_id, ops);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classes::{Binder, Reports};
    use forager_expr::Evaluator;
    use forager_ports::{Coord, Feature, Geometry, Properties, Stored};
    use forager_style_ir::{Layer, LineOffset, Rgba, Symbol, SymbolKind};

    fn bound<T>(name: &str) -> Bound<T> {
        Bound::Binding {
            binding: name.to_owned(),
        }
    }

    /// A definition's three styles: the first and the third with every property
    /// `classes::varies` checks bound between them, beside one that varies by none; a second
    /// definition of one bound width; and the symbols they name.
    fn written() -> (
        Vec<Style<LabelTransform>>,
        Vec<Style<LabelTransform>>,
        Vec<Symbol>,
    ) {
        let first = vec![
            Style {
                color: Some(bound("c")),
                size: Some(bound("s")),
                angle: Some(bound("a")),
                symbol: Some(bound("sym")),
                offset: Some([Bound::Value(1.0), bound("y")]),
                opacity: Some(Bound::Value(50.0)),
                ..Style::new(LabelTransform::Point)
            },
            Style {
                color: Some(Bound::Value(Rgba::opaque(1, 2, 3))),
                opacity: Some(Bound::Value(30.0)),
                ..Style::new(LabelTransform::Poly)
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
                ..Style::new(LabelTransform::Poly)
            },
        ];
        let second = vec![Style {
            width: Some(bound("w")),
            ..Style::new(LabelTransform::Poly)
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
        let mut binder = Binder {
            layer: &layer,
            layer_index: 0,
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
            let bound: Vec<Style<LabelTransform>> = written
                .iter()
                .map(|s| {
                    binder
                        .bind_label_style(s, &feature, &symbols, (0, 0))
                        .into_owned()
                })
                .collect();
            styles.hold(def, text, written, &bound, None);
            held.push((written, bound));
        }
        assert!(diagnostics.is_empty());
        let map = Map {
            symbols,
            ..Map::default()
        };
        let at = Point { x: 10.0, y: 20.0 };
        for (text, (written, bound)) in held.iter().enumerate() {
            for (i, style) in written.iter().enumerate() {
                assert_eq!(
                    styles.style(index(text), i, style).as_ref(),
                    &bound[i],
                    "text {text}, style {i}"
                );
            }
            // The marker the cache asks for is the one of the bound labelpoint style.
            let def = u32::from(!std::ptr::eq(*written, &first));
            let marker = label_marker(
                bound,
                &map,
                &Pixmaps::new(),
                classes::resolution_factor(&map),
            )
            .unwrap()
            .map(|m| marker_bounds(at, &m));
            assert_eq!(
                styles.marker_bounds((&map, &Pixmaps::new()), def, index(text), written, at, 1.0),
                marker,
                "text {text}"
            );
        }
    }

    #[test]
    fn a_labels_marker_is_its_first_labelpoint_styles() {
        // computeMarkerBounds takes the first labelpoint style of a label (mapdraw.c): not a
        // labelpoly style before it, nor a labelpoint style after it. MapServer's default symbol
        // is 1 by 1, anchored at its centre; the OFFSET is scaled by the factor, 2.
        let style = |transform, angle: f64, offset: [f64; 2]| Style {
            angle: Some(Bound::Value(angle)),
            offset: Some(offset.map(Bound::Value)),
            ..Style::new(transform)
        };
        let styles = [
            style(LabelTransform::Poly, 10.0, [7.0, 8.0]),
            style(LabelTransform::Point, 30.0, [2.0, 3.0]),
            style(LabelTransform::Point, 45.0, [5.0, 5.0]),
        ];
        let marker = label_marker(&styles, &Map::default(), &Pixmaps::new(), 2.0).unwrap();
        assert_eq!(
            marker,
            Some(LabelMarker {
                size: (1.0, 1.0),
                anchor_point: [0.5, 0.5],
                angle: 30.0,
                offset: [4.0, 6.0],
            })
        );
    }

    #[test]
    fn the_runs_of_labels_of_alternating_definitions_grow_by_a_quarter() {
        // Verifies: Q-9
        // Labels of two definitions in turn start a run each; the list of runs grows as the
        // label cache's lists grow, by a quarter, not by doubling, which would hold up to three
        // times what it fills while it moves.
        let (_, second, _) = written();
        let mut styles = LabelStyles::default();
        for text in 0..5_000u32 {
            styles.hold(text % 2, text, &second, &second, None);
            let (len, capacity) = (styles.runs.len(), styles.runs.capacity());
            assert_eq!(len, text as usize + 1);
            assert!(capacity <= len + len / 4 + 8, "{len} runs in {capacity}");
        }
    }

    /// Every property a feature may bind, written out apart from `Field::ALL`, so that a property
    /// left out of that is found.
    const PROPERTIES: [Field; 13] = [
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

    /// `style` with `field` bound to the feature's property `p`.
    fn bound_to(field: Field, style: &mut Style<LabelTransform>) {
        match field {
            Field::Color => style.color = Some(bound("p")),
            Field::OutlineColor => style.outline_color = Some(bound("p")),
            Field::Opacity => style.opacity = Some(bound("p")),
            Field::Angle => style.angle = Some(bound("p")),
            Field::Size => style.size = Some(bound("p")),
            Field::Width => style.width = Some(bound("p")),
            Field::OutlineWidth => style.outline_width = Some(bound("p")),
            Field::Offset(i) => style.offset.as_mut().unwrap()[i] = bound("p"),
            Field::PolarOffset(i) => style.polar_offset.as_mut().unwrap()[i] = bound("p"),
            Field::LineOffset => style.line_offset.as_mut().unwrap().distance = bound("p"),
            Field::Symbol => style.symbol = Some(bound("p")),
        }
    }

    /// The value of `field` the feature's property holds, and the value it binds to.
    fn expected(field: Field) -> (&'static str, Value) {
        match field {
            Field::Color | Field::OutlineColor => {
                ("#0a0b0c", Value::Color(Some(Rgba::opaque(10, 11, 12))))
            }
            Field::Opacity => ("70", Value::Number(70.0)),
            Field::Angle => ("12.5", Value::Number(12.5)),
            Field::Size => ("9.5", Value::Number(9.5)),
            Field::Width => ("3.25", Value::Number(3.25)),
            Field::OutlineWidth => ("2.5", Value::Number(2.5)),
            Field::Offset(0) => ("-4", Value::Number(-4.0)),
            Field::Offset(_) => ("6", Value::Number(6.0)),
            Field::PolarOffset(0) => ("8", Value::Number(8.0)),
            Field::PolarOffset(_) => ("45", Value::Number(45.0)),
            Field::LineOffset => ("1.5", Value::Number(1.5)),
            Field::Symbol => ("SQUARE", Value::Symbol(2)),
        }
    }

    #[test]
    fn each_bindable_property_takes_its_bound_value_bound_varied_held_and_drawn() {
        // Each property of the list, bound alone in a labelpnt style: the style varies, binding
        // gives the property its bound value, and the style held for a label and drawn from it,
        // and its marker, are those of the bound style. The matches name every property, so one
        // added to the enum is stated here too.
        let (_, _, symbols) = written();
        let map = Map {
            symbols,
            ..Map::default()
        };
        let layer = Layer::default();
        let mut evaluator = Evaluator::new();
        let mut reports = Reports::default();
        let mut diagnostics = Vec::new();
        let mut binder = Binder {
            layer: &layer,
            layer_index: 0,
            evaluator: &mut evaluator,
            reports: &mut reports,
            diagnostics: &mut diagnostics,
        };
        let at = Point { x: 10.0, y: 20.0 };
        for (text, field) in PROPERTIES.into_iter().enumerate() {
            let text = index(text);
            let mut style = Style {
                offset: Some([Bound::Value(1.0), Bound::Value(2.0)]),
                polar_offset: Some([Bound::Value(3.0), Bound::Value(4.0)]),
                line_offset: Some(LineOffset {
                    distance: Bound::Value(5.0),
                    both_sides: false,
                }),
                ..Style::new(LabelTransform::Point)
            };
            bound_to(field, &mut style);
            assert!(classes::varies(&style), "{field:?} varies");
            assert_eq!(Field::of(&style).collect::<Vec<_>>(), [field]);
            let (property, value) = expected(field);
            let feature = Feature {
                id: u64::from(text),
                geometry: Geometry::Point(Coord { x: 0.0, y: 0.0 }),
                properties: Properties::list(vec![("p".into(), Stored::String(property.into()))]),
            };
            let bound = binder
                .bind_label_style(&style, &feature, &map.symbols, (0, 0))
                .into_owned();
            assert_eq!(field.value(&bound), value, "{field:?} bound");
            let mut styles = LabelStyles::default();
            let written = [style];
            styles.hold(0, text, &written, std::slice::from_ref(&bound), None);
            let drawn = styles.style(text, 0, &written[0]);
            assert_eq!(field.value(&drawn), value, "{field:?} drawn");
            assert_eq!(drawn.as_ref(), &bound, "{field:?} drawn");
            let marker = label_marker(std::slice::from_ref(&bound), &map, &Pixmaps::new(), 1.0)
                .unwrap()
                .map(|m| marker_bounds(at, &m));
            assert_eq!(
                styles.marker_bounds((&map, &Pixmaps::new()), 0, text, &written, at, 1.0),
                marker,
                "{field:?} marker"
            );
        }
        assert!(diagnostics.is_empty());
    }
}
