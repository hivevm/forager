//! Each seed of the render target, decoded as the target decodes it, builds what it was made for
//! (Q-3). A view added to the target's bytes, or a field or variant added to one, changes what
//! every older seed decodes to: three seeds lost their paths that way, unnoticed (ticket 08 of
//! revision-2026-11-18). Every seed under `fuzz/seeds/render/` has an entry in [`SEEDS`], naming
//! its paths, and a seed with none fails, so that a new seed names its path.
//!
//! A path is told from what the decoding builds, the map, its features, and its files, and from
//! what the engine hands the renderer: a [`Recorder`] in front of agg-rust sees each display item.
//! Where a path lives in one layer, the map is drawn with that layer alone.
// Verifies: Q-3

#[path = "../fuzz_targets/expr_input.rs"]
mod expr_input;
#[path = "../fuzz_targets/render_input.rs"]
mod render_input;

use std::cell::RefCell;
use std::fmt;
use std::path::Path as FilePath;

use forager_engine::Rendered;
use forager_ports::Geometry;
use forager_render::clip::drawn_width;
use forager_render::{DisplayItem, Draw, DrawOp, Point, Refused, Renderer, RgbaImage};
use forager_render_agg::AggRenderer;
use forager_style_ir::{
    GeomTransform, LabelTransform, Layer, Rgba, ShapeOp, SymbolKind, TransformShape, TransformTree,
};
use render_input::{Decoded, decode};

/// How far out, in pixels, an end of a segment lies for the segment to be far: past it, the
/// parameter of a crossing measured from that end rounds to 1, as in the timeout 5e8fcf82 fixed.
const FAR: f64 = 1e15;

/// A step of the transform tree, by the name of its node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Simplify,
    Simplifypt,
    Buffer,
    SmoothSia,
}

/// A class style's `GEOMTRANSFORM` other than a tree, by the name of its variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Placed {
    Start,
    End,
    Vertices,
    Bbox,
    Centroid,
    LabelPoint,
    LabelPoly,
    LabelCenter,
}

impl Placed {
    /// Whether the style's transform is this one.
    fn is(self, transform: Option<&GeomTransform>) -> bool {
        let variant = match transform {
            Some(GeomTransform::Start) => Placed::Start,
            Some(GeomTransform::End) => Placed::End,
            Some(GeomTransform::Vertices) => Placed::Vertices,
            Some(GeomTransform::Bbox) => Placed::Bbox,
            Some(GeomTransform::Centroid) => Placed::Centroid,
            Some(GeomTransform::LabelPoint) => Placed::LabelPoint,
            Some(GeomTransform::LabelPoly) => Placed::LabelPoly,
            Some(GeomTransform::LabelCenter) => Placed::LabelCenter,
            Some(GeomTransform::Expression(_)) | None => return false,
        };
        variant == self
    }

    /// Whether a class style of the transform draws on a line or a polygon: the label transforms
    /// draw nothing there, and labelcenter is reported.
    fn draws(self) -> bool {
        !matches!(
            self,
            Placed::LabelPoint | Placed::LabelPoly | Placed::LabelCenter
        )
    }
}

