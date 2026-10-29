//! The reader of a newer minor's document that does not read as this build's version (ADR-0020,
//! ADR-0022): it finds where the typed read failed, drops what this build does not know there
//! with a warning, keeps a condition that does not read as one that never matches, and reads
//! again, at most [`MAX_DROPPED_VALUES`] times.

use std::fmt;

use serde_json::Value;

use crate::expression::nesting;
use crate::model::CONDITIONS;
use crate::{Diagnostic, FORMAT_VERSION, MAX_EXPRESSION_DEPTH, Read, StyleDocument};

/// How many unknown values a newer minor's document may drop before it is refused: each one is
/// found by reading the document again (Q-9).
pub const MAX_DROPPED_VALUES: usize = 100;

/// A step of the path to a place in a document.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Segment {
    Key(String),
    Index(usize),
}

/// The JSON Pointer (RFC 6901) of a place in a document, written and read in one place.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Pointer(Vec<Segment>);

impl Pointer {
    /// The place `serde_ignored` names.
    pub(crate) fn ignored(path: &serde_ignored::Path) -> Self {
        match path {
            serde_ignored::Path::Root => Self::default(),
            serde_ignored::Path::Seq { parent, index } => {
                Self::ignored(parent).with(Segment::Index(*index))
            }
            serde_ignored::Path::Map { parent, key } => {
                Self::ignored(parent).with(Segment::Key(key.clone()))
            }
            serde_ignored::Path::Some { parent }
            | serde_ignored::Path::NewtypeStruct { parent }
            | serde_ignored::Path::NewtypeVariant { parent } => Self::ignored(parent),
        }
    }

    /// The place at which `serde_path_to_error` saw a typed read fail.
    pub(crate) fn tracked(path: &serde_path_to_error::Path) -> Self {
        Self(
            path.iter()
                .filter_map(|segment| match segment {
                    serde_path_to_error::Segment::Map { key } => Some(Segment::Key(key.clone())),
                    serde_path_to_error::Segment::Seq { index } => Some(Segment::Index(*index)),
                    _ => None,
                })
                .collect(),
        )
    }

    fn with(mut self, segment: Segment) -> Self {
        self.0.push(segment);
        self
    }

    fn segments(&self) -> &[Segment] {
        &self.0
    }

    fn prefix(&self, len: usize) -> Self {
        Self(self.0[..len].to_vec())
    }
}

impl fmt::Display for Pointer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for segment in &self.0 {
            match segment {
                Segment::Key(key) => write!(f, "/{}", key.replace('~', "~0").replace('/', "~1"))?,
                Segment::Index(index) => write!(f, "/{index}")?,
            }
        }
        Ok(())
    }
}

/// A read error, prefixed with the JSON Pointer of where it happened.
pub(crate) fn read_error<E: fmt::Display>(
    code: &str,
    error: &serde_path_to_error::Error<E>,
) -> Diagnostic {
    let at = Pointer::tracked(error.path()).to_string();
    let message = if at.is_empty() {
        error.inner().to_string()
    } else {
        format!("{at}: {}", error.inner())
    };
    Diagnostic::error(code, message, None)
}

/// Whether serde's `reason` is a value an enum does not know (`unknown variant …`).
fn is_unknown_variant(reason: &str) -> bool {
    reason.starts_with("unknown variant")
}

/// Whether `reason` is an expression's or a binding's that fits none of their shapes, worded as
/// serde words an untagged enum's (`expression::no_variant`).
fn is_untagged_failure(reason: &str) -> bool {
    reason.contains("did not match any variant of untagged enum")
}

/// The field serde's `reason` says is missing (`missing field `…``).
fn missing_field(reason: &str) -> Option<&str> {
    Some(reason.strip_prefix("missing field `")?.split_once('`')?.0)
}

/// The keys of an expression's or a binding's object, which no other part of the IR has.
const EXPRESSION_KEYS: [&str; 6] = ["op", "args", "property", "date", "timestamp", "binding"];

/// Where on `path` the outermost expression or binding it runs into begins: the path without
/// the keys of expressions and the indices of their arguments, and of the lists among them, at
/// its end.
fn expression_start(path: &[Segment]) -> usize {
    let mut end = path.len();
    loop {
        match &path[..end] {
            [.., Segment::Key(key)] if EXPRESSION_KEYS.contains(&key.as_str()) => end -= 1,
            [rest @ .., Segment::Index(_)] => {
                let indices = rest
                    .iter()
                    .rev()
                    .take_while(|s| matches!(s, Segment::Index(_)))
                    .count();
                match rest[..rest.len() - indices].last() {
                    Some(Segment::Key(args)) if args == "args" => end -= 1,
                    _ => return end,
                }
            }
            _ => return end,
        }
    }
}

/// Whether `path` is that of one of the IR's [`CONDITIONS`], which is kept and never matches when
/// it does not read.
fn condition(path: &[Segment]) -> bool {
    match path {
        [
            ..,
            Segment::Key(list),
            Segment::Index(_),
            Segment::Key(field),
        ] => CONDITIONS
            .iter()
            .any(|c| c.list == list.as_str() && c.field == field.as_str()),
        _ => false,
    }
}

