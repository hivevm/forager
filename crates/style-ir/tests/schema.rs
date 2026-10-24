//! The JSON Schema of the current format version, derived from the IR's types, is the one
//! checked in under `schema/` (ADR-0020). `FORAGER_WRITE_SCHEMA=1` writes it instead of
//! comparing, the command the failure names.
//!
//! Once a release tag `v<semver>` of Forager holds schema files, those files are frozen, and the
//! derived schema is compared with that of the last released format version: a change that only
//! adds optional fields, enum values, or alternatives (a CQL2 function among them) asks for the
//! next minor, any other change for the next major, and no change for no bump.
#![cfg(feature = "schema")]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use forager_style_ir::{FORMAT_VERSION, StyleDocument};
use serde_json::{Map, Value, json};

fn derived() -> String {
    let schema = schemars::schema_for!(StyleDocument);
    serde_json::to_string_pretty(&schema).unwrap() + "\n"
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn schema_file(version: &str) -> String {
    format!("schema/style-document-{version}.json")
}

#[test]
fn the_checked_in_schema_is_the_one_the_ir_derives() {
    // Verifies: ADR-0020
    let derived = derived();
    let checked_in = root().join(schema_file(FORMAT_VERSION));
    if std::env::var_os("FORAGER_WRITE_SCHEMA").is_some() {
        std::fs::write(&checked_in, &derived).unwrap();
        return;
    }
    let file = std::fs::read_to_string(&checked_in).unwrap_or_default();
    assert!(
        file == derived,
        "{} is not the schema the IR derives; regenerate it with\n  \
         FORAGER_WRITE_SCHEMA=1 cargo test -p forager-style-ir --features schema --test schema",
        checked_in.display()
    );
}

#[test]
fn the_bump_matches_the_change_since_the_last_release() {
    // Verifies: ADR-0020
    let released = released(&root());
    frozen(&released, |path| {
        std::fs::read_to_string(root().join(path)).ok()
    })
    .unwrap_or_else(|e| panic!("{e}"));
    let derived: Value = serde_json::from_str(&derived()).unwrap();
    verdict(last(&released), FORMAT_VERSION, &derived).unwrap_or_else(|e| panic!("{e}"));
}

/// What changed between two schemas, as ADR-0020's version rule sorts it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Change {
    None,
    Minor,
    Major,
}

/// A schema file a release tag holds: the tag, the file's path, and its text.
struct Released {
    tag: String,
    path: String,
    text: String,
}

/// The schema files of every release tag `v<semver>` of the repository at `root`.
fn released(root: &Path) -> Vec<Released> {
    let git = |args: &[&str]| {
        let output = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .expect("git runs");
        assert!(output.status.success(), "git {args:?} fails");
        String::from_utf8(output.stdout).unwrap()
    };
    let mut files = Vec::new();
    for tag in git(&["tag", "--list", "v[0-9]*"]).lines() {
        let listing = git(&["ls-tree", "--name-only", &format!("{tag}:schema")]);
        for name in listing.lines().filter(|n| n.ends_with(".json")) {
            let path = format!("schema/{name}");
            let text = git(&["show", &format!("{tag}:{path}")]);
            files.push(Released {
                tag: tag.to_owned(),
                path,
                text,
            });
        }
    }
    files
}

/// Fails where a released schema file differs from its copy in the tree, as `read` reads it.
fn frozen(released: &[Released], read: impl Fn(&str) -> Option<String>) -> Result<(), String> {
    for file in released {
        if read(&file.path).as_deref() != Some(file.text.as_str()) {
            return Err(format!(
                "{} is part of release {} and must not change; a new shape is a new version",
                file.path, file.tag
            ));
        }
    }
    Ok(())
}

/// The major and minor of a format version.
fn version(text: &str) -> Option<(u32, u32)> {
    let (major, minor) = text.split_once('.')?;
    Some((major.parse().ok()?, minor.parse().ok()?))
}

/// The last released format version and its schema.
fn last(released: &[Released]) -> Option<((u32, u32), Value)> {
    released
        .iter()
        .filter_map(|file| {
            let name = file.path.strip_prefix("schema/style-document-")?;
            let version = version(name.strip_suffix(".json")?)?;
            Some((version, serde_json::from_str(&file.text).ok()?))
        })
        .max_by_key(|(version, _)| *version)
}