/// What a seed was made for.
#[derive(Clone, Copy, Debug)]
enum Path {
    /// The TrueType symbol, its glyph handed the renderer by the layer named, drawn alone: a
    /// glyph, or for the polygon layer a tiled fill whose tile holds one.
    TrueType(&'static str),
    /// The TrueType symbol's font is the one the bytes supply, and the point layer of the symbol,
    /// drawn alone, hands the renderer a glyph of it.
    InputFont,
    /// The point layer of the TrueType symbol, drawn alone, hands the renderer a glyph whose
    /// outline is wider, or narrower than its negative, than the renderer's bound on a stroke on
    /// the map's image, which the renderer cuts it to.
    GlyphOutlineBounded,
    /// The circle layer, drawn alone, hands the renderer an item.
    CircleLayer,
    /// A transform tree of the polygon and line layers, theirs or a class style's, holds the step,
    /// and the layers transform a feature: see [`transforms`].
    Step(Step),
    /// A tree of the polygon and line layers holds `simplifypt`, and a line or a ring among the
    /// features has three points or more, so that `simplifypt` has a shape it can simplify,
    /// rather than stopping at one of a point or two.
    SimplifyptOfThreePoints,
    /// The polygon or the line layer applies a tree of its own that is more than `[shape]`, and
    /// the layers transform a feature: see [`transforms`].
    LayerTransform,
    /// A class's style on the polygon or the line layer applies a tree, and the layers transform a
    /// feature: see [`transforms`].
    ClassTransform,
    /// A class's style on the polygon or the line layer has the transform, and the layers
    /// transform a feature: see [`transforms`]. Of a transform that draws, the layers drawn alone
    /// hand the renderer more items with the style than without it. Of a label's transform, which
    /// draws nothing there, the layers drawn alone hand the renderer an item and neither reports
    /// that its own `GEOMTRANSFORM` drew nothing, so that their class styles ran; of labelcenter,
    /// the engine also warns of the style.
    StyleTransform(Placed),
    /// A multipoint of several parts is the feature of the layer `MULTIPOINTS`, which applies a
    /// tree of its own that is more than `[shape]`, and the layer drawn alone hands the renderer
    /// an item, which a line layer draws of no multipoint untransformed.
    MultiPointLayerTransform,
    /// A polygon filled with a tile of the pixmap.
    TiledPixmap,
    /// The pixmap resampled at a point or along a line by the pixmap layers.
    ResampledPixmap,
    /// A line brushed with a tile.
    BrushedLine,
    /// An open path stroked or brushed with a segment that has an end past [`FAR`] pixels out.
    FarSegment,
    /// A stroke whose pen is not antialiased, the first style's `ANTIALIAS FALSE`.
    Aliased,
    /// A map turned by its `ANGLE`, map and layers projected, draws.
    TurnedAndProjected,
    /// The layers' `MAXFEATURES` hold back what they would draw without it.
    MaxFeatures,
    /// A label held in the label cache, walked with an edge buffer, a layer drawn after the cache,
    /// and the label's labelpoint marker.
    LabelCache,
    /// The label's markers at its point and at the centre of its box, handed the renderer by the
    /// point layer drawn alone.
    LabelMarkers,
    /// The label's style of `labelpoly`: the point layer drawn alone hands the renderer more items
    /// with it than without it.
    LabelPoly,
    /// The map has a view the engine refuses.
    ViewInvalid,
}

impl fmt::Display for Path {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Path::TrueType(layer) => write!(f, "a TrueType glyph drawn by the layer {layer}"),
            Path::InputFont => write!(f, "a TrueType glyph of the font in the bytes"),
            Path::GlyphOutlineBounded => write!(f, "a glyph outline past the bound of a stroke"),
            Path::CircleLayer => write!(f, "a circle drawn by the circle layer"),
            Path::Step(step) => write!(f, "a step {step:?} of the transform tree"),
            Path::SimplifyptOfThreePoints => {
                write!(f, "a simplifypt step over a shape of three points or more")
            }
            Path::LayerTransform => write!(f, "a layer's transform tree"),
            Path::ClassTransform => write!(f, "a class style's transform tree"),
            Path::StyleTransform(placed) => {
                write!(f, "a class style's GEOMTRANSFORM {placed:?}")
            }
            Path::MultiPointLayerTransform => {
                write!(
                    f,
                    "a layer's transform tree drawn on a multipoint of several parts"
                )
            }
            Path::TiledPixmap => write!(f, "a tiled fill of the pixmap"),
            Path::ResampledPixmap => write!(f, "a resampled pixmap"),
            Path::BrushedLine => write!(f, "a brushed line"),
            Path::FarSegment => write!(f, "a segment with an end past {FAR:e} px"),
            Path::Aliased => write!(f, "an aliased stroke"),
            Path::TurnedAndProjected => write!(f, "a turned, projected map that draws"),
            Path::MaxFeatures => write!(f, "a MAXFEATURES that holds features back"),
            Path::LabelCache => write!(f, "a label cached, walked with an edge buffer"),
            Path::LabelMarkers => write!(f, "a label's labelpoint and labelcenter markers"),
            Path::LabelPoly => write!(f, "a label's labelpoly style"),
            Path::ViewInvalid => write!(f, "a view the engine refuses"),
        }
    }
}

