use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::expression::Keys;
use crate::{Expr, Span};

/// The least `resolution` and `def_resolution` a map may have, in dpi: MapServer's
/// `MS_RESOLUTION_MIN`, which its loader holds both to.
pub const MIN_RESOLUTION: f64 = 10.0;
/// The greatest `resolution` and `def_resolution` a map may have, in dpi: MapServer's
/// `MS_RESOLUTION_MAX`.
pub const MAX_RESOLUTION: f64 = 1000.0;

/// A map: its image, its extent, and its layers in drawing order.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Map {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extent: Option<Extent>,
    /// Width and height of the image in pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<[u32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_color: Option<Rgba>,
    /// The map's projection as a proj4 string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projection: Option<String>,
    /// The resolution of the image in dpi; MapServer's default is 72. A reader drops a value
    /// outside 10 to 1000 with a warning, as MapServer's loader refuses one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "schema", schemars(range(min = MIN_RESOLUTION, max = MAX_RESOLUTION)))]
    pub resolution: Option<f64>,
    /// The resolution the map's pixel lengths are written for, in dpi; MapServer's default is 72.
    /// Every length in pixels is drawn scaled by `resolution` over this. A reader drops a value
    /// outside 10 to 1000 with a warning, as MapServer's loader refuses one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "schema", schemars(range(min = MIN_RESOLUTION, max = MAX_RESOLUTION)))]
    pub def_resolution: Option<f64>,
    /// The unit of the map's coordinates, which the scale and sizes in other units are computed
    /// in; MapServer's default is meters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub units: Option<Units>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layers: Vec<Layer>,
    /// The map's fonts by name, each a resource path inside the base context, not read at load.
    /// Names match without regard to case, as MapServer's font set compares them, so no two
    /// differ only in case.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fonts: BTreeMap<String, String>,
    /// The map's symbols in MapServer's order: those of its `SYMBOL` blocks, then those of its
    /// symbol set, then the image files its styles name. The one at position `i` has MapServer's
    /// index `i + 1`; index 0 is MapServer's default symbol, a filled circle of size 1, which the
    /// list leaves out.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub symbols: Vec<Symbol>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,
}

/// Where a condition of the IR stands: the field of each element of a list whose expression
/// decides which features that element draws. A reader that cannot read one keeps it as one that
/// never matches, since dropped it would match every feature (docs/CONVENTIONS.md).
pub(crate) struct Condition {
    pub list: &'static str,
    pub field: &'static str,
}

/// The conditions of the IR: a layer's filter, a class's expression, and a label's expression.
/// [`Map::conditions`] reads them from the typed map; a test holds the two together.
pub(crate) const CONDITIONS: [Condition; 3] = [
    Condition {
        list: "layers",
        field: "filter",
    },
    Condition {
        list: "classes",
        field: "expression",
    },
    Condition {
        list: "labels",
        field: "expression",
    },
];

impl Map {
    /// The map's conditions, at the places [`CONDITIONS`] names.
    pub(crate) fn conditions(&self) -> Vec<&Expr> {
        let layers = self.layers.iter();
        layers
            .flat_map(|layer| {
                let classes = layer.classes.iter();
                let labels = classes
                    .clone()
                    .flat_map(|c| &c.labels)
                    .filter_map(|l| l.expression.as_ref());
                let classes = classes.filter_map(|c| c.expression.as_ref());
                layer.filter.iter().chain(classes).chain(labels)
            })
            .collect()
    }

    /// Every expression of the map: its conditions, and those of its styles and labels.
    pub(crate) fn expressions(&self) -> Vec<&Expr> {
        let classes = self.layers.iter().flat_map(|layer| &layer.classes);
        let properties = classes.flat_map(|class| {
            let styles = class.styles.iter().flat_map(Style::expressions);
            styles.chain(class.labels.iter().flat_map(Label::expressions))
        });
        let mut found = self.conditions();
        found.extend(properties);
        found
    }
}

