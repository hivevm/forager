use serde::{Deserialize, Serialize};

use crate::migration::{self, STEPS};
use crate::{Diagnostic, MAX_EXPRESSION_DEPTH, Map};

/// The format version this build writes, `MAJOR.MINOR`. It is independent of the crate version.
pub const FORMAT_VERSION: &str = "1.0";

/// A style document as read, with the warnings reading it raised.
#[derive(Debug)]
pub struct Read {
    pub document: StyleDocument,
    pub warnings: Vec<Diagnostic>,
}

/// The major and minor of a format version `MAJOR.MINOR`; a bare `MAJOR` has the minor 0.
fn version(text: &str) -> Option<(u32, u32)> {
    let (major, minor) = text.split_once('.').unwrap_or((text, "0"));
    Some((major.parse().ok()?, minor.parse().ok()?))
}

/// The major of a document's version where it is older than this build's, read before the
/// document itself, whose shape that version decides.
fn older_major(document: Option<Versioned>) -> Option<(String, u32)> {
    let theirs = document?.version;
    let (major, _) = version(&theirs)?;
    let (current, _) = version(FORMAT_VERSION)?;
    (major < current).then_some((theirs, major))
}

/// A document's version alone.
#[derive(Deserialize)]
struct Versioned {
    version: String,
}

/// Whether a document of `version` is of a newer minor of this build's major.
fn newer_minor(version_text: Option<&str>) -> bool {
    match (version_text.and_then(version), version(FORMAT_VERSION)) {
        (Some(theirs), Some(ours)) => theirs.0 == ours.0 && theirs.1 > ours.1,
        _ => false,
    }
}

/// How many unknown values a newer minor's document may drop before it is refused: each one is
/// found by reading the document again (Q-9).
pub const MAX_DROPPED_VALUES: usize = 100;

/// A step of the path to a place in a document.
enum Segment {
    Key(String),
    Index(usize),
}

/// The segments of the path at which a typed read failed.
fn segments(path: &serde_path_to_error::Path) -> Vec<Segment> {
    path.iter()
        .filter_map(|segment| match segment {
            serde_path_to_error::Segment::Map { key } => Some(Segment::Key(key.clone())),
            serde_path_to_error::Segment::Seq { index } => Some(Segment::Index(*index)),
            _ => None,
        })
        .collect()
}

/// The JSON Pointer (RFC 6901) of `segments`.
fn segments_pointer(segments: &[Segment]) -> String {
    segments
        .iter()
        .map(|step| match step {
            Segment::Key(key) => format!("/{}", key.replace('~', "~0").replace('/', "~1")),
            Segment::Index(index) => format!("/{index}"),
        })
        .collect()
}

/// A read error, prefixed with the JSON Pointer of where it happened.
fn read_error<E: std::fmt::Display>(
    code: &str,
    error: &serde_path_to_error::Error<E>,
) -> Diagnostic {
    let at = segments_pointer(&segments(error.path()));
    let message = if at.is_empty() {
        error.inner().to_string()
    } else {
        format!("{at}: {}", error.inner())
    };
    Diagnostic::error(code, message, None)
}

/// The first operator in `value` that this build does not know.
fn unknown_operator(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Object(object) => {
            if let Some(serde_json::Value::String(op)) = object.get("op")
                && serde_json::from_value::<crate::Op>(op.clone().into()).is_err()
            {
                return Some(op.clone());
            }
            object.values().find_map(unknown_operator)
        }
        serde_json::Value::Array(items) => items.iter().find_map(unknown_operator),
        _ => None,
    }
}

