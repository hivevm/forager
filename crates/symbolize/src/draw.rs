// The drawing of a feature in a style follows MapServer's msDrawLineSymbol(),
// msDrawShadeSymbol(), and msOutlineRenderingPrepareStyle(), src/maprendering.c, of the release
// pinned in scripts/fetch-msautotest.sh.
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

use std::collections::BTreeMap;
use std::rc::Rc;

use forager_render::{Dashes, DrawOp, LineCap, LineJoin, Point, PremultipliedImage};
use forager_style_ir::c::ms_nint;
use forager_style_ir::{Bound, Coord, Geometry, RenderMode, Rgba, Style, Symbol, SymbolRef};

use crate::bounds::{self, MAX_SIZE};
use crate::extent::View;
use crate::geom_transform;
use crate::hatch::{self, Hatched, Hatching};
use crate::line_marker::{self, MarkerBudget};
use crate::offset::{self, Offset};
use crate::style::{GAMMA, fixed};
use crate::symbol;

/// One drawing of a style on a feature.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pass {
    /// The style as written.
    Plain,
    /// The style as written, only when it has a `COLOR`.
    PlainIfColored,
    /// The outline of a line: the style widened by twice its `OUTLINEWIDTH`, its colours swapped.
    Outline,
    /// The outline, only when the style has an `OUTLINECOLOR`.
    OutlineIfOutlined,
}

/// Whether a layer of the render mode draws the geometry. As in MapServer, a line layer draws
/// polygons as their outlines and a point layer draws a marker at every vertex; a polygon layer
/// draws no lines or points, and a line layer no points.
pub fn can_draw(mode: RenderMode, geometry: &Geometry) -> bool {
    match (mode, geometry) {
        (_, Geometry::Collection(geometries)) => geometries.iter().all(|g| can_draw(mode, g)),
        (RenderMode::Point, _) => true,
        (RenderMode::Polygon, Geometry::Polygon(_) | Geometry::MultiPolygon(_)) => true,
        (RenderMode::Line, g) => !matches!(g, Geometry::Point(_) | Geometry::MultiPoint(_)),
        _ => false,
    }
}

/// The images of a render's pixmap symbols by their resource path, each the decoded image or why
/// it has none, as the engine's store of the render holds them (ADR-0040).
pub type Pixmaps = BTreeMap<String, Result<Rc<PremultipliedImage>, String>>;

/// What a layer draws its features on beside the view: the map's symbols, the images of its
/// pixmap symbols, the image's width and height in pixels, the layer's scale factor and the
/// image's resolution factor (see the engine's `classes::scale_factor`), what is left to the
/// layer of [`line_marker::MAX_MARKER_POINTS`], the hatching of the class drawn, which counts the
/// hatches refused (Q-9), the pixmap symbols left undrawn, and whether the layer is drawn into an
/// image of its own.
pub struct Canvas<'a> {
    symbols: &'a [Symbol],
    pixmaps: Option<&'a Pixmaps>,
    /// The image's width and height in pixels, as the renderer is handed them.
    size: (u32, u32),
    pub(crate) factor: f64,
    pub(crate) markers: MarkerBudget,
    hatching: Hatching,
    /// Each pixmap symbol left undrawn, by its index from 1, and why.
    refused_pixmaps: BTreeMap<usize, String>,
    own_image: bool,
}

impl<'a> Canvas<'a> {
    /// A canvas for one layer at `factor`, its budget of markers whole.
    pub fn new(symbols: &'a [Symbol], width: u32, height: u32, factor: f64) -> Self {
        Self {
            symbols,
            pixmaps: None,
            size: (width, height),
            factor,
            markers: MarkerBudget::new(),
            hatching: Hatching::default(),
            refused_pixmaps: BTreeMap::new(),
            own_image: false,
        }
    }

    /// The canvas, its pixmap symbols drawn with the images of `pixmaps`; without them, every
    /// pixmap is left undrawn.
    pub fn with_pixmaps(self, pixmaps: &'a Pixmaps) -> Self {
        Self {
            pixmaps: Some(pixmaps),
            ..self
        }
    }

    /// Marks the layer as drawn into an image of its own, which msDrawLayer creates with
    /// msImageCreate: that image's `refpt` stays at its top left corner, where agg2CreateImage's
    /// `calloc` leaves it, and a hatch counts its lines from there.
    pub fn in_own_image(&mut self) {
        self.own_image = true;
    }

    /// Sets the class whose styles the next features are drawn in, for the clip of its hatches
    /// (see [`Hatching::for_class`]).
    pub fn class(&mut self, styles: &[Style]) {
        self.hatching = Hatching {
            refused: self.hatching.refused,
            ..Hatching::for_class(styles, self.symbols, self.factor)
        };
    }

    /// The image's width and height in pixels.
    pub(crate) fn image(&self) -> (f64, f64) {
        (f64::from(self.size.0), f64::from(self.size.1))
    }

    /// Whether symbols along the layer's lines were left out, their points past the budget.
    pub fn markers_spent(&self) -> bool {
        self.markers.spent()
    }

    /// How many hatches of the layer were refused (Q-9).
    pub fn hatches_refused(&self) -> usize {
        self.hatching.refused
    }

    /// The pixmap symbols the layer left undrawn, their image refused: each by its index from 1,
    /// with the symbol and why.
    pub fn pixmaps_refused(&self) -> Vec<(usize, &'a Symbol, &str)> {
        self.refused_pixmaps
            .iter()
            .filter_map(|(&index, why)| {
                let symbol = self.symbols.get(index.checked_sub(1)?)?;
                Some((index, symbol, why.as_str()))
            })
            .collect()
    }

    /// The image of the pixmap symbol `symbol`, at `index` from 1, or `None`, its refusal
    /// recorded, where it has none.
    fn pixmap(&mut self, index: usize, symbol: &Symbol) -> Option<Rc<PremultipliedImage>> {
        let image = match (&symbol.image, self.pixmaps) {
            (None, _) => Err("the pixmap symbol names no image".to_owned()),
            (Some(path), Some(pixmaps)) => match pixmaps.get(path) {
                Some(image) => image.clone(),
                None => Err(format!("the image {path} is not loaded for this render")),
            },
            (Some(path), None) => Err(format!("the image {path} is not loaded for this render")),
        };
        image
            .map_err(|why| {
                self.refused_pixmaps.entry(index).or_insert(why);
            })
            .ok()
    }
}

/// The image path of the pixmap symbol that [`draw_ops`] draws as the mark of `style` in a layer
/// of `mode`: at each point of a point layer, and at each point a `GEOMTRANSFORM` places on a
/// line or a polygon. The engine loads it before the layer is drawn, since this crate does no
/// I/O.
pub fn marked_pixmap<'a>(
    mode: RenderMode,
    style: &Style,
    symbols: &'a [Symbol],
) -> Option<&'a str> {
    let marked = mode == RenderMode::Point || style.geom_transform.is_some();
    match symbol::mark(style, symbols) {
        symbol::Mark::Pixmap(_, symbol) if marked => symbol.image.as_deref(),
        _ => None,
    }
}

/// The drawing operations of one pass of a style on a geometry [`can_draw`] accepts, a hatch
/// drawn as the canvas's hatching says, a hatch it refuses to draw counted there (Q-9).
pub fn draw_ops(
    mode: RenderMode,
    geometry: &Geometry,
    style: &Style,
    pass: Pass,
    view: &View,
    canvas: &mut Canvas,
) -> Vec<DrawOp> {
    let pixels = |cs: &[Coord]| cs.iter().map(|&c| view.to_pixel(c)).collect::<Vec<_>>();
    let mut ops = Vec::new();
    match (mode, geometry) {
        // The whole shape, a collection's parts together, as msDrawTransformedShape takes it.
        (RenderMode::Line | RenderMode::Polygon, _) if style.geom_transform.is_some() => {
            transformed(&mut ops, geometry, style, pass, view, canvas);
        }
        (_, Geometry::Collection(geometries)) => {
            for g in geometries {
                ops.extend(draw_ops(mode, g, style, pass, view, canvas));
            }
        }
        (RenderMode::Point, _) => {
            // pointLayerDrawShape: a point outside the map's extent draws nothing. It draws every
            // style at every point, whatever its GEOMTRANSFORM.
            for c in vertices(geometry).into_iter().filter(|&c| view.contains(c)) {
                marker_ops(&mut ops, view.point_to_pixel(c), style, canvas);
            }
        }
        (RenderMode::Polygon, Geometry::Polygon(_) | Geometry::MultiPolygon(_))
            if symbol::is_hatch(style, canvas.symbols) =>
        {
            let rings: Vec<Vec<Coord>> = match geometry {
                Geometry::Polygon(rings) => rings.clone(),
                Geometry::MultiPolygon(polygons) => polygons.concat(),
                _ => Vec::new(),
            };
            let rings = hatch::hatch_rings(&rings, view, canvas.image(), &canvas.hatching);
            let drawn = shade(&mut ops, rings, style, view, canvas);
            canvas.hatching.refused += usize::from(!drawn);
        }
        (RenderMode::Polygon, Geometry::Polygon(rings)) => {
            let rings = rings.iter().map(|r| pixels(r)).collect();
            shade(&mut ops, rings, style, view, canvas);
        }
        (RenderMode::Polygon, Geometry::MultiPolygon(polygons)) => {
            for rings in polygons {
                let rings = rings.iter().map(|r| pixels(r)).collect();
                shade(&mut ops, rings, style, view, canvas);
            }
        }
        (RenderMode::Line, Geometry::LineString(points)) => {
            line_symbol(&mut ops, vec![pixels(points)], style, pass, canvas);
        }
        (RenderMode::Line, Geometry::MultiLineString(lines) | Geometry::Polygon(lines)) => {
            let paths: Vec<_> = lines.iter().map(|l| pixels(l)).collect();
            let paths = match geometry {
                Geometry::Polygon(_) => open_rings(paths),
                _ => paths,
            };
            line_symbol(&mut ops, paths, style, pass, canvas);
        }
        (RenderMode::Line, Geometry::MultiPolygon(polygons)) => {
            let rings = polygons.iter().flatten().map(|r| pixels(r)).collect();
            line_symbol(&mut ops, open_rings(rings), style, pass, canvas);
        }
        _ => {}
    }
    ops
}