/// A symbol a style draws.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Symbol {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub kind: SymbolKind,
    /// A vector symbol's lines, each a list of `[x, y]`, y downwards, shifted as MapServer shifts
    /// them so that none is negative; an ellipse's one point, its width and height.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<Vec<[f64; 2]>>,
    /// The width and height of a vector symbol or an ellipse, as MapServer measures them. A
    /// style's size is the symbol's height drawn, or its width where it has no height.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extent: Option<[f64; 2]>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub filled: bool,
    /// The point of the symbol placed on the feature, as fractions of its extent; without one,
    /// the centre.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor_point: Option<[f64; 2]>,
    /// The glyph of a TrueType symbol, and its font by the name the font set gives it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub character: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    /// The image of a pixmap or SVG symbol: a resource path inside the base context, not read at
    /// load (Q-8).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// The palette index of a pixmap that draws transparent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transparent: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum SymbolKind {
    Vector,
    Ellipse,
    Pixmap,
    Simple,
    Truetype,
    Hatch,
    Svg,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Extent {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Layer {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub render_mode: Option<RenderMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<LayerStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceRef>,
    /// Which of the source's features the layer reads; without one, all of them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<Expr>,
    /// The unit of its styles' sizes, widths, gaps, patterns, and offsets; without one, pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_units: Option<Units>,
    /// The scale, as MapServer computes it, at which the layer's symbols are drawn at their size
    /// as written; at other scales their size scales with it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol_scale_denom: Option<f64>,
    /// The projection of the layer's features; without one, the map's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projection: Option<LayerProjection>,
    /// The layer is drawn at a scale above `min_scale_denom` and up to `max_scale_denom`; a
    /// `max_scale_denom` of 0 bounds nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_scale_denom: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_scale_denom: Option<f64>,
    /// With an opacity, in percent from 0 to 100, the layer is drawn into a transparent image of
    /// its own, which is laid over the map at that opacity; at 0 the layer is not drawn. Without
    /// one, it is drawn straight onto the map.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f64>,
    /// Whether the layer's labels go to the label cache, MapServer's `LABELCACHE`: placed with
    /// the labels of every layer and drawn after them. Without the cache, each label is drawn at
    /// once after its feature, untested against any other. Absent, it is true, MapServer's
    /// default `LABELCACHE ON`.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub label_cache: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub classes: Vec<Class>,
    /// The features written into the layer (ADR-0018). A layer with any is read from them alone,
    /// whatever source it names or a host binds to it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<InlineFeature>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,
}

impl Default for Layer {
    /// A layer of no property set, its labels in the label cache.
    fn default() -> Self {
        Self {
            name: None,
            render_mode: None,
            status: None,
            source: None,
            filter: None,
            size_units: None,
            symbol_scale_denom: None,
            projection: None,
            min_scale_denom: None,
            max_scale_denom: None,
            opacity: None,
            label_cache: true,
            classes: Vec::new(),
            features: Vec::new(),
            span: None,
        }
    }
}

fn yes() -> bool {
    true
}

fn is_true(value: &bool) -> bool {
    *value
}

/// A feature written into a layer, a shape as MapServer holds one (ADR-0018).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct InlineFeature {
    #[serde(rename = "type")]
    pub shape_type: ShapeType,
    /// Its parts, each a list of `[x, y]` in the layer's coordinates: a point shape's points, a
    /// line shape's lines, a polygon shape's rings, an outer ring not told from an inner one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<Vec<[f64; 2]>>,
    /// Its properties, each a string, in order; a reader finds a name without regard to ASCII
    /// case, the first of two that match.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub properties: Vec<FeatureProperty>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,
}

/// The type of an inline feature's shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum ShapeType {
    Point,
    Line,
    Polygon,
}

/// A property of an inline feature.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct FeatureProperty {
    pub name: String,
    pub value: String,
}

/// The projection of a layer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum LayerProjection {
    /// A proj4 string.
    Proj4(String),
    /// The projection the layer's source declares, read when it opens. Where it declares none,
    /// its coordinates are longitude and latitude in the datum of the map's projection.
    FromSource,
}

/// How a layer's features are drawn, independent of their geometry type: polygons in a line
/// layer are drawn as their outlines.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum RenderMode {
    Point,
    Line,
    Polygon,
    Raster,
    /// Labels only, no geometry.
    Annotation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum LayerStatus {
    On,
    Off,
    /// Always drawn, whatever a request asks for.
    Default,
}

/// Where a layer's data lives, kept unparsed for a resolver to bind to an adapter.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct SourceRef {
    pub kind: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub props: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Class {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Which features the class draws; without one, every feature that reaches it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expression: Option<Expr>,
    /// The class draws at a scale from `min_scale_denom` up to below `max_scale_denom`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_scale_denom: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_scale_denom: Option<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<Label>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub styles: Vec<Style>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,
}