/// Whether `current`, the format version this build writes, is the bump that the change from
/// the last released schema to `derived` asks for; without a release, any version holds.
fn verdict(
    last: Option<((u32, u32), Value)>,
    current: &str,
    derived: &Value,
) -> Result<(), String> {
    let Some(((major, minor), schema)) = last else {
        return Ok(());
    };
    let (asked, why) = match change(&schema, derived) {
        Change::None => (format!("{major}.{minor}"), "the schema is unchanged"),
        Change::Minor => (
            format!("{major}.{}", minor + 1),
            "the schema only adds optional fields, enum values, or alternatives",
        ),
        Change::Major => (
            format!("{}.0", major + 1),
            "the schema changes beyond additions",
        ),
    };
    if current == asked {
        Ok(())
    } else {
        Err(format!(
            "{why} since the released {major}.{minor}, so the format version must be {asked}, \
             and it is {current} (FORMAT_VERSION in crates/style-ir/src/document.rs)"
        ))
    }
}

/// The change from the schema `old` to `new`. Descriptions are documentation, not shape.
fn change(old: &Value, new: &Value) -> Change {
    match (old, new) {
        (Value::Object(a), Value::Object(b)) => object_change(a, b),
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => a
            .iter()
            .zip(b)
            .map(|(x, y)| change(x, y))
            .max()
            .unwrap_or(Change::None),
        _ if old == new => Change::None,
        _ => Change::Major,
    }
}

fn object_change(a: &Map<String, Value>, b: &Map<String, Value>) -> Change {
    let keys: BTreeSet<&String> = a.keys().chain(b.keys()).collect();
    keys.into_iter()
        .filter(|key| *key != "description")
        .map(|key| match (key.as_str(), a.get(key), b.get(key)) {
            ("properties", Some(Value::Object(x)), Some(Value::Object(y))) => {
                let required = b.get("required").and_then(Value::as_array);
                properties(x, y, required.map(Vec::as_slice).unwrap_or_default())
            }
            ("required", Some(x), Some(y)) => {
                let set = |v: &Value| -> BTreeSet<String> {
                    let items = v.as_array().into_iter().flatten();
                    items.map(|item| item.to_string()).collect()
                };
                if set(x) == set(y) {
                    Change::None
                } else {
                    Change::Major
                }
            }
            ("enum", Some(Value::Array(x)), Some(Value::Array(y))) => {
                if !x.iter().all(|value| y.contains(value)) {
                    Change::Major
                } else if y.len() > x.len() {
                    Change::Minor
                } else {
                    Change::None
                }
            }
            ("oneOf" | "anyOf", Some(Value::Array(x)), Some(Value::Array(y))) => alternatives(x, y),
            ("$defs", Some(Value::Object(x)), Some(Value::Object(y))) => x
                .iter()
                .map(|(name, def)| y.get(name).map_or(Change::Major, |new| change(def, new)))
                .max()
                .unwrap_or(Change::None),
            (_, Some(x), Some(y)) => change(x, y),
            _ => Change::Major,
        })
        .max()
        .unwrap_or(Change::None)
}

/// An object's properties: one removed or changed beyond additions is major, one added is minor
/// when it is not `required`.
fn properties(old: &Map<String, Value>, new: &Map<String, Value>, required: &[Value]) -> Change {
    let kept = old
        .iter()
        .map(|(name, schema)| new.get(name).map_or(Change::Major, |n| change(schema, n)));
    let added = new
        .keys()
        .filter(|name| !old.contains_key(*name))
        .map(|name| {
            if required.contains(&Value::String(name.clone())) {
                Change::Major
            } else {
                Change::Minor
            }
        });
    kept.chain(added).max().unwrap_or(Change::None)
}

/// The alternatives of an enum or a union: each old one must still be there, changed at most by
/// additions; one more is minor.
fn alternatives(old: &[Value], new: &[Value]) -> Change {
    if new.len() < old.len() {
        return Change::Major;
    }
    let kept = old.iter().map(|alternative| {
        new.iter()
            .map(|n| change(alternative, n))
            .min()
            .unwrap_or(Change::Major)
    });
    let added = (new.len() > old.len()).then_some(Change::Minor);
    kept.chain(added).max().unwrap_or(Change::None)
}