/// Every vertex of a geometry, in order.
pub fn vertices(geometry: &Geometry) -> Vec<Coord> {
    match geometry {
        Geometry::Point(c) => vec![*c],
        Geometry::MultiPoint(cs) | Geometry::LineString(cs) => cs.clone(),
        Geometry::MultiLineString(lines) | Geometry::Polygon(lines) => lines.concat(),
        Geometry::MultiPolygon(polygons) => polygons.iter().flatten().flatten().copied().collect(),
        Geometry::Collection(geometries) => geometries.iter().flat_map(vertices).collect(),
    }
}

/// Whether the style has an outline, an `OUTLINEWIDTH` above 0.
pub fn outlined(style: &Style) -> bool {
    outline_width(style) > 0.0
}

/// The style's `OUTLINEWIDTH` in pixels; 0 without one.
fn outline_width(style: &Style) -> f64 {
    fixed(&style.outline_width).unwrap_or(0.0)
}

/// The style's `WIDTH` scaled by the scale factor and bounded by its `MINWIDTH` and `MAXWIDTH`
/// scaled by the resolution factor, both `factor` here, as msDrawLineSymbol() bounds it (see
/// [`bounds::line_width`]). With the width as written.
fn bounded_width(style: &Style, factor: f64) -> (f64, f64) {
    let written = fixed(&style.width).unwrap_or(1.0);
    (bounds::line_width(written * factor, style, factor), written)
}

/// The offset of the style, its numbers multiplied by `factor`; `None` when it moves nothing.
fn style_offset(style: &Style, factor: f64) -> Option<Offset> {
    if let Some(line) = &style.line_offset {
        let distance = line.distance.value()? * factor;
        return Some(if line.both_sides {
            Offset::BothSides(distance)
        } else {
            Offset::Parallel(distance)
        });
    }
    let [x, y] = style.offset.as_ref()?;
    let (x, y) = (x.value().unwrap_or(0.0), y.value().unwrap_or(0.0));
    (x != 0.0 || y != 0.0).then_some(Offset::Shift {
        x: x * factor,
        y: y * factor,
    })
}

fn moved(paths: Vec<Vec<Point>>, offset: Option<Offset>) -> Vec<Vec<Point>> {
    match offset {
        Some(offset) => offset::apply(&paths, offset),
        None => paths,
    }
}

/// The cap and the join agg2RenderLine() strokes a line of `width` with: those given for a line
/// wider than a pixel, and for any other bevel joins and AGG's default cap, butt. (It also bevels
/// the inner joins of such a line, which the display list does not carry; they lie within a
/// pixel of the path.)
fn line_ends(width: f64, cap: LineCap, join: LineJoin) -> (LineCap, LineJoin) {
    if width > 1.0 {
        (cap, join)
    } else {
        (LineCap::Butt, LineJoin::Bevel)
    }
}

/// Rings as MapServer's line_adaptor strokes them: open lines that end on their first point,
/// with caps there and no join.
fn open_rings(mut rings: Vec<Vec<Point>>) -> Vec<Vec<Point>> {
    for ring in &mut rings {
        if let (Some(&first), Some(&last)) = (ring.first(), ring.last())
            && first != last
        {
            ring.push(first);
        }
    }
    rings
}

/// A line drawn in a style, as msDrawLineSymbol() draws one of a simple symbol: in the `COLOR`, or
/// the `OUTLINECOLOR` without one; the width scaled by the canvas's factor and bounded; the offset,
/// the dash pattern, and the initial gap scaled by the factor the scaling and the bounds applied to
/// the width; and the style's cap and join, by default `round` and `none`. An outline pass draws
/// the style widened by twice its outline width with its colours swapped, as
/// msOutlineRenderingPrepareStyle() prepares it. A style with a vector or ellipse symbol and a
/// `GAP` draws the symbol along the line instead, its markers' points taken from the canvas's
/// budget; one whose symbol index is past the last draws nothing.
fn line_symbol(
    ops: &mut Vec<DrawOp>,
    paths: Vec<Vec<Point>>,
    style: &Style,
    pass: Pass,
    canvas: &mut Canvas,
) {
    let prepared;
    let style = match pass {
        Pass::Plain | Pass::PlainIfColored => style,
        Pass::Outline | Pass::OutlineIfOutlined => {
            prepared = outline_style(style);
            &prepared
        }
    };
    let (first, second) = (fixed(&style.color), fixed(&style.outline_color));
    if matches!(pass, Pass::PlainIfColored | Pass::OutlineIfOutlined) && first.is_none() {
        return;
    }
    let Some(color) = first.or(second) else {
        return;
    };
    let (width, written) = bounded_width(style, canvas.factor);
    let factor = if written != 0.0 { width / written } else { 1.0 };
    let initial_gap = style.initial_gap.unwrap_or(-1.0);
    // msDrawLineSymbol's `gap < 0` or `gap > 0`: a gap that is not a number brushes the line.
    let gapped = matches!(style.gap, Some(gap) if gap.abs() > 0.0);
    match symbol::mark(style, canvas.symbols) {
        symbol::Mark::Nothing => return,
        symbol::Mark::Symbol(symbol) if gapped => {
            let paths = moved(paths, style_offset(style, factor));
            let initial_gap = initial_gap * factor;
            ops.extend(line_marker::draw(
                &paths,
                style,
                symbol,
                initial_gap,
                canvas.image(),
                canvas.factor,
                &mut canvas.markers,
            ));
            return;
        }
        symbol::Mark::Symbol(_) | symbol::Mark::Pixmap(..) | symbol::Mark::Square => {}
    }
    let (cap, join) = line_ends(
        width,
        style.line_cap.unwrap_or(LineCap::Round),
        style.line_join.unwrap_or(LineJoin::None),
    );
    ops.push(DrawOp::Stroke {
        paths: moved(paths, style_offset(style, factor)),
        closed: false,
        color,
        width,
        cap,
        join,
        dashes: Dashes {
            pattern: style.pattern.iter().map(|p| p * factor).collect(),
            offset: if initial_gap <= 0.0 {
                0.0
            } else {
                initial_gap * factor
            },
        },
    });
}

/// The style of an outline pass as msOutlineRenderingPrepareStyle() prepares it: its width and
/// its bounds widened by twice its `OUTLINEWIDTH`, its size, MapServer's -1 where it has none,
/// grown by it, and its colours swapped. MapServer divides the width's growth by the layer's
/// scale factor over the image's resolution factor, and divides the size's by the scale factor
/// and multiplies it by the resolution factor; the two factors are one here, so that both grow by
/// the outline width as written, and the drawing scales them after.
fn outline_style(style: &Style) -> Style {
    let outline = outline_width(style);
    Style {
        color: style.outline_color.clone(),
        outline_color: style.color.clone(),
        width: Some(Bound::Value(
            fixed(&style.width).unwrap_or(1.0) + 2.0 * outline,
        )),
        min_width: Some(bounds::min_width(style) + 2.0 * outline),
        max_width: Some(bounds::max_width(style) + 2.0 * outline),
        size: Some(Bound::Value(fixed(&style.size).unwrap_or(-1.0) + outline)),
        ..style.clone()
    }
}

/// A polygon drawn in a style, as msDrawShadeSymbol() draws one at the canvas's factor: without
/// a `COLOR`, its outline as a line in the `OUTLINECOLOR`; with one, moved by the offset scaled,
/// and with a hatch symbol hatched (see [`hatch::hatch`]), with any other filled, and outlined in
/// the `OUTLINECOLOR` at the colour's alpha, with round caps and joins, its width scaled, the
/// factor itself for a width of 0, and bounded by `MAXWIDTH` and `MINWIDTH` unscaled, as
/// MapServer bounds it there. False where a hatch is refused (Q-9).
pub fn shade(
    ops: &mut Vec<DrawOp>,
    rings: Vec<Vec<Point>>,
    style: &Style,
    view: &View,
    canvas: &mut Canvas,
) -> bool {
    let Some(color) = fixed(&style.color) else {
        line_symbol(ops, open_rings(rings), style, Pass::Plain, canvas);
        return true;
    };
    let factor = canvas.factor;
    let rings = moved(rings, style_offset(style, factor));
    if symbol::is_hatch(style, canvas.symbols) {
        return match hatch::hatch(
            rings,
            style,
            color,
            canvas.size,
            hatch::reference(canvas.own_image, view),
            factor,
        ) {
            Hatched::Drawn(hatch) => {
                ops.push(DrawOp::Hatch(hatch));
                true
            }
            Hatched::Nothing => true,
            Hatched::Refused => false,
        };
    }
    ops.push(DrawOp::Fill {
        rings: rings.clone(),
        color,
        gamma: Some(GAMMA),
    });
    if let Some(outline) = fixed(&style.outline_color) {
        let width = fixed(&style.width).unwrap_or(1.0);
        let width = if width == 0.0 { factor } else { width * factor };
        let width = bounds::shade_outline_width(width, style);
        let (cap, join) = line_ends(width, LineCap::Round, LineJoin::Round);
        ops.push(DrawOp::Stroke {
            paths: open_rings(rings),
            closed: false,
            color: Rgba {
                a: color.a,
                ..outline
            },
            width,
            cap,
            join,
            dashes: Dashes::default(),
        });
    }
    true
}