/// Every seed under `fuzz/seeds/render/`, and the paths it was made for.
const SEEDS: &[(&str, &[Path])] = &[
    ("antialias-false", &[Path::Aliased]),
    ("brushed-line-join-distance-overflow", &[Path::BrushedLine]),
    ("buffer-edges-too-short", &[Path::Step(Step::Buffer)]),
    ("circle-layer", &[Path::CircleLayer]),
    ("class-geomtransform", &[Path::ClassTransform]),
    ("far-end-rounds-to-its-parameter", &[Path::FarSegment]),
    ("far-end-rounds-to-its-parameter-slow", &[Path::FarSegment]),
    ("label-cache-edge-buffer", &[Path::LabelCache]),
    ("label-labelpoly", &[Path::LabelPoly]),
    ("label-markers", &[Path::LabelMarkers]),
    ("layer-geomtransform", &[Path::LayerTransform]),
    (
        "layer-geomtransform-multipoint",
        &[Path::MultiPointLayerTransform],
    ),
    (
        "line-buffer-near-the-grid-step",
        &[Path::Step(Step::Buffer)],
    ),
    ("map-angle-projected", &[Path::TurnedAndProjected]),
    ("max-features", &[Path::MaxFeatures]),
    ("pixmap-resampled", &[Path::ResampledPixmap]),
    ("pixmap-tiled-fill", &[Path::TiledPixmap]),
    (
        "simplify-coordinate-no-number",
        &[Path::Step(Step::Simplify)],
    ),
    (
        "smoothsia-nested-layers-of-one-tree",
        &[Path::Step(Step::SmoothSia)],
    ),
    (
        "smoothsia-parts-of-no-point-and-one",
        &[Path::Step(Step::SmoothSia)],
    ),
    ("smoothsia-short-parts", &[Path::Step(Step::SmoothSia)]),
    (
        "style-geomtransform-bbox",
        &[Path::StyleTransform(Placed::Bbox)],
    ),
    (
        "style-geomtransform-centroid",
        &[Path::StyleTransform(Placed::Centroid)],
    ),
    (
        "style-geomtransform-end",
        &[Path::StyleTransform(Placed::End)],
    ),
    (
        "style-geomtransform-labelcenter",
        &[Path::StyleTransform(Placed::LabelCenter)],
    ),
    (
        "style-geomtransform-labelpoint",
        &[Path::StyleTransform(Placed::LabelPoint)],
    ),
    (
        "style-geomtransform-labelpoly",
        &[Path::StyleTransform(Placed::LabelPoly)],
    ),
    (
        "style-geomtransform-start",
        &[Path::StyleTransform(Placed::Start)],
    ),
    (
        "style-geomtransform-vertices",
        &[Path::StyleTransform(Placed::Vertices)],
    ),
    ("transform-buffer", &[Path::Step(Step::Buffer)]),
    ("transform-simplify", &[Path::Step(Step::Simplify)]),
    (
        "transform-simplifypt",
        &[Path::Step(Step::Simplifypt), Path::SimplifyptOfThreePoints],
    ),
    ("truetype-fill", &[Path::TrueType("glyph-fills")]),
    (
        "truetype-input-font",
        &[Path::TrueType("glyph-points"), Path::InputFont],
    ),
    ("truetype-marker", &[Path::TrueType("glyph-points")]),
    (
        "truetype-marker-outline-wide",
        &[Path::TrueType("glyph-points"), Path::GlyphOutlineBounded],
    ),
    ("zeros", &[Path::ViewInvalid]),
];

/// What the engine handed the renderer of one display item.
#[derive(Clone, Debug)]
enum Op {
    Stroke { far: bool, aliased: bool },
    Brushed { far: bool },
    Fill,
    Hatch,
    Glyphs { outline: Option<f64> },
    Pixmap,
    Resampled,
    Tiled { pixmap: bool, glyph: bool },
}

impl Op {
    fn of(op: &DrawOp) -> Op {
        match op {
            DrawOp::Stroke {
                paths, closed, pen, ..
            } => Op::Stroke {
                far: !closed && far(paths),
                aliased: !pen.antialiased,
            },
            DrawOp::BrushedLine { paths, .. } => Op::Brushed { far: far(paths) },
            DrawOp::Fill { .. } => Op::Fill,
            DrawOp::Hatch(_) => Op::Hatch,
            DrawOp::Glyphs { outline, .. } => Op::Glyphs {
                outline: outline.map(|outline| outline.width),
            },
            DrawOp::Pixmap { .. } => Op::Pixmap,
            DrawOp::ResampledPixmap { .. } => Op::Resampled,
            DrawOp::TiledFill { tile, .. } => Op::Tiled {
                pixmap: tile.marks().iter().any(|mark| {
                    matches!(mark, DrawOp::Pixmap { .. } | DrawOp::ResampledPixmap { .. })
                }),
                glyph: tile
                    .marks()
                    .iter()
                    .any(|mark| matches!(mark, DrawOp::Glyphs { .. })),
            },
        }
    }
}