/// A document schema of one object with the field `a`, and `extra` merged into its properties.
fn fixture(extra: Value, required: &[&str]) -> Value {
    let mut properties = json!({ "a": { "type": "string", "description": "A field." } });
    properties
        .as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "$defs": { "Mode": { "type": "string", "enum": ["point", "line"] } }
    })
}

fn released_at(version: (u32, u32), schema: &Value) -> Option<((u32, u32), Value)> {
    Some((version, schema.clone()))
}

#[test]
fn an_added_optional_field_asks_for_the_next_minor() {
    // Verifies: ADR-0020
    let old = fixture(json!({}), &["a"]);
    let new = fixture(json!({ "b": { "type": "number" } }), &["a"]);
    let error = verdict(released_at((1, 0), &old), "1.0", &new).unwrap_err();
    assert!(error.contains("must be 1.1"), "{error}");
    assert_eq!(verdict(released_at((1, 0), &old), "1.1", &new), Ok(()));
    // An enum value or an alternative is an addition too, a description is no change.
    let mut more = old.clone();
    more["$defs"]["Mode"]["enum"] = json!(["point", "line", "polygon"]);
    assert_eq!(change(&old, &more), Change::Minor);
    let mut union = fixture(json!({ "u": { "anyOf": [{ "type": "string" }] } }), &["a"]);
    let wider = fixture(
        json!({ "u": { "anyOf": [{ "type": "string" }, { "type": "number" }] } }),
        &["a"],
    );
    assert_eq!(change(&union, &wider), Change::Minor);
    union["properties"]["a"]["description"] = json!("Reworded.");
    assert_eq!(
        change(
            &fixture(json!({ "u": { "anyOf": [{ "type": "string" }] } }), &["a"]),
            &union
        ),
        Change::None
    );
}

#[test]
fn a_removed_renamed_or_required_field_asks_for_the_next_major() {
    // Verifies: ADR-0020
    let old = fixture(json!({ "b": { "type": "number" } }), &["a"]);
    let removed = fixture(json!({}), &["a"]);
    let error = verdict(released_at((1, 0), &old), "1.1", &removed).unwrap_err();
    assert!(error.contains("must be 2.0"), "{error}");
    let renamed = fixture(json!({ "c": { "type": "number" } }), &["a"]);
    assert_eq!(change(&old, &renamed), Change::Major);
    let required = fixture(json!({ "b": { "type": "number" } }), &["a", "b"]);
    assert_eq!(change(&old, &required), Change::Major);
    let mut fewer = old.clone();
    fewer["$defs"]["Mode"]["enum"] = json!(["point"]);
    assert_eq!(change(&old, &fewer), Change::Major);
    let retyped = fixture(json!({ "b": { "type": "string" } }), &["a"]);
    assert_eq!(change(&old, &retyped), Change::Major);
    assert_eq!(verdict(released_at((1, 0), &old), "2.0", &removed), Ok(()));
}

#[test]
fn an_unchanged_schema_keeps_its_version_and_a_released_file_is_frozen() {
    // Verifies: ADR-0020
    let old = fixture(json!({}), &["a"]);
    assert_eq!(verdict(released_at((1, 0), &old), "1.0", &old), Ok(()));
    assert!(verdict(released_at((1, 0), &old), "1.1", &old).is_err());
    let file = Released {
        tag: "v0.1.0".to_owned(),
        path: schema_file("1.0"),
        text: "{}\n".to_owned(),
    };
    assert_eq!(
        frozen(std::slice::from_ref(&file), |_| Some("{}\n".to_owned())),
        Ok(())
    );
    let error = frozen(&[file], |_| Some("{ }\n".to_owned())).unwrap_err();
    assert!(error.contains("part of release v0.1.0"), "{error}");
}

#[test]
fn without_a_release_only_the_file_must_be_fresh() {
    // Verifies: ADR-0020
    let new = fixture(json!({ "b": { "type": "number" } }), &["a", "b"]);
    assert_eq!(verdict(None, "1.0", &new), Ok(()));
    assert_eq!(frozen(&[], |_| None), Ok(()));
}