/// What msDrawMarkerSymbol draws of `style` at the pixel `at` at the canvas's factor: its
/// symbol, or the square that stands for a symbol not drawn yet, moved as msAdjustMarkerPos
/// moves it. The square is Forager's, not MapServer's, which draws nothing for symbol 0; it is
/// drawn at a factor of 1. A pixmap whose image the canvas lacks draws nothing, and the canvas
/// records why.
fn marker_ops(ops: &mut Vec<DrawOp>, at: Point, style: &Style, canvas: &mut Canvas) {
    match symbol::mark(style, canvas.symbols) {
        symbol::Mark::Square => point_ops(ops, symbol::shifted(at, style, 1.0), style),
        symbol::Mark::Symbol(s) => symbol::symbol_ops(ops, at, style, s, canvas.factor),
        symbol::Mark::Pixmap(index, s) => {
            if let Some(image) = canvas.pixmap(index, s) {
                pixmap_ops(ops, at, style, s, image, canvas.factor);
            }
        }
        symbol::Mark::Nothing => {}
    }
}

/// A pixmap's `image` at the pixel `at` in `style` at `factor`, as msDrawMarkerSymbol draws it:
/// nothing at a scale of 0; moved by the style's offsets and by the symbol's anchor point, of
/// the size msGetMarkerSize gives the image (msAdjustMarkerPos); then, unturned (an angle of 0 or 360)
/// and at a scale of 1, blended with its top-left pixel at `MS_NINT(x - width / 2.)`,
/// `MS_NINT(y - height / 2.)`, and otherwise resampled into the square about the point, as
/// agg2RenderPixmapSymbol draws it (see [`symbol::pixmap_square`]). A style's colour and opacity
/// do not reach the image (computeSymbolStyle).
fn pixmap_ops(
    ops: &mut Vec<DrawOp>,
    at: Point,
    style: &Style,
    symbol: &Symbol,
    image: Rc<PremultipliedImage>,
    factor: f64,
) {
    let (scale, rotation) = symbol::pixmap_style(style, image.height(), factor);
    if scale == 0.0 {
        return;
    }
    let at = symbol::anchored(symbol::shifted(at, style, factor), symbol, rotation, || {
        symbol::pixmap_marker_size(
            style,
            fixed(&style.size),
            image.width(),
            image.height(),
            factor,
        )
    });
    if (rotation != 0.0 && rotation != std::f64::consts::PI * 2.0) || scale != 1.0 {
        let (half, inverse) =
            symbol::pixmap_square(image.width(), image.height(), at, scale, rotation);
        ops.push(DrawOp::ResampledPixmap {
            image,
            at,
            half,
            inverse,
        });
        return;
    }
    ops.push(DrawOp::Pixmap {
        left: ms_nint(at.x - f64::from(image.width()) / 2.0),
        top: ms_nint(at.y - f64::from(image.height()) / 2.0),
        image,
    });
}

/// A style with a `GEOMTRANSFORM` on a shape of a line or a polygon layer, as
/// msDrawTransformedShape draws it: its symbol at each point the transform places, turned there
/// where the style's angle is `AUTO` and the transform gives one; nothing for a style without a
/// symbol or with symbol 0, as msDrawMarkerSymbol draws none. The outline pass of a line layer's
/// first style draws the style its outline prepares, as msDrawVectorLayer prepares the style
/// before msDrawShape draws the transform. The symbols' points are taken from the canvas's
/// budget of markers (Q-9): where they would exceed what is left, none of the shape's is drawn,
/// nor any after them.
fn transformed(
    ops: &mut Vec<DrawOp>,
    geometry: &Geometry,
    style: &Style,
    pass: Pass,
    view: &View,
    canvas: &mut Canvas,
) {
    let Some(transform) = style.geom_transform else {
        return;
    };
    if matches!(style.symbol, None | Some(Bound::Value(SymbolRef::Index(0)))) {
        return;
    }
    // A pass's condition, a COLOR or an OUTLINECOLOR, does not apply here, and the engine's
    // `Plan::passes` never hands this function `PlainIfColored` or `OutlineIfOutlined`: a later
    // transformed style of a cached class takes `Plain`, the first `Plain` or `Outline`, as
    // lineLayerDrawShape (src/mapdraw.c:2140-2149) hands every style with a GEOMTRANSFORM in its
    // scale range to msDrawTransformedShape without testing a colour, whatever
    // MS_DRAWMODE_SINGLESTYLE asks, once: in a cached layer in its first pass, where only the
    // first style is prepared for its outline (1365-1378), and in an uncached one as written.
    // The cache's later passes, which test MS_VALID_COLOR, skip such a style (1443-1444). Which
    // colours a marker draws, msDrawMarkerSymbolInternal decides (`marker_ops`).
    let prepared;
    let style = match pass {
        Pass::Plain | Pass::PlainIfColored => style,
        Pass::Outline | Pass::OutlineIfOutlined => {
            prepared = outline_style(style);
            &prepared
        }
    };
    let placed = geom_transform::placed(transform, geometry, view, canvas.image());
    if placed.is_empty() {
        return;
    }
    // What one symbol costs, the same at every point and angle.
    let mut probe = Vec::new();
    marker_ops(&mut probe, Point { x: 0.0, y: 0.0 }, style, canvas);
    let cost: usize = probe.iter().map(line_marker::points).sum();
    if !canvas.markers.take(placed.len().saturating_mul(cost)) {
        return;
    }
    let mut turned = style.auto_angle.then(|| style.clone());
    for (at, angle) in placed {
        match (&mut turned, angle) {
            (Some(turned), Some(angle)) => {
                turned.angle = Some(Bound::Value(angle));
                marker_ops(ops, at, turned, canvas);
            }
            _ => marker_ops(ops, at, style, canvas),
        }
    }
}

/// A point as a square of the style's size, centred on it, filled in the `COLOR` and outlined
/// in the `OUTLINECOLOR` at the style's width, bounded as computeSymbolStyle() bounds the outline
/// of the symbol the square stands for, at a factor of 1 (see [`bounds::symbol_outline_width`]).
fn point_ops(ops: &mut Vec<DrawOp>, at: Point, style: &Style) {
    let half = fixed(&style.size).unwrap_or(1.0).clamp(0.0, MAX_SIZE) / 2.0;
    let corner = |dx: f64, dy: f64| Point {
        x: at.x + dx,
        y: at.y + dy,
    };
    let square = vec![
        corner(-half, -half),
        corner(half, -half),
        corner(half, half),
        corner(-half, half),
    ];
    if let Some(color) = fixed(&style.color) {
        ops.push(DrawOp::Fill {
            rings: vec![square.clone()],
            color,
            gamma: None,
        });
    }
    if let Some(color) = fixed(&style.outline_color) {
        let width = bounds::symbol_outline_width(fixed(&style.width).unwrap_or(1.0), style, 1.0);
        let (cap, join) = line_ends(width, LineCap::Round, LineJoin::None);
        ops.push(DrawOp::Stroke {
            paths: vec![square],
            closed: true,
            color,
            width,
            cap,
            join,
            dashes: Dashes::default(),
        });
    }
}

#[cfg(test)]
mod tests {
    use forager_style_ir::{Extent, GeomTransform, LineOffset, SymbolKind};

    use super::*;

    const BLUE: Rgba = Rgba::opaque(30, 60, 200);
    const BLACK: Rgba = Rgba::opaque(0, 0, 0);

    fn p(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    /// A canvas without symbols, of 100 × 100.
    fn plain() -> Canvas<'static> {
        Canvas::new(&[], 100, 100, 1.0)
    }

    fn line() -> Vec<Vec<Point>> {
        vec![vec![p(0.0, 0.0), p(10.0, 0.0)]]
    }

    fn colored(color: Option<Rgba>, outline: Option<Rgba>) -> Style {
        Style {
            color: color.map(Bound::Value),
            outline_color: outline.map(Bound::Value),
            ..Style::default()
        }
    }

    /// The strokes of one pass of a style on [`line`]: color, width, dashes, and paths.
    fn strokes(style: &Style, pass: Pass) -> Vec<(Rgba, f64, Dashes, Vec<Vec<Point>>)> {
        let mut ops = Vec::new();
        line_symbol(&mut ops, line(), style, pass, &mut plain());
        ops.into_iter()
            .map(|op| match op {
                DrawOp::Stroke {
                    color,
                    width,
                    dashes,
                    paths,
                    ..
                } => (color, width, dashes, paths),
                DrawOp::Fill { .. }
                | DrawOp::Hatch(_)
                | DrawOp::Glyphs { .. }
                | DrawOp::Pixmap { .. }
                | DrawOp::ResampledPixmap { .. } => {
                    panic!("a line is not filled")
                }
            })
            .collect()
    }