/// Whether a path has a segment of finite ends and a finite difference, one end past [`FAR`]:
/// one the clip measures, rather than drops.
fn far(paths: &[Vec<Point>]) -> bool {
    let finite = |p: Point| p.x.is_finite() && p.y.is_finite();
    paths.iter().any(|path| {
        path.windows(2).any(|pair| {
            let (a, b) = (pair[0], pair[1]);
            finite(a)
                && finite(b)
                && (b.x - a.x).is_finite()
                && (b.y - a.y).is_finite()
                && [a.x, a.y, b.x, b.y].iter().any(|c| c.abs() > FAR)
        })
    })
}

/// agg-rust, with every display item handed it seen on the way.
struct Recorder<'s> {
    inner: AggRenderer,
    seen: &'s RefCell<Vec<Op>>,
}

/// A layer of the [`Recorder`]'s image, its items seen as the recorder's are.
struct RecorderLayer<'s, L> {
    inner: L,
    seen: &'s RefCell<Vec<Op>>,
}

fn see(seen: &RefCell<Vec<Op>>, item: &DisplayItem) {
    seen.borrow_mut().push(Op::of(&item.op));
}

impl Draw for Recorder<'_> {
    fn draw(&mut self, item: &DisplayItem) {
        see(self.seen, item);
        self.inner.draw(item);
    }
}

impl<L: Draw> Draw for RecorderLayer<'_, L> {
    fn draw(&mut self, item: &DisplayItem) {
        see(self.seen, item);
        self.inner.draw(item);
    }
}

impl Renderer for Recorder<'_> {
    type Layer<'a>
        = RecorderLayer<'a, <AggRenderer as Renderer>::Layer<'a>>
    where
        Self: 'a;

    fn begin(&mut self, width: u32, height: u32, background: Rgba) {
        self.inner.begin(width, height, background);
    }

    fn layer(&mut self, opacity: u8) -> Self::Layer<'_> {
        RecorderLayer {
            inner: self.inner.layer(opacity),
            seen: self.seen,
        }
    }

    fn take_refused(&mut self) -> Refused {
        self.inner.take_refused()
    }

    fn finish(self) -> RgbaImage {
        self.inner.finish()
    }
}

/// What the map of `decoded` hands the renderer, and the render.
fn drawn(decoded: &Decoded) -> (Vec<Op>, Rendered) {
    let seen = RefCell::new(Vec::new());
    let rendered = decoded.render(Recorder {
        inner: AggRenderer::new(),
        seen: &seen,
    });
    (seen.into_inner(), rendered)
}

/// What the map of `decoded` hands the renderer with the layer named `layer` alone.
fn drawn_alone(decoded: &mut Decoded, layer: &str) -> Vec<Op> {
    decoded
        .map
        .layers
        .retain(|l| l.name.as_deref() == Some(layer));
    drawn(decoded).0
}

/// The layers that apply transform trees, the polygon and the line layer: the first two.
const TRANSFORMED: [&str; 2] = ["polygons", "lines"];

fn transformed_layers(decoded: &Decoded) -> impl Iterator<Item = &Layer> {
    decoded
        .map
        .layers
        .iter()
        .filter(|layer| TRANSFORMED.contains(&layer.name.as_deref().unwrap_or_default()))
}

/// Whether the polygon and line layers, drawn alone, transform a feature: the map has a view the
/// engine draws, a line or a polygon among the features, and the layers hand the renderer an item
/// or report that a transform drew nothing (`geomtransform-*`).
fn transforms(decoded: &mut Decoded) -> bool {
    if !has_line_or_polygon(decoded) {
        return false;
    }
    decoded
        .map
        .layers
        .retain(|layer| TRANSFORMED.contains(&layer.name.as_deref().unwrap_or_default()));
    let (ops, rendered) = drawn(decoded);
    let codes = || rendered.diagnostics.iter().map(|d| d.code.as_str());
    !codes().any(|code| code == "view-invalid")
        && (!ops.is_empty() || codes().any(|code| code.starts_with("geomtransform-")))
}