/// The depth of an expression's value tree, counted as [`crate::Expr::depth`] counts the tree, so
/// that one kept unread is still held to the limit (Q-9).
fn value_depth(value: &Value) -> usize {
    nesting(value, |value| match value {
        Value::Object(object) => object
            .get("args")
            .and_then(Value::as_array)
            .map_or(&[], Vec::as_slice),
        Value::Array(items) => items,
        _ => &[],
    })
}

/// The first operator in `value` that this build does not know.
fn unknown_operator(value: &Value) -> Option<String> {
    match value {
        Value::Object(object) => {
            if let Some(Value::String(op)) = object.get("op")
                && serde_json::from_value::<crate::Op>(op.clone().into()).is_err()
            {
                return Some(op.clone());
            }
            object.values().find_map(unknown_operator)
        }
        Value::Array(items) => items.iter().find_map(unknown_operator),
        _ => None,
    }
}

/// The error for a field this reader dropped that what held it cannot do without, a symbol's
/// kind: `reason` says a field is missing at `path`, and `removed` holds where values were
/// dropped, with the values.
fn required_dropped(
    reason: &str,
    path: &Pointer,
    removed: &[(Pointer, String)],
) -> Option<Diagnostic> {
    let place = path
        .clone()
        .with(Segment::Key(missing_field(reason)?.to_owned()));
    let (_, shown) = removed.iter().find(|(at, _)| *at == place)?;
    let message = format!(
        "{place}: `{shown}` is unknown to format version {FORMAT_VERSION}, and what holds it \
         cannot be read without it"
    );
    Some(Diagnostic::error("unknown-value", message, None))
}

/// Removes the field at `at` from its object.
fn remove(value: &mut Value, at: &Pointer) {
    let Some((Segment::Key(key), parent)) = at.segments().split_last() else {
        return;
    };
    let parent = Pointer(parent.to_vec()).to_string();
    if let Some(Value::Object(object)) = value.pointer_mut(&parent) {
        object.remove(key);
    }
}