    #[test]
    fn the_bound_on_the_width_scales_the_pattern_the_initial_gap_and_the_offset() {
        // WIDTH 10 bounded by MAXWIDTH 5: msDrawLineSymbol's finalscalefactor is 0.5.
        let style = Style {
            width: Some(Bound::Value(10.0)),
            max_width: Some(5.0),
            pattern: vec![20.0, 8.0],
            initial_gap: Some(6.0),
            line_offset: Some(LineOffset {
                distance: Bound::Value(4.0),
                both_sides: false,
            }),
            ..colored(Some(BLUE), None)
        };
        let drawn = strokes(&style, Pass::Plain);
        let (_, width, dashes, paths) = &drawn[0];
        assert_eq!(*width, 5.0);
        assert_eq!(dashes.pattern, vec![10.0, 4.0]);
        assert_eq!(dashes.offset, 3.0);
        assert_eq!(paths[0], vec![p(0.0, 2.0), p(10.0, 2.0)]);
        // MINWIDTH raises the width and scales alike.
        let raised = Style {
            width: Some(Bound::Value(2.0)),
            min_width: Some(4.0),
            pattern: vec![3.0, 1.0],
            ..colored(Some(BLUE), None)
        };
        assert_eq!(strokes(&raised, Pass::Plain)[0].2.pattern, vec![6.0, 2.0]);
    }

    #[test]
    fn an_outline_pass_of_symbols_grows_their_width_and_size_and_swaps_their_colors() {
        // msOutlineRenderingPrepareStyle at a scale factor of 1: the width and its bounds by
        // twice the outline width, the size by it, MapServer's -1 where none is given.
        let style = Style {
            width: Some(Bound::Value(2.0)),
            size: Some(Bound::Value(6.0)),
            outline_width: Some(Bound::Value(1.5)),
            ..colored(Some(BLUE), Some(BLACK))
        };
        let prepared = outline_style(&style);
        assert_eq!(fixed(&prepared.width), Some(5.0));
        assert_eq!(
            (prepared.min_width, prepared.max_width),
            (Some(3.0), Some(35.0))
        );
        assert_eq!(fixed(&prepared.size), Some(7.5));
        assert_eq!(
            (fixed(&prepared.color), fixed(&prepared.outline_color)),
            (Some(BLACK), Some(BLUE))
        );
        let without_size = Style {
            size: None,
            ..style
        };
        assert_eq!(fixed(&outline_style(&without_size).size), Some(0.5));
    }

    #[test]
    fn an_outline_pass_grows_the_style_s_own_width_bounds() {
        // msOutlineRenderingPrepareStyle grows the style's MINWIDTH and MAXWIDTH where it has
        // them, and MapServer's defaults, 0 and 32, where it has none.
        let style = Style {
            min_width: Some(4.0),
            max_width: Some(10.0),
            outline_width: Some(Bound::Value(1.5)),
            ..colored(Some(BLUE), Some(BLACK))
        };
        let prepared = outline_style(&style);
        assert_eq!(
            (prepared.min_width, prepared.max_width),
            (Some(7.0), Some(13.0))
        );
        let defaults = outline_style(&Style {
            min_width: None,
            max_width: None,
            ..style
        });
        assert_eq!(
            (defaults.min_width, defaults.max_width),
            (Some(3.0), Some(35.0))
        );
    }

    /// The half side of the stand-in square [`point_ops`] draws for `style`.
    fn half_side(style: &Style) -> f64 {
        let mut ops = Vec::new();
        point_ops(&mut ops, p(0.0, 0.0), style);
        match &ops[0] {
            DrawOp::Fill { rings, .. } => rings[0][1].x,
            _ => panic!("the square is filled first"),
        }
    }

    #[test]
    fn the_stand_in_square_bounds_its_size_by_mapserver_s_defaults_alone() {
        // Forager's stand-in, not MapServer's: its size is cut to between 0 and MS_MAXSYMBOLSIZE,
        // 500, and the style's MINSIZE and MAXSIZE do not count.
        let sized = |size: f64| Style {
            size: Some(Bound::Value(size)),
            ..colored(Some(BLUE), None)
        };
        assert_eq!(half_side(&sized(6.0)), 3.0);
        assert_eq!(half_side(&sized(600.0)), 250.0);
        assert_eq!(half_side(&sized(-4.0)), 0.0);
        let bounded = Style {
            min_size: Some(10.0),
            max_size: Some(2.0),
            ..sized(6.0)
        };
        assert_eq!(half_side(&bounded), 3.0);
    }

    /// The points of drawing operations.
    fn points(ops: &[DrawOp]) -> usize {
        ops.iter()
            .map(|op| match op {
                DrawOp::Stroke { paths, .. } => paths.iter().map(Vec::len).sum(),
                DrawOp::Fill { rings, .. } => rings.iter().map(Vec::len).sum(),
                DrawOp::Hatch(_)
                | DrawOp::Glyphs { .. }
                | DrawOp::Pixmap { .. }
                | DrawOp::ResampledPixmap { .. } => 0,
            })
            .sum()
    }

    #[test]
    fn the_symbols_along_the_lines_of_a_layer_share_one_budget() {
        // Verifies: Q-9
        // A filled triangle of 3 points every 0.01 pixels from 0.005 along lines 100 pixels long:
        // 10 000 markers, 30 000 points, each line far under the limit. A feature of 20 such lines
        // takes 600 000 points and one of 10 another 300 000; a third of 20 would pass the
        // million and draws nothing, nor does any feature after it.
        let view = View::for_image(
            Extent {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 100.0,
                max_y: 100.0,
            },
            101,
            101,
        )
        .unwrap();
        let triangle = Symbol {
            name: None,
            kind: SymbolKind::Vector,
            paths: vec![vec![[0.0, 0.0], [2.0, 0.0], [1.0, 2.0]]],
            extent: Some([2.0, 2.0]),
            filled: true,
            anchor_point: None,
            character: None,
            font: None,
            image: None,
            transparent: None,
            span: None,
        };
        let symbols = [triangle];
        let style = Style {
            symbol: Some(Bound::Value(SymbolRef::Index(1))),
            gap: Some(-0.01),
            ..colored(Some(BLUE), None)
        };
        let lines = |count: i32| {
            Geometry::MultiLineString(
                (0..count)
                    .map(|i| {
                        let y = f64::from(i) * 5.0;
                        vec![Coord { x: 0.0, y }, Coord { x: 100.0, y }]
                    })
                    .collect(),
            )
        };
        let mut canvas = Canvas::new(&symbols, 101, 101, 1.0);
        let mut draw = |geometry: &Geometry| {
            let ops = draw_ops(
                RenderMode::Line,
                geometry,
                &style,
                Pass::Plain,
                &view,
                &mut canvas,
            );
            (points(&ops), canvas.markers_spent())
        };
        assert_eq!(draw(&lines(20)), (600_000, false));
        assert_eq!(draw(&lines(10)), (300_000, false));
        assert_eq!(draw(&lines(20)), (0, true));
        assert_eq!(
            draw(&lines(1)),
            (0, true),
            "nothing once the budget is spent"
        );
    }

    #[test]
    fn a_symbol_index_past_the_last_draws_no_line() {
        // msDrawLineSymbol returns at once for a symbol it does not have.
        let style = Style {
            symbol: Some(Bound::Value(SymbolRef::Index(9))),
            ..colored(Some(BLUE), None)
        };
        assert!(strokes(&style, Pass::Plain).is_empty());
    }

    #[test]
    fn a_line_without_a_color_is_drawn_in_its_outline_color() {
        let drawn = strokes(&colored(None, Some(BLACK)), Pass::Plain);
        assert_eq!(drawn[0].0, BLACK);
        assert!(strokes(&colored(None, None), Pass::Plain).is_empty());
    }

    #[test]
    fn an_outline_pass_widens_the_line_and_swaps_its_colors() {
        let style = Style {
            width: Some(Bound::Value(6.0)),
            outline_width: Some(Bound::Value(2.0)),
            ..colored(Some(BLUE), Some(BLACK))
        };
        let drawn = strokes(&style, Pass::Outline);
        assert_eq!((drawn[0].0, drawn[0].1), (BLACK, 10.0));
        // The first style's outline falls back to its colour, as the swapped style does.
        let uncolored_outline = Style {
            outline_width: Some(Bound::Value(2.0)),
            ..colored(Some(BLUE), None)
        };
        assert_eq!(strokes(&uncolored_outline, Pass::Outline)[0].0, BLUE);
        // A later style draws its outline only with an OUTLINECOLOR, itself only with a COLOR.
        assert!(strokes(&uncolored_outline, Pass::OutlineIfOutlined).is_empty());
        assert!(strokes(&colored(None, Some(BLACK)), Pass::PlainIfColored).is_empty());
    }