/// The transform trees of the polygon and line layers: theirs and their classes' styles'.
fn trees(decoded: &Decoded) -> impl Iterator<Item = &TransformShape> {
    transformed_layers(decoded).flat_map(|layer| {
        let styles = layer
            .classes
            .iter()
            .flat_map(|class| &class.styles)
            .filter_map(|style| match &style.geom_transform {
                Some(GeomTransform::Expression(tree)) => Some(tree),
                _ => None,
            });
        layer
            .geom_transform
            .iter()
            .chain(styles)
            .filter_map(|tree| match tree {
                TransformTree::Drawn(shape) => Some(shape),
                TransformTree::NotDrawn(_) => None,
            })
    })
}

/// Whether the shape holds the step, at any depth.
fn holds(shape: &TransformShape, step: Step) -> bool {
    let (this, inner) = match shape {
        TransformShape::Shape => return false,
        TransformShape::Op(ShapeOp::Simplify, inner, _) => (Some(Step::Simplify), inner),
        TransformShape::Op(ShapeOp::Simplifypt, inner, _) => (Some(Step::Simplifypt), inner),
        TransformShape::Op(ShapeOp::Buffer, inner, _) => (Some(Step::Buffer), inner),
        TransformShape::SmoothSia { shape, .. } => (Some(Step::SmoothSia), shape),
        TransformShape::Op(ShapeOp::Generalize, inner, _)
        | TransformShape::Op(ShapeOp::Densify, inner, _)
        | TransformShape::Outer(inner)
        | TransformShape::Inner(inner) => (None, inner),
    };
    this == Some(step) || holds(inner, step)
}

fn has_line_or_polygon(decoded: &Decoded) -> bool {
    decoded.features.iter().any(|feature| {
        matches!(
            feature.geometry,
            Geometry::LineString(_) | Geometry::Polygon(_)
        )
    })
}

fn truetype(decoded: &Decoded) -> Option<&forager_style_ir::Symbol> {
    decoded
        .map
        .symbols
        .iter()
        .find(|symbol| symbol.kind == SymbolKind::Truetype)
}