/// Reads `value`, a document of a newer minor that does not read as this build's version,
/// dropping each value this build does not know with a warning naming it and its path: an enum
/// value unsets its field, and an expression with an unknown operator or function becomes one
/// that never matches, so that its class, label, or filter is kept (ADR-0030, ADR-0020). Anything else
/// that does not read is the error `code`, naming its path.
pub(crate) fn read(mut value: Value, code: &str) -> Result<Read, Diagnostic> {
    let mut dropped = Vec::new();
    // Where a value was dropped, and the value, for an error that its absence causes.
    let mut removed: Vec<(Pointer, String)> = Vec::new();
    loop {
        let mut ignored = Vec::new();
        let read = serde_path_to_error::deserialize(serde_ignored::Deserializer::new(
            value.clone(),
            &mut |path: serde_ignored::Path| ignored.push(Pointer::ignored(&path).to_string()),
        ));
        let error = match read {
            Ok(document) => {
                let mut read = StyleDocument::checked(document, ignored)?;
                dropped.append(&mut read.warnings);
                read.warnings = dropped;
                return Ok(read);
            }
            Err(error) => error,
        };
        let path = Pointer::tracked(error.path());
        let reason = error.inner().to_string();
        // What failed inside an expression or a binding is the whole one's.
        let slot = path.prefix(expression_start(path.segments()));
        let inside = slot.segments().len() < path.segments().len();
        // The field that holds what did not read: the nearest key on its path.
        let Some(key_at) = slot
            .segments()
            .iter()
            .rposition(|s| matches!(s, Segment::Key(_)))
        else {
            return Err(read_error(code, &error));
        };
        let at = slot.prefix(key_at + 1);
        let Some(failed) = value.pointer(&slot.to_string()).cloned() else {
            return Err(read_error(code, &error));
        };
        if let Some(error) = required_dropped(&reason, &path, &removed) {
            return Err(error);
        }
        let shown = |value: &Value| match value {
            Value::String(text) => text.clone(),
            other => other.to_string(),
        };
        // Only an expression, which reads as an untagged enum would, holds operators.
        let untagged = inside || is_untagged_failure(&reason);
        let operator = untagged.then(|| unknown_operator(&failed)).flatten();
        let warning = if condition(at.segments()) {
            // A class, label, or filter that does not read is kept and never matches; dropped, it
            // would match every feature.
            let field = value
                .pointer_mut(&at.to_string())
                .expect("the failed place is inside the field");
            if value_depth(field) > MAX_EXPRESSION_DEPTH {
                let message =
                    format!("{at}: an expression nests deeper than {MAX_EXPRESSION_DEPTH}");
                return Err(Diagnostic::error("expression-too-deep", message, None));
            }
            *field = Value::Bool(false);
            match operator {
                Some(op) => Diagnostic::warning(
                    "unknown-operator",
                    format!(
                        "{at}: the operator `{op}` is unknown to format version \
                         {FORMAT_VERSION}, and the expression never matches"
                    ),
                    None,
                ),
                None => Diagnostic::warning(
                    "unknown-value",
                    format!(
                        "{at}: the expression does not read in format version \
                         {FORMAT_VERSION}, and it never matches"
                    ),
                    None,
                ),
            }
        } else if let Some(op) = operator {
            remove(&mut value, &at);
            let message = format!(
                "{at}: the operator `{op}` is unknown to format version {FORMAT_VERSION}, and \
                 the field is ignored"
            );
            Diagnostic::warning("unknown-operator", message, None)
        } else if is_unknown_variant(&reason) || untagged {
            let shown = shown(&failed);
            let message = format!(
                "{at}: `{shown}` is unknown to format version {FORMAT_VERSION} and is ignored"
            );
            remove(&mut value, &at);
            removed.push((at, shown));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Bound, Class, Expr, Label, Layer, Map, Op, RenderMode, Style, Symbol};

    /// What serde says when `T` does not read from `json`.
    fn reason<T: serde::de::DeserializeOwned + fmt::Debug>(json: &str) -> String {
        let value: Value = serde_json::from_str(json).unwrap();
        serde_json::from_value::<T>(value).unwrap_err().to_string()
    }

    #[test]
    fn serdes_words_are_read_as_the_reader_expects_them() {
        // A serde update that rewords one of these fails here, not in a document.
        assert!(is_unknown_variant(&reason::<RenderMode>(r#""hologram""#)));
        assert!(!is_unknown_variant(&reason::<RenderMode>("5")));
        assert!(is_untagged_failure(&reason::<Expr>("{}")));
        assert!(!is_untagged_failure(&reason::<RenderMode>(r#""hologram""#)));
        assert_eq!(
            missing_field(&reason::<Symbol>(r#"{"name":"s"}"#)),
            Some("kind")
        );
        assert_eq!(missing_field(&reason::<RenderMode>("5")), None);
    }

    #[test]
    fn the_conditions_are_the_maps_layer_filters_and_class_and_label_expressions() {
        let filter = Expr::Bool(true);
        let expression = Expr::call(Op::Eq, vec![Expr::property("a"), Expr::Number(1.0)]);
        let shown = Expr::call(Op::Eq, vec![Expr::property("b"), Expr::Number(2.0)]);
        // Every other place an expression stands is filled too, so that a list naming one of
        // them, or a typed method reading one, finds it.
        let property = Some(Bound::Expr(Expr::property("w")));
        let style = Style {
            width: property.clone(),
            ..Style::default()
        };
        let map = Map {
            layers: vec![Layer {
                filter: Some(filter.clone()),
                classes: vec![Class {
                    expression: Some(expression.clone()),
                    styles: vec![style.clone()],
                    labels: vec![Label {
                        text: Some(Expr::property("t")),
                        expression: Some(shown.clone()),
                        styles: vec![style],
                        ..Label::default()
                    }],
                    ..Class::default()
                }],
                ..Layer::default()
            }],
            ..Map::default()
        };
        assert_eq!(map.conditions(), [&filter, &expression, &shown]);
        // The places CONDITIONS names hold what the typed map gives, each at least once.
        let tree = serde_json::to_value(&map).unwrap();
        let mut found = Vec::new();
        let mut matched = vec![false; CONDITIONS.len()];
        let mut pending = vec![&tree];
        while let Some(node) = pending.pop() {
            let Value::Object(object) = node else {
                continue;
            };
            for (i, condition) in CONDITIONS.iter().enumerate() {
                let items = object.get(condition.list).and_then(Value::as_array);
                for item in items.into_iter().flatten() {
                    if let Some(value) = item.get(condition.field) {
                        found.push(value.clone());
                        matched[i] = true;
                    }
                }
            }
            pending.extend(
                object
                    .values()
                    .flat_map(|v| v.as_array().into_iter().flatten()),
            );
        }
        assert!(
            matched.iter().all(|m| *m),
            "a condition names no place of the map"
        );
        let typed: Vec<Value> = map
            .conditions()
            .into_iter()
            .map(|e| serde_json::to_value(e).unwrap())
            .collect();
        assert_eq!(found.len(), typed.len());
        assert!(typed.iter().all(|e| found.contains(e)), "{found:?}");
    }

    #[test]
    fn a_pointer_escapes_its_keys_once() {
        let at = Pointer::default()
            .with(Segment::Key("a/b~".to_owned()))
            .with(Segment::Index(2));
        assert_eq!(at.to_string(), "/a~1b~0/2");
        let mut value = serde_json::json!({ "x": { "a/b~": 1, "c": 2 } });
        remove(
            &mut value,
            &Pointer::default()
                .with(Segment::Key("x".to_owned()))
                .with(Segment::Key("a/b~".to_owned())),
        );
        assert_eq!(value, serde_json::json!({ "x": { "c": 2 } }));
    }
}