    #[test]
    fn a_line_no_wider_than_a_pixel_has_butt_caps_and_bevel_joins() {
        // agg2RenderLine applies the style's cap and join only to a line wider than a pixel.
        let style = |width| Style {
            width: Some(Bound::Value(width)),
            line_cap: Some(LineCap::Square),
            line_join: Some(LineJoin::Miter),
            ..colored(Some(BLUE), None)
        };
        let ends = |width| {
            let mut ops = Vec::new();
            line_symbol(&mut ops, line(), &style(width), Pass::Plain, &mut plain());
            match &ops[0] {
                DrawOp::Stroke { cap, join, .. } => (*cap, *join),
                DrawOp::Fill { .. }
                | DrawOp::Hatch(_)
                | DrawOp::Glyphs { .. }
                | DrawOp::Pixmap { .. }
                | DrawOp::ResampledPixmap { .. } => {
                    panic!("a line is not filled")
                }
            }
        };
        assert_eq!(ends(1.0), (LineCap::Butt, LineJoin::Bevel));
        assert_eq!(ends(1.5), (LineCap::Square, LineJoin::Miter));
    }

    #[test]
    fn a_filled_polygon_is_outlined_at_its_fill_s_alpha() {
        let fill = Rgba {
            a: 100,
            ..Rgba::opaque(200, 80, 80)
        };
        let mut ops = Vec::new();
        let style = Style {
            width: Some(Bound::Value(2.0)),
            ..colored(Some(fill), Some(BLACK))
        };
        let view = View::for_image(
            forager_style_ir::Extent {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 10.0,
                max_y: 10.0,
            },
            11,
            11,
        )
        .unwrap();
        assert!(shade(&mut ops, line(), &style, &view, &mut plain()));
        // The polygon shape is one fill item, through MapServer's default gamma, the 0.75 of
        // agg2CreateImage's msGetOutputFormatOption(format, "GAMMA", "0.75").
        assert!(matches!(
            ops[0],
            DrawOp::Fill {
                color,
                gamma: Some(0.75),
                ..
            } if color == fill
        ));
        match &ops[1] {
            DrawOp::Stroke {
                color, cap, join, ..
            } => {
                assert_eq!(*color, Rgba { a: 100, ..BLACK });
                assert_eq!((*cap, *join), (LineCap::Round, LineJoin::Round));
            }
            DrawOp::Fill { .. }
            | DrawOp::Hatch(_)
            | DrawOp::Glyphs { .. }
            | DrawOp::Pixmap { .. }
            | DrawOp::ResampledPixmap { .. } => {
                panic!("the outline is a stroke")
            }
        }
    }

    /// A view of 0..100 on 101 × 101 pixels: a map unit is a pixel, y downwards from 100.
    fn unit_view() -> View {
        View::for_image(
            Extent {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 100.0,
                max_y: 100.0,
            },
            101,
            101,
        )
        .unwrap()
    }

    /// A filled circle, symbol 1.
    fn circle() -> Symbol {
        Symbol {
            name: None,
            kind: SymbolKind::Ellipse,
            paths: vec![vec![[1.0, 1.0]]],
            extent: Some([1.0, 1.0]),
            filled: true,
            anchor_point: None,
            character: None,
            font: None,
            image: None,
            transparent: None,
            span: None,
        }
    }

    fn coords(points: &[(f64, f64)]) -> Vec<Coord> {
        points.iter().map(|&(x, y)| Coord { x, y }).collect()
    }

    /// An L from pixel (10, 20) right to (90, 20), then down to (90, 90).
    fn ell() -> Geometry {
        Geometry::LineString(coords(&[(10.0, 80.0), (90.0, 80.0), (90.0, 10.0)]))
    }

    fn transformed_style(transform: GeomTransform, symbol: Option<u32>) -> Style {
        Style {
            symbol: symbol.map(|i| Bound::Value(SymbolRef::Index(i))),
            size: Some(Bound::Value(6.0)),
            geom_transform: Some(transform),
            ..colored(Some(BLUE), None)
        }
    }

    fn transformed_ops(geometry: &Geometry, style: &Style, canvas: &mut Canvas) -> Vec<DrawOp> {
        draw_ops(
            RenderMode::Line,
            geometry,
            style,
            Pass::Plain,
            &unit_view(),
            canvas,
        )
    }

    #[test]
    fn a_transformed_style_without_a_symbol_or_with_symbol_0_draws_nothing() {
        // msDrawMarkerSymbolInternal returns at a symbol of 0 or below, where a point layer draws
        // its stand-in square.
        let symbols = [circle()];
        for symbol in [None, Some(0)] {
            let style = transformed_style(GeomTransform::Start, symbol);
            let mut canvas = Canvas::new(&symbols, 101, 101, 1.0);
            assert!(transformed_ops(&ell(), &style, &mut canvas).is_empty());
        }
        let style = transformed_style(GeomTransform::Start, Some(1));
        let mut canvas = Canvas::new(&symbols, 101, 101, 1.0);
        assert!(!transformed_ops(&ell(), &style, &mut canvas).is_empty());
    }

    #[test]
    fn without_angle_auto_a_transformed_symbol_keeps_its_angle() {
        // The arrow at the corner of the L: at its own ANGLE 30 without AUTO, turned by -45°, as
        // calcMidAngle gives it, with.
        let arrow = Symbol {
            kind: SymbolKind::Vector,
            paths: vec![vec![[0.0, 2.0], [2.0, 0.0], [4.0, 2.0]]],
            extent: Some([4.0, 2.0]),
            filled: false,
            ..circle()
        };
        let symbols = [arrow.clone()];
        let style = Style {
            angle: Some(Bound::Value(30.0)),
            ..transformed_style(GeomTransform::Vertices, Some(1))
        };
        let at_corner = |style: &Style| {
            let mut ops = Vec::new();
            symbol::symbol_ops(&mut ops, p(90.0, 20.0), style, &arrow, 1.0);
            ops
        };
        let mut canvas = Canvas::new(&symbols, 101, 101, 1.0);
        assert_eq!(
            transformed_ops(&ell(), &style, &mut canvas),
            at_corner(&style)
        );
        let auto = Style {
            auto_angle: true,
            ..style.clone()
        };
        let turned = Style {
            angle: Some(Bound::Value(-45.0)),
            ..style
        };
        let drawn = transformed_ops(&ell(), &auto, &mut canvas);
        let expected = at_corner(&turned);
        let (DrawOp::Stroke { paths: a, .. }, DrawOp::Stroke { paths: b, .. }) =
            (&drawn[0], &expected[0])
        else {
            panic!("{drawn:?}");
        };
        for (a, b) in a[0].iter().zip(&b[0]) {
            assert!(
                (a.x - b.x).abs() < 1e-6 && (a.y - b.y).abs() < 1e-6,
                "{a:?} {b:?}"
            );
        }
    }

    #[test]
    fn the_symbols_a_transform_places_share_the_layer_s_budget() {
        // Verifies: Q-9
        // Four inner vertices take four circles; a budget one circle short of them draws none
        // of the shape's, nor anything after it.
        let symbols = [circle()];
        let style = transformed_style(GeomTransform::Vertices, Some(1));
        let zigzag = Geometry::LineString(coords(&[
            (10.0, 50.0),
            (20.0, 60.0),
            (30.0, 50.0),
            (40.0, 60.0),
            (50.0, 50.0),
            (60.0, 60.0),
        ]));
        let mut canvas = Canvas::new(&symbols, 101, 101, 1.0);
        let drawn = transformed_ops(&zigzag, &style, &mut canvas);
        assert_eq!(drawn.len(), 4);
        let cost = points(&drawn);
        let mut canvas = Canvas::new(&symbols, 101, 101, 1.0);
        canvas.markers = MarkerBudget::of(cost - 1);
        assert!(transformed_ops(&zigzag, &style, &mut canvas).is_empty());
        assert!(canvas.markers_spent());
        assert!(transformed_ops(&ell(), &style, &mut canvas).is_empty());
        let mut canvas = Canvas::new(&symbols, 101, 101, 1.0);
        canvas.markers = MarkerBudget::of(cost);
        assert_eq!(transformed_ops(&zigzag, &style, &mut canvas).len(), 4);
        assert!(!canvas.markers_spent());
    }

    #[test]
    fn a_centroid_of_a_collection_is_that_of_all_its_parts() {
        // Two lines of a collection, (10, 50)–(30, 50) and (70, 50)–(90, 50): one centroid, at
        // (50, 50), not one for each.
        let symbols = [circle()];
        let style = transformed_style(GeomTransform::Centroid, Some(1));
        let collection = Geometry::Collection(vec![
            Geometry::LineString(coords(&[(10.0, 50.0), (30.0, 50.0)])),
            Geometry::LineString(coords(&[(70.0, 50.0), (90.0, 50.0)])),
        ]);
        let mut canvas = Canvas::new(&symbols, 101, 101, 1.0);
        let drawn = transformed_ops(&collection, &style, &mut canvas);
        let mut expected = Vec::new();
        symbol::symbol_ops(&mut expected, p(50.0, 50.0), &style, &symbols[0], 1.0);
        assert_eq!(drawn, expected);
    }

    fn hatch_symbol() -> Symbol {
        Symbol {
            name: None,
            kind: forager_style_ir::SymbolKind::Hatch,
            paths: Vec::new(),
            extent: None,
            filled: false,
            anchor_point: None,
            character: None,
            font: None,
            image: None,
            transparent: None,
            span: None,
        }
    }

