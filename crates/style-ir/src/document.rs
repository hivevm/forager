use serde::{Deserialize, Serialize};

use crate::{Diagnostic, MAX_EXPRESSION_DEPTH, Map};

/// The format version this build writes, `MAJOR.MINOR`. It is independent of the crate version.
pub const FORMAT_VERSION: &str = "1.0";

/// The style IR as a document: a format version and the map (ADR-0014).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
            (Some(theirs), Some(ours)) if theirs == ours => self.bounded(),
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

impl StyleDocument {
    /// Refuses a document with an expression nested deeper than [`MAX_EXPRESSION_DEPTH`] (Q-9).
    fn bounded(self) -> Result<Self, Diagnostic> {
        let layers = self.map.layers.iter();
        let expressions = layers.flat_map(|layer| {
            let classes = layer.classes.iter().flat_map(|class| {
                let styles = class.styles.iter().flat_map(|style| style.expressions());
                let labels = class.labels.iter().flat_map(|label| label.expressions());
                class.expression.iter().chain(styles).chain(labels)
            });
            layer.filter.iter().chain(classes)
        });
        for expression in expressions {
            if expression.depth() > MAX_EXPRESSION_DEPTH {
                let message = format!("an expression nests deeper than {MAX_EXPRESSION_DEPTH}");
                return Err(Diagnostic::error("expression-too-deep", message, None));
            }
        }
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Align, AngleMode, Bound, Class, Expr, Extent, Force, GeomTransform, Label, Layer,
        LayerStatus, LineCap, LineJoin, LineOffset, MinFeatureSize, Op, Position, RenderMode, Rgba,
        SourceRef, Span, Style, Symbol, SymbolKind, SymbolRef, Units,
    };

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
            resolution: Some(96.0),
            def_resolution: None,
            layers: vec![Layer {
                name: Some("countries".into()),
                render_mode: Some(RenderMode::Polygon),
                status: Some(LayerStatus::On),
                source: Some(SourceRef {
                    kind: "local".into(),
                    props: [("data".to_owned(), "countries.shp".to_owned())].into(),
                    span: Some(Span::new("world.map", 9, 5)),
                }),
                size_units: Some(Units::Meters),
                filter: Some(Expr::call(
                    Op::In,
                    vec![
                        Expr::call(Op::ToString, vec![Expr::property("iso")]),
                        Expr::List(vec!["AT".into(), "1000".into(), "true".into()]),
                    ],
                )),
                classes: vec![Class {
                    name: Some("all".into()),
                    expression: Some(Expr::call(
                        Op::And,
                        vec![
                            Expr::call(
                                Op::Ge,
                                vec![
                                    Expr::call(Op::ToNumber, vec![Expr::property("pop")]),
                                    Expr::Number(27374987.048899982),
                                ],
                            ),
                            Expr::call(
                                Op::TAfter,
                                vec![
                                    Expr::call(Op::ToTimestamp, vec![Expr::property("t")]),
                                    Expr::Date {
                                        date: "2010-12-01".into(),
                                    },
                                ],
                            ),
                            Expr::Bool(false),
                        ],
                    )),
                    styles: vec![sample_style()],
                    labels: vec![sample_label()],
                    span: Some(Span::new("world.map", 12, 7)),
                }],
                span: Some(Span::new("world.map", 6, 3)),
            }],
            symbols: vec![sample_symbol()],
            fonts: [("sans".to_owned(), "fonts/DejaVuSans.ttf".to_owned())].into(),
            span: Some(Span::new("world.map", 1, 1)),
        })
    }

    fn sample_style() -> Style {
        Style {
            color: Some(Bound::Value(Rgba::opaque(200, 220, 180))),
            outline_color: Some(Bound::Binding {
                binding: "rim".into(),
            }),
            width: Some(Bound::Value(0.5)),
            size: Some(Bound::Binding {
                binding: "VAL".into(),
            }),
            angle: Some(Bound::Expr(Expr::call(
                Op::ToNumber,
                vec![Expr::call(
                    Op::Tostring,
                    vec![
                        Expr::call(Op::ToNumber, vec![Expr::property("a")]),
                        "%g".into(),
                    ],
                )],
            ))),
            auto_angle: true,
            symbol: Some(Bound::Value(SymbolRef::Name("1".into()))),
            offset: Some([
                Bound::Value(1.0),
                Bound::Binding {
                    binding: "dy".into(),
                },
            ]),
            line_offset: Some(LineOffset {
                distance: Bound::Value(3.0),
                both_sides: true,
            }),
            pattern: vec![10.0, 5.0],
            line_cap: Some(LineCap::Butt),
            line_join: Some(LineJoin::Miter),
            opacity: Some(Bound::Value(50.0)),
            max_width: Some(8.0),
            geom_transform: Some(GeomTransform::Vertices),
            span: Some(Span::new("world.map", 14, 9)),
            ..Style::default()
        }
    }

    fn sample_symbol() -> Symbol {
        Symbol {
            name: Some("arrow".into()),
            kind: SymbolKind::Vector,
            paths: vec![vec![[0.0, 0.0], [2.0, 4.0]], vec![[1.0, 0.0]]],
            extent: Some([2.0, 4.0]),
            filled: true,
            anchor_point: Some([0.5, 1.0]),
            character: None,
            font: None,
            image: None,
            transparent: None,
            span: None,
        }
    }

    fn sample_label() -> Label {
        Label {
            text: Some(Expr::call(
                Op::Concat,
                vec![
                    Expr::call(Op::ToString, vec![Expr::property("name")]),
                    " (".into(),
                ],
            )),
            font: Some(Bound::Value("sans".into())),
            size: Some(Bound::Binding {
                binding: "s".into(),
            }),
            position: Some(Bound::Value(Position::Auto)),
            angle_mode: Some(AngleMode::Follow),
            align: Some(Bound::Value(Align::Center)),
            wrap: Some(" ".into()),
            min_feature_size: Some(MinFeatureSize::Pixels(8.0)),
            force: Some(Force::Group),
            priority: Some(Bound::Value(5.0)),
            styles: vec![Style {
                geom_transform: Some(GeomTransform::LabelPoly),
                ..Style::default()
            }],
            ..Label::default()
        }
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
    fn numbers_read_back_from_json_bit_for_bit() {
        // Verifies: ADR-0014
        // The first of 15,585 in 200,000 coordinate-like decimals that serde_json's default
        // float parser read back one unit in the last place off.
        let x = 27374987.048899982_f64;
        let document = StyleDocument::new(Map {
            extent: Some(Extent {
                min_x: x,
                min_y: -x,
                max_x: x,
                max_y: x,
            }),
            ..Map::default()
        });
        let read = StyleDocument::from_json(&document.to_json().unwrap()).unwrap();
        let extent = read.map.extent.unwrap();
        assert_eq!(extent.min_x.to_bits(), x.to_bits());
        assert_eq!(extent.min_y.to_bits(), (-x).to_bits());
    }

    #[test]
    fn an_expression_past_the_nesting_limit_is_refused_and_one_at_it_reads() {
        // Verifies: Q-9, ADR-0019
        let nested = |depth: usize| {
            let mut expr = Expr::Bool(true);
            for _ in 1..depth {
                expr = Expr::call(Op::Not, vec![expr]);
            }
            let mut document = sample();
            document.map.layers[0].filter = Some(expr);
            document
        };
        let at_limit = nested(MAX_EXPRESSION_DEPTH);
        let yaml = at_limit.to_yaml().unwrap();
        assert_eq!(StyleDocument::from_yaml(&yaml).unwrap(), at_limit);
        let json = at_limit.to_json().unwrap();
        assert_eq!(StyleDocument::from_json(&json).unwrap(), at_limit);

        let past = nested(MAX_EXPRESSION_DEPTH + 1);
        let error = StyleDocument::from_json(&past.to_json().unwrap()).unwrap_err();
        assert_eq!(error.code, "expression-too-deep");
        let error = StyleDocument::from_yaml(&past.to_yaml().unwrap()).unwrap_err();
        assert_eq!(error.code, "expression-too-deep");
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
