use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{Expr, Span};

/// A map: its image, its extent, and its layers in drawing order.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
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
    /// The resolution of the image in dpi; MapServer's default is 72.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution: Option<f64>,
    /// The resolution the map's pixel lengths are written for, in dpi; MapServer's default is 72.
    /// Every length in pixels is drawn scaled by `resolution` over this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub def_resolution: Option<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layers: Vec<Layer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Extent {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub classes: Vec<Class>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,
}

/// How a layer's features are drawn, independent of their geometry type: polygons in a line
/// layer are drawn as their outlines.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
#[serde(rename_all = "lowercase")]
pub enum LayerStatus {
    On,
    Off,
    /// Always drawn, whatever a request asks for.
    Default,
}

/// Where a layer's data lives, kept unparsed for a resolver to bind to an adapter.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SourceRef {
    pub kind: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub props: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Class {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Which features the class draws; without one, every feature that reaches it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expression: Option<Expr>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub styles: Vec<Style>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Style {
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
    /// A line drawn parallel to the feature's line at this distance, positive to its left in
    /// the direction of drawing; MapServer writes it as `OFFSET d -99`, or `-999` for both sides.
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
    /// The geometry the style draws instead of the feature's own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geom_transform: Option<GeomTransform>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,
}

impl Style {
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

/// A symbol of the map's symbol set: by its name, or by MapServer's index, in which 0 is the
/// default symbol.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SymbolRef {
    Index(u32),
    Name(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LineOffset {
    pub distance: Bound<f64>,
    /// Drawn on both sides of the line, MapServer's `-999`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub both_sides: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LineCap {
    Butt,
    Round,
    Square,
    Triangle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LineJoin {
    None,
    Round,
    Miter,
    Bevel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GeomTransform {
    /// The first point of a line.
    Start,
    /// The last point of a line.
    End,
    /// Every point of a line but the first and the last.
    Vertices,
    Centroid,
}

/// A unit of length.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
/// attribute, or an expression (ADR-0019).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Bound<T> {
    Value(T),
    /// The feature's property `binding`, read as MapServer's `bindStyle` reads an attribute: a
    /// number with C's `atof`, a colour as `#rrggbb`, `#rrggbbaa`, or three integers separated
    /// by spaces. An empty value leaves a number at its default, and a colour that does not read
    /// leaves no colour.
    Binding {
        binding: String,
    },
    Expr(Expr),
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
        let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
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