    /// The hatch ops of `geometry` in a hatch style of SIZE 10 and WIDTH 1 on a view of the map
    /// from 0 to 100 at a pixel a unit, y downwards from 100.
    fn hatches(geometry: &Geometry) -> Vec<forager_render::Hatch> {
        let view = View::for_image(
            forager_style_ir::Extent {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 100.0,
                max_y: 100.0,
            },
            101,
            101,
        )
        .unwrap();
        let style = Style {
            symbol: Some(Bound::Value(forager_style_ir::SymbolRef::Index(1))),
            size: Some(Bound::Value(10.0)),
            ..colored(Some(BLUE), None)
        };
        let symbols = [hatch_symbol()];
        let mut canvas = Canvas::new(&symbols, 101, 101, 1.0);
        canvas.class(std::slice::from_ref(&style));
        let ops = draw_ops(
            RenderMode::Polygon,
            geometry,
            &style,
            Pass::Plain,
            &view,
            &mut canvas,
        );
        ops.into_iter()
            .map(|op| match op {
                DrawOp::Hatch(hatch) => hatch,
                other => panic!("{other:?}"),
            })
            .collect()
    }

    fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Coord> {
        [(x0, y0), (x1, y0), (x1, y1), (x0, y1), (x0, y0)]
            .map(|(x, y)| Coord { x, y })
            .to_vec()
    }

    #[test]
    fn a_hatch_s_box_is_that_of_the_polygon_clipped_beyond_the_extent() {
        // The class's clip buffer is max(1, max(10, 1)) + 1 + 2 = 13 pixels: the polygon from
        // x = -1000 is clipped at -13, and the box of the hatch, grown by 0.7072, starts there.
        let reaching = Geometry::Polygon(vec![square(-1000.0, 10.0, 50.0, 60.0)]);
        let hatch = &hatches(&reaching)[0];
        assert!(
            (hatch.lines[0][0].x - (-13.7072)).abs() < 1e-9,
            "{:?}",
            hatch.lines[0]
        );
        // Inside the extent the polygon is not clipped.
        let inside = Geometry::Polygon(vec![square(5.0, 10.0, 50.0, 60.0)]);
        let hatch = &hatches(&inside)[0];
        assert!(
            (hatch.lines[0][0].x - 4.2928).abs() < 1e-9,
            "{:?}",
            hatch.lines[0]
        );
    }

    #[test]
    fn a_hatch_is_bounded_on_the_image_s_own_size_not_one_rebuilt_from_the_extent() {
        // An image of 1001 × 1001 pixels whose view spans 0..99 at a pixel a unit: the extent
        // over the cellsize would give 100 × 100. A hatch a tenth of a pixel apart, a pixel
        // wide, over a polygon wider than the image lays some 10 060 lines of 1 006 pixels over
        // the image itself, past the bound on their length (Q-9), which the renderer measures
        // on 1001 × 1001; over 100 × 100 they would be some 110 000 pixels in all.
        let view = View {
            extent: Extent {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 99.0,
                max_y: 99.0,
            },
            cellsize: 1.0,
        };
        let style = Style {
            symbol: Some(Bound::Value(SymbolRef::Index(1))),
            size: Some(Bound::Value(0.1)),
            ..colored(Some(BLUE), None)
        };
        let symbols = [hatch_symbol()];
        let ring = |side: f64| {
            vec![vec![
                p(-10.0, -10.0),
                p(side, -10.0),
                p(side, side),
                p(-10.0, side),
            ]]
        };
        let shaded = |width: u32, ring| {
            let mut canvas = Canvas::new(&symbols, width, width, 1.0);
            let mut ops = Vec::new();
            shade(&mut ops, ring, &style, &view, &mut canvas)
        };
        assert!(!shaded(1001, ring(1100.0)), "refused on 1001 × 1001");
        // For contrast, not deciding: the hatch over a polygon about an image that is truly
        // 100 × 100 is drawn. The view is built by hand, since `View::for_image` always gives
        // an extent that rebuilds the image's own size, and the criterion is the case where the
        // two disagree.
        assert!(shaded(100, ring(110.0)), "drawn on 100 × 100");
    }

    #[test]
    fn a_multipolygon_is_hatched_as_one_shape() {
        // msDrawShadeSymbol hatches the whole shape: one box over both squares.
        let two = Geometry::MultiPolygon(vec![
            vec![square(5.0, 10.0, 20.0, 20.0)],
            vec![square(60.0, 70.0, 90.0, 90.0)],
        ]);
        let drawn = hatches(&two);
        assert_eq!(drawn.len(), 1);
        assert_eq!(drawn[0].gamma, Some(0.75));
        assert_eq!(drawn[0].rings.len(), 2);
        let first = drawn[0].lines[0];
        assert!((first[0].x - 4.2928).abs() < 1e-9 && (first[1].x - 91.2928).abs() < 1e-9);
    }

    /// The operations of `style` on [`line`] at `factor`.
    fn strokes_at(style: &Style, factor: f64) -> Vec<DrawOp> {
        let mut ops = Vec::new();
        let mut canvas = Canvas::new(&[], 100, 100, factor);
        line_symbol(&mut ops, line(), style, Pass::Plain, &mut canvas);
        ops
    }

