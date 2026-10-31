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

use forager_label::{Bounds, Member, Placement, label_poly_at, marker_bounds};
use forager_render::{DisplayItem, Point, Renderer};
use forager_style_ir::{Bound, GeomTransform, Label, Map, Style, Symbol, SymbolKind, SymbolRef};

use crate::classes::{self, scale_factor};
use crate::draw::{Canvas, shade};
use crate::extent::View;
use crate::symbol::{self, marker_size_of};

/// The kind of a label's style that is drawn: by its `GEOMTRANSFORM`, on the box around the text,
/// at the label's point, or at the centre of that box.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Poly,
    Point,
    Center,
}

/// The kind of a label's `style`, or why it is not drawn: a labelpoly with a symbol, a labelpoint
/// or labelcenter whose marker is not drawn, or a style of any other transform or none.
pub(crate) fn kind(style: &Style, symbols: &[Symbol]) -> Result<Kind, &'static str> {
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

/// The styles of the labels cached and the markers of their first `labelpoint` styles: those of a
/// label whose styles vary by feature bound to the feature, by the index of its text, ascending,
/// held apart so that a label without costs nothing; a label whose styles vary by none holds its
/// marker by its definition's index, so that no label holds bounds of its own (Q-9).
#[derive(Default)]
pub(crate) struct LabelStyles {
    /// The `labelpoly` styles of each feature's label cached whose styles vary by feature, bound
    /// to the feature, by the index of its text.
    bound: Vec<(u32, Box<[Style]>)>,
    /// The marker of the first `labelpoint` style of each feature's label cached whose styles vary
    /// by feature, by the index of its text.
    markers: Vec<(u32, LabelMarker)>,
    /// The marker of the first `labelpoint` style of each label whose styles vary by no feature,
    /// by its definition's index, ascending.
    def_markers: Vec<(u32, LabelMarker)>,
}

impl LabelStyles {
    /// Holds the styles of the feature's label of text `text` and definition `def` and their
    /// marker: `styles` bound to the feature where they vary by it, else empty, and the marker
    /// held by the definition.
    pub(crate) fn hold(
        &mut self,
        def: u32,
        text: u32,
        styles: Box<[Style]>,
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
        } else {
            self.bound.push((text, styles));
            if let Some(m) = marker {
                self.markers.push((text, m));
            }
        }
    }

    /// The bounds of the marker of the label of text `text` and definition `def` at its point
    /// `at`, as computeMarkerBounds computes them; `None` where it has none.
    pub(crate) fn marker_bounds(&self, def: u32, text: u32, at: Point) -> Option<Bounds> {
        let by = |table: &[(u32, LabelMarker)], key: u32| {
            table
                .binary_search_by_key(&key, |(k, _)| *k)
                .ok()
                .map(|i| table[i].1)
        };
        by(&self.markers, text)
            .or_else(|| by(&self.def_markers, def))
            .map(|m| marker_bounds(at, m.size, m.anchor_point, m.angle, m.offset))
    }

    /// The styles of the label of text `text` bound to its feature, where they vary by it.
    fn bound(&self, text: u32) -> Option<&[Style]> {
        self.bound
            .binary_search_by_key(&text, |(t, _)| *t)
            .ok()
            .map(|at| &self.bound[at].1[..])
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
        let bound = self.styles.bound(text);
        let point_label = member.label.expect("a drawn label has a box");
        let poly = label_poly_at(member.point, &point_label, placement.anchor);
        for (i, style) in written.iter().enumerate() {
            let Ok(kind) = kind(style, &self.map.symbols) else {
                continue;
            };
            let style = match bound {
                Some(styles) => std::borrow::Cow::Borrowed(&styles[i]),
                None => classes::unbound(style),
            };
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