/// Whether the seed of `data` builds `path`.
fn reaches(data: &[u8], path: Path) -> bool {
    let Some(mut decoded) = decode(data) else {
        return false;
    };
    match path {
        Path::TrueType(layer) => {
            truetype(&decoded).is_some()
                && drawn_alone(&mut decoded, layer)
                    .iter()
                    .any(|seen| matches!(seen, Op::Glyphs { .. } | Op::Tiled { glyph: true, .. }))
        }
        Path::InputFont => {
            truetype(&decoded)
                .is_some_and(|symbol| symbol.font.as_deref() == Some(render_input::INPUT_FONT_NAME))
                && drawn_alone(&mut decoded, "glyph-points")
                    .iter()
                    .any(|seen| matches!(seen, Op::Glyphs { .. }))
        }
        Path::GlyphOutlineBounded => {
            let [width, height] = decoded.map.size.unwrap_or_default();
            truetype(&decoded).is_some()
                && drawn_alone(&mut decoded, "glyph-points")
                    .iter()
                    .any(|seen| match seen {
                        Op::Glyphs { outline: Some(w) } => {
                            w.is_finite() && drawn_width(w.abs(), width, height) < w.abs()
                        }
                        _ => false,
                    })
        }
        Path::CircleLayer => !drawn_alone(&mut decoded, "circles").is_empty(),
        Path::Step(step) => {
            trees(&decoded).any(|tree| holds(tree, step)) && transforms(&mut decoded)
        }
        Path::SimplifyptOfThreePoints => {
            let three = decoded
                .features
                .iter()
                .any(|feature| match &feature.geometry {
                    Geometry::LineString(line) => line.len() >= 3,
                    Geometry::Polygon(rings) => rings.iter().any(|ring| ring.len() >= 3),
                    _ => false,
                });
            three && trees(&decoded).any(|tree| holds(tree, Step::Simplifypt))
        }
        Path::LayerTransform => {
            let own = transformed_layers(&decoded).any(|layer| {
                matches!(
                    &layer.geom_transform,
                    Some(TransformTree::Drawn(shape)) if *shape != TransformShape::Shape
                )
            });
            own && transforms(&mut decoded)
        }
        Path::ClassTransform => {
            let styled = transformed_layers(&decoded).any(|layer| {
                layer.classes.iter().any(|class| {
                    class.styles.iter().any(|style| {
                        matches!(&style.geom_transform, Some(GeomTransform::Expression(_)))
                    })
                })
            });
            styled && transforms(&mut decoded)
        }
        Path::StyleTransform(placed) => {
            let styled = transformed_layers(&decoded).any(|layer| {
                layer.classes.iter().any(|class| {
                    class
                        .styles
                        .iter()
                        .any(|style| placed.is(style.geom_transform.as_ref()))
                })
            });
            if !styled || !transforms(&mut decoded) {
                return false;
            }
            if !placed.draws() {
                decoded.map.layers.retain(|layer| {
                    TRANSFORMED.contains(&layer.name.as_deref().unwrap_or_default())
                });
                let (ops, rendered) = drawn(&decoded);
                let messages = || rendered.diagnostics.iter().map(|d| d.message.as_str());
                let layer_undrawn =
                    messages().any(|message| message.contains(" draws nothing: its GEOMTRANSFORM"));
                let warned = placed != Placed::LabelCenter
                    || messages()
                        .any(|message| message.contains("style of GEOMTRANSFORM labelcenter"));
                return !ops.is_empty() && !layer_undrawn && warned;
            }
            let with = drawn(&decoded).0.len();
            for layer in &mut decoded.map.layers {
                for class in &mut layer.classes {
                    class
                        .styles
                        .retain(|style| !placed.is(style.geom_transform.as_ref()));
                }
            }
            with > drawn(&decoded).0.len()
        }
        Path::MultiPointLayerTransform => {
            let parts = decoded.multipoint.as_ref().is_some_and(|feature| {
                matches!(&feature.geometry, Geometry::MultiPoint(parts) if parts.len() >= 2)
            });
            let own = decoded.map.layers.iter().any(|layer| {
                layer.name.as_deref() == Some(render_input::MULTIPOINTS)
                    && matches!(
                        &layer.geom_transform,
                        Some(TransformTree::Drawn(shape)) if *shape != TransformShape::Shape
                    )
            });
            parts && own && !drawn_alone(&mut decoded, render_input::MULTIPOINTS).is_empty()
        }
        Path::LabelPoly => {
            let mut without = decode(data).expect("decoded above");
            for layer in &mut without.map.layers {
                for class in &mut layer.classes {
                    for label in &mut class.labels {
                        label
                            .styles
                            .retain(|style| style.geom_transform != LabelTransform::Poly);
                    }
                }
            }
            let without = drawn_alone(&mut without, "points").len();
            drawn_alone(&mut decoded, "points").len() > without
        }
        Path::TiledPixmap => drawn(&decoded)
            .0
            .iter()
            .any(|seen| matches!(seen, Op::Tiled { pixmap: true, .. })),
        Path::ResampledPixmap => ["pixmap-points", "pixmap-lines"].iter().any(|layer| {
            let mut alone = decode(data).expect("decoded above");
            drawn_alone(&mut alone, layer)
                .iter()
                .any(|seen| matches!(seen, Op::Resampled))
        }),
        Path::BrushedLine => drawn(&decoded)
            .0
            .iter()
            .any(|seen| matches!(seen, Op::Brushed { .. })),
        Path::FarSegment => drawn(&decoded).0.iter().any(|seen| {
            matches!(
                seen,
                Op::Stroke { far: true, .. } | Op::Brushed { far: true }
            )
        }),
        Path::Aliased => drawn(&decoded)
            .0
            .iter()
            .any(|seen| matches!(seen, Op::Stroke { aliased: true, .. })),
        Path::TurnedAndProjected => {
            decoded.map.angle.is_some_and(|angle| angle != 0.0)
                && decoded.map.projection.is_some()
                && !drawn(&decoded).0.is_empty()
        }
        Path::MaxFeatures => {
            let limited = drawn(&decoded).0.len();
            for layer in &mut decoded.map.layers {
                layer.max_features = None;
            }
            drawn(&decoded).0.len() > limited
        }
        Path::LabelCache => {
            let labelpoint = decoded.map.layers.iter().any(|layer| {
                layer.classes.iter().any(|class| {
                    class.labels.iter().any(|label| {
                        label
                            .styles
                            .iter()
                            .any(|style| style.geom_transform == LabelTransform::Point)
                    })
                })
            });
            labelpoint
                && decoded
                    .map
                    .label_cache_edge_buffer
                    .is_some_and(|edge| edge > 0)
                && decoded
                    .map
                    .layers
                    .iter()
                    .any(|layer| layer.post_label_cache)
                && !drawn(&decoded).1.labels.is_empty()
        }
        Path::LabelMarkers => {
            drawn_alone(&mut decoded, "points")
                .iter()
                .filter(|seen| matches!(seen, Op::Pixmap | Op::Resampled))
                .count()
                >= 2
        }
        Path::ViewInvalid => drawn(&decoded)
            .1
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "view-invalid"),
    }
}