/// The JSON Pointer (RFC 6901) of a place in a document.
fn pointer(path: &serde_ignored::Path) -> String {
    match path {
        serde_ignored::Path::Root => String::new(),
        serde_ignored::Path::Seq { parent, index } => format!("{}/{index}", pointer(parent)),
        serde_ignored::Path::Map { parent, key } => {
            let key = key.replace('~', "~0").replace('/', "~1");
            format!("{}/{key}", pointer(parent))
        }
        serde_ignored::Path::Some { parent }
        | serde_ignored::Path::NewtypeStruct { parent }
        | serde_ignored::Path::NewtypeVariant { parent } => pointer(parent),
    }
}

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

    /// Reads a document from YAML, with the warnings of reading it (ADR-0020).
    pub fn from_yaml(text: &str) -> Result<Read, Diagnostic> {
        Self::from_yaml_with(text, STEPS)
    }

    /// Reads a document from JSON, with the warnings of reading it (ADR-0020).
    pub fn from_json(text: &str) -> Result<Read, Diagnostic> {
        Self::from_json_with(text, STEPS)
    }

    /// [`StyleDocument::from_yaml`] with the migration steps `steps`, for the tests and the fuzz
    /// target that exercise the chain before a second major exists.
    #[doc(hidden)]
    pub fn from_yaml_with(text: &str, steps: &[migration::Step]) -> Result<Read, Diagnostic> {
        let versioned: Option<Versioned> = serde_saphyr::from_str(text).ok();
        let lenient = newer_minor(versioned.as_ref().map(|v| v.version.as_str()));
        if let Some((theirs, major)) = older_major(versioned) {
            let value = serde_saphyr::from_str(text)
                .map_err(|e| Diagnostic::error("yaml-read", e.to_string(), None))?;
            return Self::migrated(value, &theirs, major, steps);
        }
        let mut ignored = Vec::new();
        let mut track = serde_path_to_error::Track::new();
        let read = serde_saphyr::with_deserializer_from_str(text, |de| {
            let de = serde_path_to_error::Deserializer::new(de, &mut track);
            serde_ignored::deserialize(de, |path| ignored.push(pointer(&path)))
        });
        match read {
            Ok(document) => Self::checked(document, ignored),
            Err(_) if lenient => {
                let value = serde_saphyr::from_str(text)
                    .map_err(|e| Diagnostic::error("yaml-read", e.to_string(), None))?;
                Self::lenient(value, "yaml-read")
            }
            Err(e) => Err(read_error(
                "yaml-read",
                &serde_path_to_error::Error::new(track.path(), e),
            )),
        }
    }

    /// [`StyleDocument::from_json`] with the migration steps `steps`.
    #[doc(hidden)]
    pub fn from_json_with(text: &str, steps: &[migration::Step]) -> Result<Read, Diagnostic> {
        let versioned: Option<Versioned> = serde_json::from_str(text).ok();
        let lenient = newer_minor(versioned.as_ref().map(|v| v.version.as_str()));
        if let Some((theirs, major)) = older_major(versioned) {
            let value = serde_json::from_str(text)
                .map_err(|e| Diagnostic::error("json-read", e.to_string(), None))?;
            return Self::migrated(value, &theirs, major, steps);
        }
        let mut ignored = Vec::new();
        let mut track = serde_path_to_error::Track::new();
        let mut deserializer = serde_json::Deserializer::from_str(text);
        let read = serde_ignored::deserialize(
            serde_path_to_error::Deserializer::new(&mut deserializer, &mut track),
            |path| ignored.push(pointer(&path)),
        )
        .and_then(|document: Self| deserializer.end().map(|()| document));
        match read {
            Ok(document) => Self::checked(document, ignored),
            Err(_) if lenient => {
                let value = serde_json::from_str(text)
                    .map_err(|e| Diagnostic::error("json-read", e.to_string(), None))?;
                Self::lenient(value, "json-read")
            }
            Err(e) => Err(read_error(
                "json-read",
                &serde_path_to_error::Error::new(track.path(), e),
            )),
        }
    }

    /// Reads `value`, a document of a newer minor that does not read as this build's version,
    /// dropping each value this build does not know with a warning naming it and its path: an
    /// enum value unsets its field, and an expression with an unknown operator or function
    /// becomes one that never matches, so that its class or filter is kept (ADR-0019, ADR-0020).
    /// Anything else that does not read is the error `code`, naming its path.
    fn lenient(mut value: serde_json::Value, code: &str) -> Result<Read, Diagnostic> {
        let mut dropped = Vec::new();
        loop {
            let mut ignored = Vec::new();
            let read = serde_path_to_error::deserialize(serde_ignored::Deserializer::new(
                value.clone(),
                &mut |path: serde_ignored::Path| ignored.push(pointer(&path)),
            ));
            let error = match read {
                Ok(document) => {
                    let mut read = Self::checked(document, ignored)?;
                    dropped.append(&mut read.warnings);
                    read.warnings = dropped;
                    return Ok(read);
                }
                Err(error) => error,
            };
            let path = segments(error.path());
            // The field that holds what did not read: the nearest key on its path.
            let Some(key_at) = path.iter().rposition(|s| matches!(s, Segment::Key(_))) else {
                return Err(read_error(code, &error));
            };
            let at = segments_pointer(&path[..=key_at]);
            let Some(field) = value.pointer_mut(&at) else {
                return Err(read_error(code, &error));
            };
            let warning = if let Some(op) = unknown_operator(field) {
                let filter =
                    matches!(&path[key_at], Segment::Key(k) if k == "expression" || k == "filter");
                if filter {
                    *field = serde_json::Value::Bool(false);
                    let message = format!(
                        "{at}: the operator `{op}` is unknown to format version {FORMAT_VERSION}, \
                         and the expression never matches"
                    );
                    Diagnostic::warning("unknown-operator", message, None)
                } else {
                    let message = format!(
                        "{at}: the operator `{op}` is unknown to format version {FORMAT_VERSION}, \
                         and the field is ignored"
                    );
                    Self::remove(&mut value, &at);
                    Diagnostic::warning("unknown-operator", message, None)
                }
            } else if error.inner().to_string().starts_with("unknown variant")
                || error.inner().to_string().contains("untagged enum")
            {
                let shown = match field {
                    serde_json::Value::String(text) => text.clone(),
                    other => other.to_string(),
                };
                let message = format!(
                    "{at}: `{shown}` is unknown to format version {FORMAT_VERSION} and is ignored"
                );
                Self::remove(&mut value, &at);
                Diagnostic::warning("unknown-value", message, None)
            } else {
                return Err(read_error(code, &error));
            };
            if dropped.len() == MAX_DROPPED_VALUES {
                let message = format!(
                    "the document holds more than {MAX_DROPPED_VALUES} values unknown to format \
                     version {FORMAT_VERSION}"
                );
                return Err(Diagnostic::error("too-many-unknown-values", message, None));
            }
            dropped.push(warning);
        }
    }

    /// Removes the field at the JSON Pointer `at` from its object.
    fn remove(value: &mut serde_json::Value, at: &str) {
        let (parent, key) = at.rsplit_once('/').unwrap_or(("", at));
        let key = key.replace("~1", "/").replace("~0", "~");
        if let Some(serde_json::Value::Object(object)) = value.pointer_mut(parent) {
            object.remove(&key);
        }
    }

    /// Reads `value`, a document of the older format version `theirs` with the major `major`,
    /// migrated to the current major and then read as that version is, with a warning naming the
    /// version it came from.
    fn migrated(
        value: serde_json::Value,
        theirs: &str,
        major: u32,
        steps: &[migration::Step],
    ) -> Result<Read, Diagnostic> {
        let current = version(FORMAT_VERSION).map_or(major, |(current, _)| current);
        let value = migration::migrate(value, theirs, major, current, steps)?;
        let mut ignored = Vec::new();
        let document: Self = serde_ignored::deserialize(value, |path| ignored.push(pointer(&path)))
            .map_err(|e| {
                let message = format!("format version {theirs}, migrated: {e}");
                Diagnostic::error("format-migration-failed", message, None)
            })?;
        // What the steps leave unknown, the document's own version did not know either.
        if let Some(field) = ignored.first() {
            let message = format!(
                "format version {theirs} has no field {field}; this build reads {FORMAT_VERSION}"
            );
            return Err(Diagnostic::error("unknown-field", message, None));
        }
        let mut read = document.checked(ignored)?;
        let message =
            format!("the document was migrated from format version {theirs} to {FORMAT_VERSION}");
        read.warnings
            .insert(0, Diagnostic::warning("format-migrated", message, None));
        Ok(read)
    }

    /// Checks the format version, and decides on the fields the reader did not know, `ignored`
    /// by their JSON Pointer: a newer minor than this build's may have them, each a warning, and
    /// its document is read without them; in the current or an older minor, no version this
    /// build knows has them, so the first is a load error.
    fn checked(self, ignored: Vec<String>) -> Result<Read, Diagnostic> {
        match (version(&self.version), version(FORMAT_VERSION)) {
            (Some(theirs), Some(ours)) if theirs.0 == ours.0 => {
                let newer_minor = theirs.1 > ours.1;
                if !newer_minor && let Some(field) = ignored.first() {
                    let message = format!(
                        "format version {} has no field {field}; this build reads {FORMAT_VERSION}",
                        self.version
                    );
                    return Err(Diagnostic::error("unknown-field", message, None));
                }
                let warnings = ignored
                    .into_iter()
                    .map(|field| {
                        let message = format!(
                            "{field} is unknown to format version {FORMAT_VERSION} and is ignored"
                        );
                        Diagnostic::warning("unknown-field", message, None)
                    })
                    .collect();
                Ok(Read {
                    document: self.bounded()?,
                    warnings,
                })
            }
            (Some(theirs), Some(ours)) if theirs.0 > ours.0 => Err(Diagnostic::error(
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
    use crate::Step;
    use crate::{
        Align, AngleMode, Bound, Class, Expr, Extent, Force, GeomTransform, Label, Layer,
        LayerProjection, LayerStatus, LineCap, LineJoin, LineOffset, MinFeatureSize, Op, Position,
        RenderMode, Rgba, SourceRef, Span, Style, Symbol, SymbolKind, SymbolRef, Units,
    };
    use serde_json::Value;

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
            units: Some(Units::Degrees),
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
                symbol_scale_denom: Some(25000.0),
                projection: Some(LayerProjection::FromSource),
                min_scale_denom: Some(1000.0),
                max_scale_denom: None,
                filter: Some(Expr::call(
                    Op::In,
                    vec![
                        Expr::call(Op::ToString, vec![Expr::property("iso")]),
                        Expr::List(vec!["AT".into(), "1000".into(), "true".into()]),
                    ],
                )),
                classes: vec![Class {
                    name: Some("all".into()),
                    min_scale_denom: None,
                    max_scale_denom: Some(50000.0),
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
        let from_yaml = StyleDocument::from_yaml(&document.to_yaml().unwrap())
            .unwrap()
            .document;
        let from_json = StyleDocument::from_json(&document.to_json().unwrap())
            .unwrap()
            .document;
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
        let read = StyleDocument::from_json(&document.to_json().unwrap())
            .unwrap()
            .document;
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
        assert_eq!(StyleDocument::from_yaml(&yaml).unwrap().document, at_limit);
        let json = at_limit.to_json().unwrap();
        assert_eq!(StyleDocument::from_json(&json).unwrap().document, at_limit);

        let past = nested(MAX_EXPRESSION_DEPTH + 1);
        let error = StyleDocument::from_json(&past.to_json().unwrap()).unwrap_err();
        assert_eq!(error.code, "expression-too-deep");
        let error = StyleDocument::from_yaml(&past.to_yaml().unwrap()).unwrap_err();
        assert_eq!(error.code, "expression-too-deep");
    }

    #[test]
    fn a_newer_minors_unknown_fields_are_named_by_their_path() {
        // Verifies: ADR-0020
        let json = r#"{"version":"1.9","map":{"layers":[{"name":"a","glow":1}],"a/b~":2}}"#;
        let yaml = "version: \"1.9\"\nmap:\n  layers:\n  - name: a\n    glow: 1\n  a/b~: 2\n";
        for read in [
            StyleDocument::from_json(json),
            StyleDocument::from_yaml(yaml),
        ] {
            let read = read.unwrap();
            assert_eq!(read.document.map.layers[0].name.as_deref(), Some("a"));
            let messages: Vec<_> = read.warnings.iter().map(|w| w.message.as_str()).collect();
            assert_eq!(
                messages,
                [
                    "/map/layers/0/glow is unknown to format version 1.0 and is ignored",
                    "/map/a~1b~0 is unknown to format version 1.0 and is ignored",
                ]
            );
            assert!(read.warnings.iter().all(|w| w.code == "unknown-field"));
        }
    }

    #[test]
    fn an_unknown_field_in_the_current_or_an_older_minor_is_a_load_error() {
        // Verifies: ADR-0020
        for version in ["1.0", "1"] {
            let json = format!(r#"{{"version":"{version}","map":{{"layers":[{{"glow":1}}]}}}}"#);
            let error = StyleDocument::from_json(&json).unwrap_err();
            assert_eq!(error.code, "unknown-field");
            assert!(
                error.message.contains("/map/layers/0/glow"),
                "{}",
                error.message
            );
        }
        let read = StyleDocument::from_yaml("version: \"1.0\"\nmap: {}\n").unwrap();
        assert!(read.warnings.is_empty());
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

    /// A fixture step from the test-only major 0: `colour` became `image_color`.
    fn rename_colour(mut value: Value) -> Result<Value, String> {
        let map = value
            .get_mut("map")
            .and_then(Value::as_object_mut)
            .ok_or("a document of major 0 has a map")?;
        if let Some(colour) = map.remove("colour") {
            map.insert("image_color".to_owned(), colour);
        }
        Ok(value)
    }

    const FIXTURE: &[Step] = &[Step {
        from_major: 0,
        migrate: rename_colour,
    }];

    #[test]
    fn an_older_major_is_migrated_step_by_step() {
        // Verifies: ADR-0020
        let json = r##"{"version":"0.3","map":{"name":"m","colour":"#010203"}}"##;
        let yaml = "version: \"0.3\"\nmap:\n  name: m\n  colour: \"#010203\"\n";
        for read in [
            StyleDocument::from_json_with(json, FIXTURE),
            StyleDocument::from_yaml_with(yaml, FIXTURE),
        ] {
            let read = read.unwrap();
            assert_eq!(read.document.version, FORMAT_VERSION);
            assert_eq!(read.document.map.image_color, Some(Rgba::opaque(1, 2, 3)));
            let messages: Vec<_> = read.warnings.iter().map(|w| w.message.as_str()).collect();
            assert_eq!(
                messages,
                ["the document was migrated from format version 0.3 to 1.0"]
            );
        }
    }

    #[test]
    fn an_older_major_without_a_step_is_a_load_error() {
        let json = r#"{"version":"0.3","map":{}}"#;
        let error = StyleDocument::from_json(json).unwrap_err();
        assert_eq!(error.code, "format-version-unsupported");
        assert!(error.message.contains("0.3"), "{}", error.message);
    }

    #[test]
    fn a_failing_step_is_a_load_error() {
        let error =
            StyleDocument::from_json_with(r#"{"version":"0.1","map":[]}"#, FIXTURE).unwrap_err();
        assert_eq!(error.code, "format-migration-failed");
        // What a step leaves unknown to the current version is the old document's own error.
        let json = r#"{"version":"0.1","map":{"glow":1}}"#;
        let error = StyleDocument::from_json_with(json, FIXTURE).unwrap_err();
        assert_eq!(error.code, "unknown-field");
        assert_eq!(
            error.message,
            "format version 0.1 has no field /map/glow; this build reads 1.0"
        );
    }

    #[test]
    fn newer_minors_are_lenient_newer_majors_refused_older_majors_migrated() {
        // Verifies: G-4
        let newer_minor = r#"{"version":"1.9","map":{"name":"m","glow":1}}"#;
        let read = StyleDocument::from_json(newer_minor).unwrap();
        assert_eq!(read.document.map.name.as_deref(), Some("m"));
        assert_eq!(
            read.warnings[0].message,
            "/map/glow is unknown to format version 1.0 and is ignored"
        );
        let newer_major = r#"{"version":"2.0","map":{}}"#;
        let error = StyleDocument::from_json(newer_major).unwrap_err();
        assert_eq!(error.code, "format-version-unsupported");
        let older_major = r##"{"version":"0.3","map":{"colour":"#010203"}}"##;
        let read = StyleDocument::from_json_with(older_major, FIXTURE).unwrap();
        assert_eq!(read.document.map.image_color, Some(Rgba::opaque(1, 2, 3)));
        assert_eq!(read.warnings[0].code, "format-migrated");
    }

    /// A layer with a render mode and a class whose expression uses `op`, in YAML of `version`.
    fn with_unknowns(version: &str, render_mode: &str, op: &str) -> String {
        format!(
            "version: \"{version}\"\nmap:\n  layers:\n  - render_mode: {render_mode}\n    \
             classes:\n    - name: c\n      expression: {{op: {op}, args: [{{property: geom}}, 5]}}\n"
        )
    }

    #[test]
    fn a_newer_minors_unknown_enum_value_is_dropped_with_a_warning() {
        // Verifies: ADR-0020
        let read = StyleDocument::from_yaml(&with_unknowns("1.9", "hologram", "=")).unwrap();
        let layer = &read.document.map.layers[0];
        assert_eq!(layer.render_mode, None);
        assert_eq!(layer.classes[0].name.as_deref(), Some("c"));
        let messages: Vec<_> = read.warnings.iter().map(|w| w.message.as_str()).collect();
        assert_eq!(
            messages,
            [
                "/map/layers/0/render_mode: `hologram` is unknown to format version 1.0 and is ignored"
            ]
        );
        assert_eq!(read.warnings[0].code, "unknown-value");
        let again = StyleDocument::from_yaml(&read.document.to_yaml().unwrap()).unwrap();
        assert_eq!(
            again.document, read.document,
            "the dropped value does not reappear"
        );
        assert!(again.warnings.is_empty());
    }

    #[test]
    fn a_newer_minors_unknown_operator_keeps_its_class_which_never_matches() {
        // Verifies: ADR-0019
        let read = StyleDocument::from_yaml(&with_unknowns("1.9", "polygon", "s_buffer")).unwrap();
        let class = &read.document.map.layers[0].classes[0];
        assert_eq!(class.name.as_deref(), Some("c"));
        assert_eq!(class.expression, Some(Expr::Bool(false)));
        let messages: Vec<_> = read.warnings.iter().map(|w| w.message.as_str()).collect();
        assert_eq!(
            messages,
            [
                "/map/layers/0/classes/0/expression: the operator `s_buffer` is unknown to format \
                 version 1.0, and the expression never matches"
            ]
        );
        assert_eq!(read.warnings[0].code, "unknown-operator");
        let json = read.document.to_json().unwrap();
        assert!(!json.contains("s_buffer"), "the operator does not reappear");
        // A filter is kept the same way; JSON reads as YAML does.
        let json = r#"{"version":"1.2","map":{"layers":[{"filter":{"op":"s_buffer","args":[]}}]}}"#;
        let read = StyleDocument::from_json(json).unwrap();
        assert_eq!(read.document.map.layers[0].filter, Some(Expr::Bool(false)));
    }

    #[test]
    fn in_the_current_minor_an_unknown_value_or_operator_is_a_load_error_naming_its_path() {
        let error = StyleDocument::from_yaml(&with_unknowns("1.0", "hologram", "=")).unwrap_err();
        assert!(
            error.message.starts_with("/map/layers/0/render_mode: "),
            "{}",
            error.message
        );
        let error =
            StyleDocument::from_yaml(&with_unknowns("1.0", "polygon", "s_buffer")).unwrap_err();
        assert!(
            error
                .message
                .starts_with("/map/layers/0/classes/0/expression: "),
            "{}",
            error.message
        );
        let json = r#"{"version":"1.0","map":{"layers":[{"status":"dim"}]}}"#;
        let error = StyleDocument::from_json(json).unwrap_err();
        assert!(
            error.message.starts_with("/map/layers/0/status: "),
            "{}",
            error.message
        );
    }

    #[test]
    fn a_newer_minor_drops_a_bounded_number_of_values() {
        // Verifies: Q-9
        let layers = |n: usize| {
            let layer = r#"{"status":"dim"}"#;
            let layers = vec![layer; n].join(",");
            format!(r#"{{"version":"1.9","map":{{"layers":[{layers}]}}}}"#)
        };
        let read = StyleDocument::from_json(&layers(MAX_DROPPED_VALUES)).unwrap();
        assert_eq!(read.warnings.len(), MAX_DROPPED_VALUES);
        let error = StyleDocument::from_json(&layers(MAX_DROPPED_VALUES + 1)).unwrap_err();
        assert_eq!(error.code, "too-many-unknown-values");
    }
}