    fn stroke(op: &DrawOp) -> (f64, &Dashes, &[Vec<Point>]) {
        match op {
            DrawOp::Stroke {
                width,
                dashes,
                paths,
                ..
            } => (*width, dashes, paths),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn at_a_factor_a_line_is_scaled_and_bounded_by_the_resolution_factor() {
        // msDrawLineSymbol, maprendering.c:622-653, at a scale and a resolution factor of 2:
        // the width times the scale factor, MAXWIDTH and MINWIDTH times the resolution factor,
        // and the offset, the pattern, and the initial gap by the final width over the WIDTH.
        let bounded = Style {
            width: Some(Bound::Value(10.0)),
            max_width: Some(5.0),
            pattern: vec![20.0, 8.0],
            initial_gap: Some(6.0),
            line_offset: Some(LineOffset {
                distance: Bound::Value(4.0),
                both_sides: false,
            }),
            ..colored(Some(BLUE), None)
        };
        let ops = strokes_at(&bounded, 2.0);
        let (width, dashes, paths) = stroke(&ops[0]);
        assert_eq!(width, 10.0, "MAXWIDTH 5 is 10");
        assert_eq!(dashes.pattern, vec![20.0, 8.0]);
        assert_eq!(dashes.offset, 6.0);
        assert_eq!(paths[0], vec![p(0.0, 4.0), p(10.0, 4.0)]);
        let plain = Style {
            width: Some(Bound::Value(3.0)),
            pattern: vec![3.0, 1.0],
            ..colored(Some(BLUE), None)
        };
        let ops = strokes_at(&plain, 2.0);
        assert_eq!(stroke(&ops[0]).0, 6.0);
        assert_eq!(stroke(&ops[0]).1.pattern, vec![6.0, 2.0]);
        let wide = Style {
            width: Some(Bound::Value(40.0)),
            ..colored(Some(BLUE), None)
        };
        assert_eq!(
            stroke(&strokes_at(&wide, 2.0)[0]).0,
            64.0,
            "the default MAXWIDTH 32 is 64"
        );
        let raised = Style {
            width: Some(Bound::Value(1.0)),
            min_width: Some(4.0),
            ..colored(Some(BLUE), None)
        };
        assert_eq!(stroke(&strokes_at(&raised, 2.0)[0]).0, 8.0);
    }

    #[test]
    fn at_a_factor_symbols_along_a_line_are_those_of_the_scaled_style() {
        // msDrawLineSymbol hands computeSymbolStyle the scale factor, and the initial gap is
        // scaled by the final width over the WIDTH: at 2, SIZE 6, GAP 20, and INITIALGAP 5 draw
        // what SIZE 12, GAP 40, and INITIALGAP 10 draw at 1.
        let symbols = [circle()];
        let gapped = |size: f64, gap: f64, initial_gap: f64| Style {
            symbol: Some(Bound::Value(SymbolRef::Index(1))),
            size: Some(Bound::Value(size)),
            gap: Some(gap),
            initial_gap: Some(initial_gap),
            ..colored(Some(BLUE), None)
        };
        let along = |style: &Style, factor: f64| {
            let mut ops = Vec::new();
            let mut canvas = Canvas::new(&symbols, 101, 101, factor);
            let long = vec![vec![p(0.0, 50.0), p(100.0, 50.0)]];
            line_symbol(&mut ops, long, style, Pass::Plain, &mut canvas);
            ops
        };
        let scaled = along(&gapped(6.0, 20.0, 5.0), 2.0);
        assert_eq!(scaled.len(), 3);
        assert_eq!(scaled, along(&gapped(12.0, 40.0, 10.0), 1.0));
    }

    #[test]
    fn at_a_factor_a_polygon_moves_by_its_offset_scaled_and_its_outline_bounds_stay() {
        // msDrawShadeSymbol, maprendering.c:771-799, at a scale factor of 2: the OFFSET
        // times the scale factor; the outline's WIDTH times it, the factor itself for a WIDTH of
        // 0, bounded by MAXWIDTH and MINWIDTH as written.
        let outlined = |width: f64, max_width: Option<f64>| {
            let style = Style {
                width: Some(Bound::Value(width)),
                max_width,
                offset: Some([Bound::Value(3.0), Bound::Value(4.0)]),
                ..colored(Some(BLUE), Some(BLACK))
            };
            let mut ops = Vec::new();
            let mut canvas = Canvas::new(&[], 101, 101, 2.0);
            let ring = vec![vec![p(10.0, 10.0), p(20.0, 10.0), p(20.0, 20.0)]];
            assert!(shade(&mut ops, ring, &style, &unit_view(), &mut canvas));
            ops
        };
        let ops = outlined(3.0, None);
        let DrawOp::Fill { rings, .. } = &ops[0] else {
            panic!("{ops:?}");
        };
        assert_eq!(rings[0], vec![p(16.0, 18.0), p(26.0, 18.0), p(26.0, 28.0)]);
        assert_eq!(stroke(&ops[1]).0, 6.0);
        assert_eq!(
            stroke(&outlined(0.0, None)[1]).0,
            2.0,
            "WIDTH 0 is the factor"
        );
        assert_eq!(
            stroke(&outlined(3.0, Some(5.0))[1]).0,
            5.0,
            "MAXWIDTH 5 stays 5"
        );
    }

    #[test]
    fn at_a_factor_a_point_draws_its_symbol_scaled_and_its_stand_in_square_as_written() {
        // pointLayerDrawShape and msDrawTransformedShape hand msDrawMarkerSymbol the style's
        // scale factor; the square of SYMBOL 0 is Forager's stand-in, drawn at 1. Symbol 1 is a
        // vector square of side 2: at the factor 2, computeSymbolStyle's scale is SIZE 6 × 2 over
        // msSymbolGetDefaultSize's sizey 2, 6 (maprendering.c:62-65, mapsymbol.c:87), and its
        // outline the default WIDTH 1 × 2 (maprendering.c:69); msAdjustMarkerPos moves the point
        // by OFFSET (2, 1) × 2 (maprendering.c:1022-1023); agg2RenderVectorSymbol moves the
        // square's centre (1, 1) to the origin, scales it by 6, and moves it to the point
        // (mapagg.cpp:615-623).
        let square = Symbol {
            kind: SymbolKind::Vector,
            paths: vec![vec![
                [0.0, 0.0],
                [2.0, 0.0],
                [2.0, 2.0],
                [0.0, 2.0],
                [0.0, 0.0],
            ]],
            extent: Some([2.0, 2.0]),
            ..circle()
        };
        let symbols = [square];
        let ring = |x0: f64, y0: f64, x1: f64, y1: f64| {
            vec![vec![p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1), p(x0, y0)]]
        };
        let marked = |symbol: Option<u32>, mode: RenderMode, transform| Style {
            symbol: symbol.map(|i| Bound::Value(SymbolRef::Index(i))),
            size: Some(Bound::Value(6.0)),
            offset: Some([Bound::Value(2.0), Bound::Value(1.0)]),
            geom_transform: transform,
            ..colored(Some(BLUE), (mode == RenderMode::Point).then_some(BLACK))
        };
        let drawn = |style: &Style, mode: RenderMode, geometry: &Geometry, factor: f64| {
            let mut canvas = Canvas::new(&symbols, 101, 101, factor);
            draw_ops(
                mode,
                geometry,
                style,
                Pass::Plain,
                &unit_view(),
                &mut canvas,
            )
        };
        let point = Geometry::Point(Coord { x: 50.0, y: 50.0 });
        let style = marked(Some(1), RenderMode::Point, None);
        // The point (50, 50) moved to (54, 52): the square from (48, 46) to (60, 58), filled,
        // then outlined open, as agg2RenderVectorSymbol strokes its path.
        let expected = vec![
            DrawOp::Fill {
                rings: ring(48.0, 46.0, 60.0, 58.0),
                color: BLUE,
                gamma: None,
            },
            DrawOp::Stroke {
                paths: ring(48.0, 46.0, 60.0, 58.0),
                closed: false,
                color: BLACK,
                width: 2.0,
                cap: LineCap::Butt,
                join: LineJoin::Miter,
                dashes: Dashes::default(),
            },
        ];
        assert_eq!(drawn(&style, RenderMode::Point, &point, 2.0), expected);
        assert_ne!(drawn(&style, RenderMode::Point, &point, 1.0), expected);
        let square = marked(None, RenderMode::Point, None);
        assert_eq!(
            drawn(&square, RenderMode::Point, &point, 2.0),
            drawn(&square, RenderMode::Point, &point, 1.0)
        );
        // GEOMTRANSFORM start at the L's first vertex, the pixel (10, 20), moved to (14, 22)
        // (mapgeomtransform.c:132-142): the square from (8, 16) to (20, 28), filled alone.
        let start = marked(Some(1), RenderMode::Line, Some(GeomTransform::Start));
        assert_eq!(
            drawn(&start, RenderMode::Line, &ell(), 2.0),
            [DrawOp::Fill {
                rings: ring(8.0, 16.0, 20.0, 28.0),
                color: BLUE,
                gamma: None,
            }]
        );
    }

    /// A style with `MINWIDTH` and `MAXWIDTH` whose `WIDTH` is bound to a feature's property that
    /// reads as no number, `nan`, which atof() reads as NaN.
    fn bounded_nan(min_width: f64, max_width: f64, outline: Option<Rgba>) -> Style {
        Style {
            width: Some(Bound::Value(forager_style_ir::c::atof(b"nan"))),
            min_width: Some(min_width),
            max_width: Some(max_width),
            ..colored(Some(BLUE), outline)
        }
    }

    #[test]
    fn a_line_bounds_its_width_as_ms_draw_line_symbol_does() {
        // msDrawLineSymbol, maprendering.c:622-624: MS_MIN with MAXWIDTH times the resolution
        // factor, then MS_MAX with MINWIDTH times it. A NaN width becomes MAXWIDTH, and a MINWIDTH
        // above the MAXWIDTH wins.
        let style = bounded_nan(2.0, 5.0, None);
        assert_eq!(
            stroke(&strokes_at(&style, 2.0)[0]).0,
            10.0,
            "MAXWIDTH 5 is 10"
        );
        let crossed = Style {
            width: Some(Bound::Value(4.0)),
            ..bounded_nan(6.0, 3.0, None)
        };
        assert_eq!(
            stroke(&strokes_at(&crossed, 1.0)[0]).0,
            6.0,
            "MINWIDTH last"
        );
    }

    #[test]
    fn a_polygon_bounds_its_outline_as_ms_draw_shade_symbol_does() {
        // msDrawShadeSymbol, maprendering.c:796-799: the WIDTH times the scale factor (NaN is not
        // 0), MS_MIN with MAXWIDTH, then MS_MAX with MINWIDTH, neither scaled. A NaN width becomes
        // MAXWIDTH as written, and a MINWIDTH above the MAXWIDTH wins.
        let outline = |style: &Style| {
            let mut ops = Vec::new();
            let mut canvas = Canvas::new(&[], 101, 101, 2.0);
            let ring = vec![vec![p(10.0, 10.0), p(20.0, 10.0), p(20.0, 20.0)]];
            assert!(shade(&mut ops, ring, style, &unit_view(), &mut canvas));
            stroke(&ops[1]).0
        };
        assert_eq!(outline(&bounded_nan(2.0, 5.0, Some(BLACK))), 5.0);
        let crossed = Style {
            width: Some(Bound::Value(4.0)),
            ..bounded_nan(6.0, 3.0, Some(BLACK))
        };
        assert_eq!(outline(&crossed), 6.0, "MINWIDTH last");
    }

    #[test]
    fn a_stand_in_square_bounds_its_outline_as_compute_symbol_style_does() {
        // The square stands for a marker symbol, whose outline computeSymbolStyle bounds,
        // maprendering.c:69-71: MS_MAX with MINWIDTH, then MS_MIN with MAXWIDTH, here at a
        // factor of 1. A NaN width becomes MINWIDTH, and a MAXWIDTH below the MINWIDTH wins.
        let outline = |style: &Style| {
            let mut canvas = Canvas::new(&[], 101, 101, 2.0);
            let point = Geometry::Point(Coord { x: 50.0, y: 50.0 });
            let ops = draw_ops(
                RenderMode::Point,
                &point,
                style,
                Pass::Plain,
                &unit_view(),
                &mut canvas,
            );
            stroke(&ops[1]).0
        };
        assert_eq!(outline(&bounded_nan(2.0, 5.0, Some(BLACK))), 2.0);
        let crossed = Style {
            width: Some(Bound::Value(4.0)),
            ..bounded_nan(6.0, 3.0, Some(BLACK))
        };
        assert_eq!(outline(&crossed), 3.0, "MAXWIDTH last");
    }

    #[test]
    fn at_a_factor_the_canvas_clips_a_class_s_hatches_by_the_scaled_buffer() {
        // msDrawShape's clip buffer at a factor of 2: a plain style 1 × 2 + 1, and 2 more.
        let mut canvas = Canvas::new(&[], 101, 101, 2.0);
        canvas.class(&[Style::default()]);
        assert_eq!(canvas.hatching.clip_buffer, 5.0);
    }

    /// A pixmap symbol of `path`.
    fn pixmap_symbol(path: Option<&str>) -> Symbol {
        Symbol {
            name: Some("pin".into()),
            kind: SymbolKind::Pixmap,
            paths: Vec::new(),
            extent: None,
            filled: false,
            anchor_point: None,
            character: None,
            font: None,
            image: path.map(str::to_owned),
            transparent: None,
            span: None,
        }
    }

    /// The operations of `style`, of symbol 1, at the point (50, 50) of [`unit_view`], with
    /// `pixmaps` for `symbols`, and the pixmaps the canvas left undrawn.
    fn pixmap_ops_of(
        style: &Style,
        symbols: &[Symbol],
        pixmaps: &Pixmaps,
    ) -> (Vec<DrawOp>, Vec<(usize, String)>) {
        let mut canvas = Canvas::new(symbols, 101, 101, 1.0).with_pixmaps(pixmaps);
        let point = Geometry::Point(Coord { x: 50.0, y: 50.0 });
        let ops = draw_ops(
            RenderMode::Point,
            &point,
            style,
            Pass::Plain,
            &unit_view(),
            &mut canvas,
        );
        let refused = canvas
            .pixmaps_refused()
            .into_iter()
            .map(|(index, _, why)| (index, why.to_owned()))
            .collect();
        (ops, refused)
    }

    #[test]
    fn a_pixmap_is_its_image_at_scale_1_unturned_and_resampled_otherwise() {
        let image = Rc::new(PremultipliedImage::new(4, 3, vec![9; 48]).unwrap());
        let pixmaps: Pixmaps = [("pin.png".to_owned(), Ok(Rc::clone(&image)))].into();
        let symbols = [pixmap_symbol(Some("pin.png"))];
        let style = |size: Option<f64>, angle: Option<f64>| Style {
            symbol: Some(Bound::Value(SymbolRef::Index(1))),
            size: size.map(Bound::Value),
            angle: angle.map(Bound::Value),
            offset: Some([Bound::Value(3.0), Bound::Value(-2.0)]),
            color: Some(Bound::Value(BLUE)),
            ..Style::default()
        };
        // Moved by the offset to (53, 48): MS_NINT(53 - 2) = 51 and MS_NINT(48 - 1.5) =
        // MS_NINT(46.5) = 46, half to even.
        for (size, angle) in [(None, None), (Some(3.0), Some(0.0)), (None, Some(360.0))] {
            let (ops, refused) = pixmap_ops_of(&style(size, angle), &symbols, &pixmaps);
            assert!(refused.is_empty());
            match &ops[..] {
                [
                    DrawOp::Pixmap {
                        image: drawn,
                        left,
                        top,
                    },
                ] => {
                    assert!(Rc::ptr_eq(drawn, &image), "the image itself, not a copy");
                    assert_eq!((*left, *top), (51, 46), "{size:?} {angle:?}");
                }
                other => panic!("{size:?} {angle:?}: {other:?}"),
            }
        }
        // A size other than the image's height scales it, an angle turns it: resampled into the
        // square about the point moved by the offset, (53, 48), agg2RenderPixmapSymbol's
        // half-side MS_NINT(4 × scale × 1.415) / 2 + 1 and the inverse of its transformation,
        // computed in trans_affine's order of operations at the pin.
        // SIZE 6: scale 6 / 3 = 2, MS_NINT(11.32) = 11, 11 / 2 + 1 = 6; the inverse of
        // translate(-2, -1.5), rotate(-0), scale(2), translate(53, 48).
        // ANGLE 30 at scale 1: MS_NINT(5.66) = 6, 6 / 2 + 1 = 4; rotate(-30 × MS_DEG_TO_RAD).
        let cases = [
            (Some(6.0), None, 6, [0.5, -0.0, -0.0, 0.5, -24.5, -22.5]),
            (
                None,
                Some(30.0),
                4,
                [
                    0.866_025_403_784_438_7,
                    0.499_999_999_999_999_94,
                    -0.499_999_999_999_999_94,
                    0.866_025_403_784_438_7,
                    -19.899_346_400_575_258,
                    -66.569_219_381_653_06,
                ],
            ),
        ];
        for (size, angle, half_side, [sx, shy, shx, sy, tx, ty]) in cases {
            let (ops, refused) = pixmap_ops_of(&style(size, angle), &symbols, &pixmaps);
            assert!(refused.is_empty());
            match &ops[..] {
                [
                    DrawOp::ResampledPixmap {
                        image: drawn,
                        at,
                        half,
                        inverse,
                    },
                ] => {
                    assert!(Rc::ptr_eq(drawn, &image), "the image itself, not a copy");
                    assert_eq!(*at, p(53.0, 48.0));
                    assert_eq!(*half, half_side, "{size:?} {angle:?}");
                    let expected = forager_render::Affine {
                        sx,
                        shy,
                        shx,
                        sy,
                        tx,
                        ty,
                    };
                    assert_eq!(*inverse, expected, "{size:?} {angle:?}");
                }
                other => panic!("{size:?} {angle:?}: {other:?}"),
            }
        }
        // A size of 0 is a scale of 0, which msDrawMarkerSymbol draws nothing at.
        let (ops, refused) = pixmap_ops_of(&style(Some(0.0), None), &symbols, &pixmaps);
        assert!(ops.is_empty() && refused.is_empty(), "{ops:?}");
    }

    #[test]
    fn a_pixmap_s_scale_is_bounded_by_its_size_bounds_times_the_resolution_factor() {
        // computeSymbolStyle, maprendering.c:62-65, at a scale and resolution factor of 2, for an
        // image 3 high: SIZE 6 × 2 = 12 bounded by MAXSIZE 5 × 2 = 10, scale 10 / 3; SIZE 1 × 2 =
        // 2 raised by MINSIZE 4 × 2 = 8, scale 8 / 3. The half-side of the 4 × 3 image: MS_NINT(4
        // × 10 / 3 × 1.415 = 18.866...) = 19, 19 / 2 + 1 = 10; MS_NINT(4 × 8 / 3 × 1.415 =
        // 15.093...) = 15, 15 / 2 + 1 = 8. The inverse, by trans_affine's invert, scales by
        // (10 / 3) × 1 / ((10 / 3) × (10 / 3)) = 0.29999999999999993 in doubles, and 0.375.
        let image = Rc::new(PremultipliedImage::new(4, 3, vec![9; 48]).unwrap());
        let pixmaps: Pixmaps = [("pin.png".to_owned(), Ok(Rc::clone(&image)))].into();
        let symbols = [pixmap_symbol(Some("pin.png"))];
        let bounded = |size: f64| Style {
            symbol: Some(Bound::Value(SymbolRef::Index(1))),
            size: Some(Bound::Value(size)),
            min_size: Some(4.0),
            max_size: Some(5.0),
            ..Style::default()
        };
        for (size, half_side, scale) in [(6.0, 10, 0.299_999_999_999_999_93), (1.0, 8, 0.375)] {
            let mut canvas = Canvas::new(&symbols, 101, 101, 2.0).with_pixmaps(&pixmaps);
            let point = Geometry::Point(Coord { x: 50.0, y: 50.0 });
            let ops = draw_ops(
                RenderMode::Point,
                &point,
                &bounded(size),
                Pass::Plain,
                &unit_view(),
                &mut canvas,
            );
            match &ops[..] {
                [DrawOp::ResampledPixmap { half, inverse, .. }] => {
                    assert_eq!(*half, half_side, "SIZE {size}");
                    assert_eq!(inverse.sx, scale, "SIZE {size}");
                    assert_eq!(inverse.sy, scale, "SIZE {size}");
                }
                other => panic!("SIZE {size}: {other:?}"),
            }
        }
    }

    #[test]
    fn a_pixmap_without_its_image_draws_nothing_and_the_canvas_says_why() {
        let pixmaps: Pixmaps = [(
            "gone.png".to_owned(),
            Err("the image gone.png does not load".into()),
        )]
        .into();
        let symbols = [
            pixmap_symbol(Some("gone.png")),
            pixmap_symbol(None),
            pixmap_symbol(Some("never.png")),
        ];
        let style = |index| Style {
            symbol: Some(Bound::Value(SymbolRef::Index(index))),
            color: Some(Bound::Value(BLUE)),
            ..Style::default()
        };
        let (ops, refused) = pixmap_ops_of(&style(1), &symbols, &pixmaps);
        assert!(ops.is_empty());
        assert_eq!(
            refused,
            [(1, "the image gone.png does not load".to_owned())]
        );
        let (ops, refused) = pixmap_ops_of(&style(2), &symbols, &pixmaps);
        assert!(ops.is_empty());
        assert_eq!(
            refused,
            [(2, "the pixmap symbol names no image".to_owned())]
        );
        let (ops, refused) = pixmap_ops_of(&style(3), &symbols, &pixmaps);
        assert!(ops.is_empty());
        assert_eq!(
            refused,
            [(
                3,
                "the image never.png is not loaded for this render".to_owned()
            )]
        );
    }

    #[test]
    fn a_pixmap_is_loaded_where_it_is_drawn_as_a_mark() {
        let symbols = [pixmap_symbol(Some("pin.png"))];
        let style = Style {
            symbol: Some(Bound::Value(SymbolRef::Index(1))),
            ..Style::default()
        };
        let transformed = Style {
            geom_transform: Some(GeomTransform::Vertices),
            ..style.clone()
        };
        assert_eq!(
            marked_pixmap(RenderMode::Point, &style, &symbols),
            Some("pin.png")
        );
        assert_eq!(marked_pixmap(RenderMode::Line, &style, &symbols), None);
        assert_eq!(marked_pixmap(RenderMode::Polygon, &style, &symbols), None);
        assert_eq!(
            marked_pixmap(RenderMode::Line, &transformed, &symbols),
            Some("pin.png")
        );
        assert_eq!(
            marked_pixmap(RenderMode::Point, &Style::default(), &symbols),
            None
        );
    }
}