/// A style of a class, whose transform may be absent, or, as `Style<LabelTransform>`, of a label,
/// whose transform may not.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "schema", schemars(rename = "Style_{T}"))]
#[serde(bound(serialize = "T: Serialize + Transform"))]
pub struct Style<T = Option<GeomTransform>> {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Bound<Rgba>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline_color: Option<Bound<Rgba>>,
    /// Line width in pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<Bound<f64>>,
    /// Symbol size in pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<Bound<f64>>,
    /// Rotation in degrees, counter-clockwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub angle: Option<Bound<f64>>,
    /// Whether the symbol turns with the direction of the line it is drawn on, MapServer's
    /// `ANGLE AUTO`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub auto_angle: bool,
    /// The symbol drawn, by its name or MapServer's index into the symbol set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<Bound<SymbolRef>>,
    /// A shift of what is drawn, `[x, y]`, y downwards.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<[Bound<f64>; 2]>,
    /// A line drawn parallel to the feature's line at this distance, positive to its right in
    /// the direction of drawing on the image, as MapServer's offset curve places it; MapServer
    /// writes it as `OFFSET d -99`, or `-999` for both sides.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_offset: Option<LineOffset>,
    /// A shift by distance and angle in degrees, `[distance, angle]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polar_offset: Option<[Bound<f64>; 2]>,
    /// The distance between symbols drawn along a line; a negative one turns them with the line,
    /// a positive one keeps them upright.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap: Option<f64>,
    /// The distance before the first symbol or dash along a line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_gap: Option<f64>,
    /// Dash and gap lengths, alternating, 2 to 10 of them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pattern: Vec<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_cap: Option<LineCap>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_join: Option<LineJoin>,
    /// How far a miter join may reach, as a multiple of the width.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_join_max_size: Option<f64>,
    /// Opacity in percent, 0 to 100.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity: Option<Bound<f64>>,
    /// The width of a symbol's outline in pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline_width: Option<Bound<f64>>,
    /// The bounds of the scaled size and width, in pixels at the map's `def_resolution`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_size: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_size: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_width: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_width: Option<f64>,
    /// The geometry the style draws instead of the feature's own: on a class's style, the
    /// feature's own where it is absent; on a label's style, where the style draws about the
    /// label, never absent.
    #[serde(skip_serializing_if = "Transform::is_absent")]
    pub geom_transform: T,
    /// The style draws at a scale from `min_scale_denom` up to below `max_scale_denom`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_scale_denom: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_scale_denom: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,
}

impl<T> Style<T> {
    /// A style of the transform `geom_transform`, without any other property.
    pub fn new(geom_transform: T) -> Self {
        Style::<Option<GeomTransform>>::default().with_transform(geom_transform)
    }

    /// The style with the transform `transform` in place of its own.
    pub fn with_transform<U>(self, transform: U) -> Style<U> {
        let Style {
            color,
            outline_color,
            width,
            size,
            angle,
            auto_angle,
            symbol,
            offset,
            line_offset,
            polar_offset,
            gap,
            initial_gap,
            pattern,
            line_cap,
            line_join,
            line_join_max_size,
            opacity,
            outline_width,
            min_size,
            max_size,
            min_width,
            max_width,
            geom_transform: _,
            min_scale_denom,
            max_scale_denom,
            span,
        } = self;
        Style {
            color,
            outline_color,
            width,
            size,
            angle,
            auto_angle,
            symbol,
            offset,
            line_offset,
            polar_offset,
            gap,
            initial_gap,
            pattern,
            line_cap,
            line_join,
            line_join_max_size,
            opacity,
            outline_width,
            min_size,
            max_size,
            min_width,
            max_width,
            geom_transform: transform,
            min_scale_denom,
            max_scale_denom,
            span,
        }
    }

    /// The expressions among the style's properties.
    pub(crate) fn expressions(&self) -> Vec<&Expr> {
        let colors = [&self.color, &self.outline_color].into_iter().flatten();
        let numbers = [
            &self.width,
            &self.size,
            &self.angle,
            &self.opacity,
            &self.outline_width,
        ];
        let pairs = [&self.offset, &self.polar_offset]
            .into_iter()
            .flatten()
            .flatten();
        let numbers = numbers
            .into_iter()
            .flatten()
            .chain(pairs)
            .chain(self.line_offset.iter().map(|o| &o.distance));
        let symbol = self.symbol.iter().filter_map(Bound::expression);
        colors
            .filter_map(Bound::expression)
            .chain(numbers.filter_map(Bound::expression))
            .chain(symbol)
            .collect()
    }
}

