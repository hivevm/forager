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
    pub color: Option<Rgba>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline_color: Option<Rgba>,
    /// Line width in pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    /// Symbol size in pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,
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
