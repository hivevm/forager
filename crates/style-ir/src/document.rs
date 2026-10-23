use serde::{Deserialize, Serialize};

use crate::{Diagnostic, Map};

/// The format version this build writes, `MAJOR.MINOR`. It is independent of the crate version.
pub const FORMAT_VERSION: &str = "1.0";

/// The style IR as a document: a format version and the map (ADR-0014).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StyleDocument {
    pub version: String,
    pub map: Map,
}

impl StyleDocument {
    /// A document of the current format version.
    pub fn new(map: Map) -> Self {
        Self {
            version: FORMAT_VERSION.to_owned(),
            map,
        }
    }

    pub fn to_yaml(&self) -> Result<String, Diagnostic> {
        serde_saphyr::to_string(self)
            .map_err(|e| Diagnostic::error("yaml-write", e.to_string(), None))
    }

    pub fn to_json(&self) -> Result<String, Diagnostic> {
        serde_json::to_string_pretty(self)
            .map_err(|e| Diagnostic::error("json-write", e.to_string(), None))
    }

    pub fn from_yaml(text: &str) -> Result<Self, Diagnostic> {
        let document: Self = serde_saphyr::from_str(text)
            .map_err(|e| Diagnostic::error("yaml-read", e.to_string(), None))?;
        document.checked()
    }

    pub fn from_json(text: &str) -> Result<Self, Diagnostic> {
        let document: Self = serde_json::from_str(text)
            .map_err(|e| Diagnostic::error("json-read", e.to_string(), None))?;
        document.checked()
    }

    fn checked(self) -> Result<Self, Diagnostic> {
        let major = |v: &str| v.split('.').next().and_then(|m| m.parse::<u32>().ok());
        match (major(&self.version), major(FORMAT_VERSION)) {
            (Some(theirs), Some(ours)) if theirs == ours => Ok(self),
            (Some(theirs), Some(ours)) if theirs > ours => Err(Diagnostic::error(
                "format-version-unsupported",
                format!(
                    "format version {} is newer than {FORMAT_VERSION}, the newest this build reads",
                    self.version
                ),
                None,
            )),
            _ => Err(Diagnostic::error(
                "format-version-invalid",
                format!(
                    "`{}` is not a format version this build reads",
                    self.version
                ),
                None,
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Class, Extent, Layer, LayerStatus, RenderMode, Rgba, SourceRef, Span, Style};

    fn sample() -> StyleDocument {
        StyleDocument::new(Map {
            name: Some("world".into()),
            extent: Some(Extent {
                min_x: -180.0,
                min_y: -90.0,
                max_x: 180.0,
                max_y: 90.0,
            }),
            size: Some([400, 200]),
            image_color: Some(Rgba::opaque(255, 255, 255)),
            projection: Some("+proj=longlat +datum=WGS84".into()),
            layers: vec![Layer {
                name: Some("countries".into()),
                render_mode: Some(RenderMode::Polygon),
                status: Some(LayerStatus::On),
                source: Some(SourceRef {
                    kind: "local".into(),
                    props: [("data".to_owned(), "countries.shp".to_owned())].into(),
                    span: Some(Span::new("world.map", 9, 5)),
                }),
                classes: vec![Class {
                    name: Some("all".into()),
                    styles: vec![Style {
                        color: Some(Rgba::opaque(200, 220, 180)),
                        outline_color: Some(Rgba::opaque(0, 0, 0)),
                        width: Some(0.5),
                        size: None,
                        span: Some(Span::new("world.map", 14, 9)),
                    }],
                    span: Some(Span::new("world.map", 12, 7)),
                }],
                span: Some(Span::new("world.map", 6, 3)),
            }],
            span: Some(Span::new("world.map", 1, 1)),
        })
    }

    #[test]
    fn a_document_reads_back_equal_from_yaml_and_from_json() {
        // Verifies: ADR-0014
        let document = sample();
        let from_yaml = StyleDocument::from_yaml(&document.to_yaml().unwrap()).unwrap();
        let from_json = StyleDocument::from_json(&document.to_json().unwrap()).unwrap();
        assert_eq!(from_yaml, document);
        assert_eq!(from_json, document);
    }

    #[test]
    fn a_newer_major_version_is_a_load_error() {
        let error = StyleDocument::from_json(r#"{"version": "2.0", "map": {}}"#).unwrap_err();
        assert_eq!(error.code, "format-version-unsupported");
    }

    #[test]
    fn a_version_that_is_not_a_number_is_a_load_error() {
        let error = StyleDocument::from_yaml("version: abc\nmap: {}\n").unwrap_err();
        assert_eq!(error.code, "format-version-invalid");
    }

    #[test]
    fn malformed_input_is_a_diagnostic_not_a_panic() {
        assert_eq!(
            StyleDocument::from_yaml("map: [").unwrap_err().code,
            "yaml-read"
        );
        assert_eq!(StyleDocument::from_json("{").unwrap_err().code, "json-read");
    }
}