impl Label {
    /// The expressions among the label's properties and its styles'.
    pub(crate) fn expressions(&self) -> Vec<&Expr> {
        let numbers = [&self.size, &self.angle, &self.priority]
            .into_iter()
            .flatten();
        let pairs = [&self.shadow_size, &self.offset]
            .into_iter()
            .flatten()
            .flatten();
        let colors = [&self.color, &self.outline_color];
        let mut found: Vec<&Expr> = self.text.iter().collect();
        found.extend(numbers.chain(pairs).filter_map(Bound::expression));
        found.extend(colors.into_iter().flatten().filter_map(Bound::expression));
        found.extend(self.font.iter().filter_map(Bound::expression));
        found.extend(self.styles.iter().flat_map(Style::expressions));
        found
    }
}

/// A symbol of the map's symbol set: by MapServer's index, in which 0 is the default symbol, or by
/// its name: the first symbol of that name, without regard to case, as MapServer's
/// `msGetSymbolIndex` finds it. A name no symbol carries draws nothing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(untagged)]
pub enum SymbolRef {
    Index(u32),
    Name(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct LineOffset {
    pub distance: Bound<f64>,
    /// Drawn on both sides of the line, MapServer's `-999`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub both_sides: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum LineCap {
    Butt,
    Round,
    Square,
    Triangle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum LineJoin {
    None,
    Round,
    Miter,
    Bevel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum GeomTransform {
    /// The first point of a line.
    Start,
    /// The last point of a line.
    End,
    /// Every point of a line but the first and the last.
    Vertices,
    /// The bounding box of the shape in pixels, each side held within the image grown by the
    /// style's width or size, whichever is larger, and 3 pixels, drawn as a polygon is.
    Bbox,
    Centroid,
    /// A label's transform, [`LabelTransform::Point`], which a class's style of a line or a
    /// polygon layer draws nothing at.
    #[serde(rename = "labelpoint")]
    LabelPoint,
    /// A label's transform, [`LabelTransform::Poly`], which a class's style of a line or a
    /// polygon layer draws nothing at.
    #[serde(rename = "labelpoly")]
    LabelPoly,
    /// A label's transform, [`LabelTransform::Center`], which a class's style of a line or a
    /// polygon layer draws nothing at.
    #[serde(rename = "labelcenter")]
    LabelCenter,
}

/// Where a label's style draws, which it always names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum LabelTransform {
    /// At the label's point: a marker, before the text.
    #[serde(rename = "labelpoint")]
    Point,
    /// The box around the label's text.
    #[serde(rename = "labelpoly")]
    Poly,
    /// The centre of the box around the label's text: a marker drawn there, before the text.
    #[serde(rename = "labelcenter")]
    Center,
}

/// The transform of a style, [`Style::geom_transform`]: whether it is absent, and so left out of
/// the document.
pub trait Transform {
    fn is_absent(&self) -> bool;
}

impl Transform for Option<GeomTransform> {
    fn is_absent(&self) -> bool {
        self.is_none()
    }
}

impl Transform for LabelTransform {
    fn is_absent(&self) -> bool {
        false
    }
}

/// A label a class draws for each feature.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Label {
    /// The text: the label's `TEXT`, else the class's, else the layer's `LABELITEM`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<Expr>,
    /// Whether the label is drawn for a feature: its `EXPRESSION`, read as a class's is, with the
    /// layer's `LABELITEM` as the attribute a string, regular expression, or list compares.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expression: Option<Expr>,
    /// A font of the map's fonts, by name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<Bound<String>>,
    /// The size in points; MapServer's bitmap sizes `tiny` to `giant` stand as 5, 7, 10, 13, 16.
    /// A bound value is rounded to an integer, as MapServer's `bindLabel` reads it with
    /// `bindIntegerAttribute`: `MS_NINT(atof(value))`, `lrint` at the pin, half to even, undefined
    /// past a `long`, and the `long` narrowed to an `int`. An empty value is 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<Bound<f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_size: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_size: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Bound<Rgba>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline_color: Option<Bound<Rgba>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline_width: Option<f64>,
    /// A colour only, never a binding or an expression: MapServer's `loadLabel` reads it with
    /// `loadColor` and no binding, and refuses a mapfile that binds it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow_color: Option<Rgba>,
    /// The shadow's offset, `[x, y]`. A bound value is rounded to an integer as a bound `size`
    /// is; an empty one is 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow_size: Option<[Bound<f64>; 2]>,
    /// A bound value is read as MapServer's `bindLabel` reads it: rounded to an integer as a
    /// bound `size` is, which where it is not 0 is taken as MapServer's position constant without
    /// a range check (`MS_UL` 101, `MS_LR`, `MS_UR`, `MS_LL`, `MS_CR`, `MS_CL`, `MS_UC`, `MS_LC`,
    /// `MS_CC`, `MS_AUTO` 110); where it is 0, a value of exactly two characters that is one of
    /// `ul` … `lr` and `cc`, compared without case. A value that reads as 0 and matches none, an
    /// empty one among them, leaves the position as it stood: the previous feature's, or on the
    /// first feature the one the load left, `MS_BINDING` (2007) where the binding is the last
    /// `POSITION`, else the last keyword. What is drawn there is not settled until the engine
    /// draws bound label values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<Bound<Position>>,
    /// `[x, y]`. A bound value is rounded to an integer as a bound `size` is; an empty one is 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<[Bound<f64>; 2]>,
    /// Degrees counter-clockwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub angle: Option<Bound<f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub angle_mode: Option<AngleMode>,
    /// A bound value is read as MapServer's `bindLabel` reads it: rounded to an integer as a
    /// bound `size` is, which where it is not 0 is taken as MapServer's alignment constant
    /// without a range check (`MS_ALIGN_LEFT` 1, `MS_ALIGN_CENTER` 2, `MS_ALIGN_RIGHT` 3); where
    /// it is 0, a value of at least four characters that begins with `left`, `cente`, or `right`,
    /// compared without case. A value that reads as 0 and matches none, an empty one among them,
    /// leaves the alignment as it stood: the previous feature's, or on the first feature the one
    /// the load left, a keyword `ALIGN` before or after the binding, else `MS_ALIGN_DEFAULT` 0.
    /// What is drawn there is not settled until the engine draws bound label values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align: Option<Bound<Align>>,
    /// The character at which the text wraps.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrap: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_length: Option<f64>,
    /// Space kept free around the label, in pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub buffer: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_distance: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeat_distance: Option<f64>,
    /// The smallest feature labelled, in pixels; `auto`: one the label fits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_feature_size: Option<MinFeatureSize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_overlap_angle: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partials: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub force: Option<Force>,
    /// 1 to 10; a higher one is placed first. A bound value is rounded to an integer as a bound
    /// `size` is; an empty one is 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<Bound<f64>>,
    /// The label is placed at a scale from `min_scale_denom` up to below `max_scale_denom`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_scale_denom: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_scale_denom: Option<f64>,
    /// Drawn with the label, each where its transform says.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub styles: Vec<Style<LabelTransform>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum Position {
    Ul,
    Uc,
    Ur,
    Cl,
    Cc,
    Cr,
    Ll,
    Lc,
    Lr,
    Auto,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum AngleMode {
    /// Along the line.
    Auto,
    /// Along the line, and never upside down.
    Auto2,
    /// Following the line's curve, character by character.
    Follow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum Align {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum MinFeatureSize {
    Auto,
    #[serde(untagged)]
    Pixels(f64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum Force {
    On,
    Off,
    /// The label and its styles are placed together or not at all.
    Group,
}

/// A unit of length.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum Units {
    Pixels,
    Inches,
    Feet,
    Miles,
    Meters,
    Kilometers,
    #[serde(rename = "nautical_miles")]
    NauticalMiles,
    /// Decimal degrees.
    Degrees,
}

/// A style property that may vary by feature: a value, a property bound as MapServer binds an
/// attribute, or an expression (ADR-0015). It is read as an untagged enum would read it, a value
/// before an expression, but an object by its keys, so that a key it does not know is named.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(untagged)]
pub enum Bound<T> {
    Value(T),
    /// The feature's property `binding`, read as MapServer's `bindStyle` reads an attribute: a
    /// number with C's `atof`, a label's integer properties as the property says (`bindLabel`), a
    /// colour as `#rrggbb`, `#rrggbbaa`, or three integers separated by spaces, each with C's
    /// `atoi`. An empty value leaves a number at the value `bindStyle` sets before it reads one,
    /// which for a size, an outline width, or an angle is not the value the property has without
    /// the binding (docs/semantics/binding.md), and a colour that does not read leaves no colour.
    Binding {
        binding: String,
    },
    Expr(Expr),
}

struct BoundVisitor<T>(std::marker::PhantomData<T>);

impl<'de, T: Deserialize<'de>> BoundVisitor<T> {
    /// The value, if `T` reads from `scalar`, else the expression `literal`.
    fn value<E: serde::de::Error>(
        scalar: impl serde::de::IntoDeserializer<'de, serde::de::value::Error>,
        literal: Expr,
    ) -> Result<Bound<T>, E> {
        Ok(T::deserialize(scalar.into_deserializer()).map_or(Bound::Expr(literal), Bound::Value))
    }
}

impl<'de, T: Deserialize<'de>> serde::de::Visitor<'de> for BoundVisitor<T> {
    type Value = Bound<T>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a value, a binding, or an expression")
    }

    fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<Bound<T>, E> {
        Self::value(v, Expr::Bool(v))
    }

    fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Bound<T>, E> {
        Self::value(v, Expr::Number(v as f64))
    }

    fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Bound<T>, E> {
        Self::value(v, Expr::Number(v as f64))
    }

    fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Bound<T>, E> {
        Self::value(v, Expr::Number(v))
    }

    fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Bound<T>, E> {
        Self::value(v.to_owned(), Expr::String(v.to_owned()))
    }

    fn visit_seq<A: serde::de::SeqAccess<'de>>(self, seq: A) -> Result<Bound<T>, A::Error> {
        let seq = serde::de::value::SeqAccessDeserializer::new(seq);
        Expr::deserialize(seq).map(Bound::Expr)
    }

    fn visit_map<A: serde::de::MapAccess<'de>>(self, map: A) -> Result<Bound<T>, A::Error> {
        let mut keys = Keys::read(map, true)?;
        match keys.binding.take() {
            Some(binding) => Ok(Bound::Binding { binding }),
            None => keys.expr("Bound").map(Bound::Expr),
        }
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Bound<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(BoundVisitor(std::marker::PhantomData))
    }
}

impl<T: Copy> Bound<T> {
    /// The value, if it does not vary by feature.
    pub fn value(&self) -> Option<T> {
        match self {
            Bound::Value(value) => Some(*value),
            _ => None,
        }
    }
}

impl<T> Bound<T> {
    /// The expression, if the property is one.
    pub fn expression(&self) -> Option<&Expr> {
        match self {
            Bound::Expr(expr) => Some(expr),
            _ => None,
        }
    }
}

/// A colour with straight alpha. It serializes as `#rrggbb`, or `#rrggbbaa` when not opaque.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const fn opaque(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Parses `#rrggbb` or `#rrggbbaa`, case-insensitive.
    pub fn from_hex(text: &str) -> Option<Self> {
        let hex = text.strip_prefix('#')?;
        if !(hex.len() == 6 || hex.len() == 8) || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let byte = |i: usize| {
            hex.get(i..i + 2)
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        };
        Some(Self {
            r: byte(0)?,
            g: byte(2)?,
            b: byte(4)?,
            a: if hex.len() == 8 { byte(6)? } else { 255 },
        })
    }
}

impl fmt::Display for Rgba {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.r, self.g, self.b)?;
        if self.a != 255 {
            write!(f, "{:02x}", self.a)?;
        }
        Ok(())
    }
}

impl Serialize for Rgba {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

#[cfg(feature = "schema")]
impl schemars::JsonSchema for Rgba {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Rgba".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "pattern": "^#[0-9a-fA-F]{6}([0-9a-fA-F]{2})?$"
        })
    }
}

impl<'de> Deserialize<'de> for Rgba {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Rgba::from_hex(&text).ok_or_else(|| {
            serde::de::Error::custom(format!(
                "a colour is `#rrggbb` or `#rrggbbaa`, found `{text}`"
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_parse_and_print_as_hex() {
        assert_eq!(Rgba::from_hex("#FF8000"), Some(Rgba::opaque(255, 128, 0)));
        assert_eq!(Rgba::opaque(255, 128, 0).to_string(), "#ff8000");
        let translucent = Rgba {
            a: 0x80,
            ..Rgba::opaque(0, 0, 255)
        };
        assert_eq!(translucent.to_string(), "#0000ff80");
        assert_eq!(Rgba::from_hex("#0000ff80"), Some(translucent));
    }

    #[test]
    fn malformed_colours_are_rejected() {
        for text in ["ff8000", "#ff80", "#gg8000", "#ff8000f", ""] {
            assert_eq!(Rgba::from_hex(text), None, "{text}");
        }
    }
}