/// The paths the seed named `name` misses, of those its entry names, each named with the seed.
fn check(name: &str, data: &[u8], paths: &[Path]) -> Result<(), String> {
    let missed: Vec<String> = paths
        .iter()
        .filter(|&&path| !reaches(data, path))
        .map(|path| format!("the seed {name} no longer builds {path}"))
        .collect();
    if missed.is_empty() {
        Ok(())
    } else {
        Err(missed.join("\n"))
    }
}

/// The entry of each seed named, or the names of those without one, and of the entries that name
/// no seed.
fn entries(names: &[String]) -> Result<Vec<(&str, &'static [Path])>, String> {
    let mut problems = Vec::new();
    let mut found = Vec::new();
    for name in names {
        match SEEDS.iter().find(|(seed, _)| seed == name) {
            Some((_, paths)) => found.push((name.as_str(), *paths)),
            None => problems.push(format!(
                "the seed {name} has no entry in SEEDS; name the path it was made for"
            )),
        }
    }
    for (seed, _) in SEEDS {
        if !names.iter().any(|name| name == seed) {
            problems.push(format!("SEEDS names {seed}, which is no seed"));
        }
    }
    if problems.is_empty() {
        Ok(found)
    } else {
        Err(problems.join("\n"))
    }
}

fn seed_dir() -> &'static FilePath {
    FilePath::new(concat!(env!("CARGO_MANIFEST_DIR"), "/seeds/render"))
}

fn seed_names() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(seed_dir())
        .expect("the render target has seeds")
        .map(|entry| {
            entry
                .expect("a seed is read")
                .file_name()
                .into_string()
                .expect("a seed's name is UTF-8")
        })
        .collect();
    names.sort();
    names
}

#[test]
fn every_seed_builds_the_paths_its_entry_names() {
    let names = seed_names();
    let seeds = entries(&names).unwrap_or_else(|problems| panic!("{problems}"));
    let missed: Vec<String> = seeds
        .into_iter()
        .filter_map(|(name, paths)| {
            let data = std::fs::read(seed_dir().join(name)).expect("a seed is read");
            check(name, &data, paths).err()
        })
        .collect();
    assert!(missed.is_empty(), "{}", missed.join("\n"));
}

#[test]
fn a_seed_that_no_longer_builds_its_path_fails_with_the_seed_and_the_path() {
    let data = std::fs::read(seed_dir().join("zeros")).expect("a seed is read");
    let error = check("zeros", &data, &[Path::ViewInvalid, Path::TiledPixmap])
        .expect_err("zeros draws no tiled fill");
    assert_eq!(
        error,
        "the seed zeros no longer builds a tiled fill of the pixmap"
    );
}

#[test]
fn a_seed_without_an_entry_fails_with_its_name() {
    let mut names = seed_names();
    names.push("a-new-seed".to_owned());
    let error = entries(&names).expect_err("a-new-seed has no entry");
    assert_eq!(
        error,
        "the seed a-new-seed has no entry in SEEDS; name the path it was made for"
    );
}

#[test]
fn an_entry_without_its_seed_fails_with_its_name() {
    let names: Vec<String> = seed_names()
        .into_iter()
        .filter(|name| name != "zeros")
        .collect();
    let error = entries(&names).expect_err("zeros is gone");
    assert_eq!(error, "SEEDS names zeros, which is no seed");
}
