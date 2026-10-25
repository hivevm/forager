//! The JSON Schema of the current format version, derived from the IR's types, is the one
//! checked in under `schema/` (ADR-0020). `FORAGER_WRITE_SCHEMA=1` writes it instead of
//! comparing, the command the failure names.
//!
//! Once a release tag `v<semver>` of Forager holds schema files, those files are frozen, and the
//! derived schema is compared with that of the last released format version: a change that only
//! adds optional fields or enum values (a CQL2 function among them) asks for the next minor, any
//! other change for the next major, and no change for no bump.
#![cfg(feature = "schema")]

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
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
    let released = released(&root()).unwrap_or_else(|e| panic!("{e}"));
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

/// git in the repository at `dir`, and only there: without git's repository variables, which a
/// caller such as a hook in a linked worktree sets and `-C` does not override
/// (docs/CONVENTIONS.md).
fn git_in(dir: &Path) -> Command {
    let names = Command::new("git")
        .args(["rev-parse", "--local-env-vars"])
        .output()
        .expect("git runs");
    assert!(names.status.success(), "git names its repository variables");
    let mut git = Command::new("git");
    for name in String::from_utf8_lossy(&names.stdout).lines() {
        git.env_remove(name);
    }
    git.arg("-C").arg(dir);
    git
}

/// The schema files of every release tag `v<semver>` of the repository at `root`.
/// A clone without its history has no tags to read, and would pass every change as made before
/// the first release, so it is refused.
fn released(root: &Path) -> Result<Vec<Released>, String> {
    let git = |args: &[&str]| {
        let output = git_in(root).args(args).output().expect("git runs");
        assert!(output.status.success(), "git {args:?} fails");
        String::from_utf8(output.stdout).unwrap()
    };
    // An old git echoes the option it does not know; anything but `false` is refused.
    if git(&["rev-parse", "--is-shallow-repository"]).trim() != "false" {
        return Err(format!(
            "{} is a shallow clone, so the schema check cannot see the release tags whose schema \
             files are frozen; fetch its history with\n  git fetch --unshallow --tags",
            root.display()
        ));
    }
    let mut files = Vec::new();
    for tag in git(&["tag", "--list", "v[0-9]*"]).lines() {
        // A tag before the schema existed holds none, and lists nothing.
        let listing = git(&["ls-tree", "-z", "--name-only", tag, "--", "schema/"]);
        for path in listing.split('\0').filter(|p| p.ends_with(".json")) {
            let text = git(&["show", &format!("{tag}:{path}")]);
            files.push(Released {
                tag: tag.to_owned(),
                path: path.to_owned(),
                text,
            });
        }
    }
    Ok(files)
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
            let schema = serde_json::from_str(&file.text).unwrap_or_else(|e| {
                panic!("{} of release {} is no schema: {e}", file.path, file.tag)
            });
            Some((version, schema))
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
            "the schema only adds optional fields or enum values",
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

/// The change from the schema `old` to `new`.
fn change(old: &Value, new: &Value) -> Change {
    let defs = |v: &Value| {
        let defs = v.get("$defs").and_then(Value::as_object);
        defs.cloned().unwrap_or_default()
    };
    let compare = Compare {
        old: defs(old),
        new: defs(new),
        seen: RefCell::new(BTreeMap::new()),
    };
    compare.schema(old, new)
}

/// A comparison of two schemas with their definitions. A definition is compared where a `$ref`
/// names it, not by its name, which schemars numbers for each use of a generic type.
struct Compare {
    old: Map<String, Value>,
    new: Map<String, Value>,
    /// The pairs of definitions compared, with their change; one still being compared counts
    /// as unchanged, so that a recursive type ends.
    seen: RefCell<BTreeMap<(String, String), Change>>,
}

/// The definition a schema refers to, if it is a reference.
fn reference(schema: &Value) -> Option<&str> {
    schema.get("$ref")?.as_str()?.strip_prefix("#/$defs/")
}

/// The alternatives of an enum, each value of its own: `enum` values and `const` alternatives
/// alike, since a documented variant moves from the one to the other.
fn variants(schema: &Value) -> Option<Vec<Value>> {
    let object = schema.as_object()?;
    let values = |values: &Value| -> Vec<Value> {
        let values = values.as_array().into_iter().flatten();
        values.map(|v| json!({ "const": v })).collect()
    };
    if let Some(list) = object.get("enum") {
        return Some(values(list));
    }
    let mut variants = Vec::new();
    for alternative in object.get("oneOf")?.as_array()? {
        match alternative.get("enum") {
            Some(list) => variants.extend(values(list)),
            None => match alternative.get("const") {
                Some(value) => variants.push(json!({ "const": value })),
                None => variants.push(alternative.clone()),
            },
        }
    }
    Some(variants)
}

impl Compare {
    fn schema(&self, old: &Value, new: &Value) -> Change {
        match (reference(old), reference(new)) {
            (Some(a), Some(b)) => return self.definitions(a, b),
            (Some(a), None) => {
                return self
                    .old
                    .get(a)
                    .map_or(Change::Major, |x| self.schema(x, new));
            }
            (None, Some(b)) => {
                return self
                    .new
                    .get(b)
                    .map_or(Change::Major, |y| self.schema(old, y));
            }
            (None, None) => {}
        }
        if let (Some(a), Some(b)) = (variants(old), variants(new)) {
            return self.alternatives(&a, &b, true);
        }
        match (old, new) {
            (Value::Object(a), Value::Object(b)) => self.object(a, b),
            (Value::Array(a), Value::Array(b)) if a.len() == b.len() => a
                .iter()
                .zip(b)
                .map(|(x, y)| self.schema(x, y))
                .max()
                .unwrap_or(Change::None),
            _ if old == new => Change::None,
            _ => Change::Major,
        }
    }

    fn definitions(&self, a: &str, b: &str) -> Change {
        let key = (a.to_owned(), b.to_owned());
        if let Some(change) = self.seen.borrow().get(&key) {
            return *change;
        }
        self.seen.borrow_mut().insert(key.clone(), Change::None);
        let change = match (self.old.get(a), self.new.get(b)) {
            (Some(x), Some(y)) => self.schema(x, y),
            _ => Change::Major,
        };
        self.seen.borrow_mut().insert(key, change);
        change
    }

    /// Two schema objects, key by key. Descriptions and titles are documentation, not shape;
    /// definitions are compared where they are referred to.
    fn object(&self, a: &Map<String, Value>, b: &Map<String, Value>) -> Change {
        let keys: BTreeSet<&String> = a.keys().chain(b.keys()).collect();
        keys.into_iter()
            .filter(|key| !matches!(key.as_str(), "description" | "title" | "$defs"))
            .map(|key| match (key.as_str(), a.get(key), b.get(key)) {
                ("properties", Some(Value::Object(x)), Some(Value::Object(y))) => {
                    let required = b.get("required").and_then(Value::as_array);
                    self.properties(x, y, required.map(Vec::as_slice).unwrap_or_default())
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
                // A union a reader tries in turn: an older reader knows no new alternative.
                ("anyOf", Some(Value::Array(x)), Some(Value::Array(y))) => {
                    self.alternatives(x, y, false)
                }
                (_, Some(x), Some(y)) => self.schema(x, y),
                _ => Change::Major,
            })
            .max()
            .unwrap_or(Change::None)
    }

    /// An object's properties: one removed or changed beyond additions is major, one added is
    /// minor when it is not `required`.
    fn properties(
        &self,
        old: &Map<String, Value>,
        new: &Map<String, Value>,
        required: &[Value],
    ) -> Change {
        let kept = old.iter().map(|(name, schema)| {
            new.get(name)
                .map_or(Change::Major, |n| self.schema(schema, n))
        });
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

    /// Alternatives matched one to one, the unchanged first: each old one must still be there,
    /// changed at most by additions. One more is minor where `may_grow`, an enum's variant, and
    /// major in a union.
    fn alternatives(&self, old: &[Value], new: &[Value], may_grow: bool) -> Change {
        if new.len() < old.len() {
            return Change::Major;
        }
        let mut free: Vec<bool> = vec![true; new.len()];
        let mut worst = Change::None;
        let mut pending = Vec::new();
        for alternative in old {
            match new
                .iter()
                .enumerate()
                .position(|(i, n)| free[i] && n == alternative)
            {
                Some(i) => free[i] = false,
                None => pending.push(alternative),
            }
        }
        for alternative in pending {
            let best = new
                .iter()
                .enumerate()
                .filter(|(i, _)| free[*i])
                .map(|(i, n)| (self.schema(alternative, n), i))
                .min();
            match best {
                Some((change, i)) if change < Change::Major => {
                    free[i] = false;
                    worst = worst.max(change);
                }
                _ => return Change::Major,
            }
        }
        if new.len() > old.len() {
            worst = worst.max(if may_grow {
                Change::Minor
            } else {
                Change::Major
            });
        }
        worst
    }
}

/// A document schema of one object with the field `a`, and `extra` merged into its properties.
fn fixture(extra: Value, required: &[&str]) -> Value {
    let mut properties = json!({
        "a": { "type": "string", "description": "A field." },
        "m": { "$ref": "#/$defs/Mode" }
    });
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
    // An enum value is an addition too, a description is no change.
    let mut more = old.clone();
    more["$defs"]["Mode"]["enum"] = json!(["point", "line", "polygon"]);
    assert_eq!(change(&old, &more), Change::Minor);
    let mut union = fixture(json!({ "u": { "anyOf": [{ "type": "string" }] } }), &["a"]);
    let wider = fixture(
        json!({ "u": { "anyOf": [{ "type": "string" }, { "type": "number" }] } }),
        &["a"],
    );
    assert_eq!(
        change(&union, &wider),
        Change::Major,
        "a union's new shape is major"
    );
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

#[test]
fn definitions_compare_where_they_are_referred_to_not_by_their_number() {
    // Verifies: ADR-0020
    // schemars numbers each use of a generic type: an added `Bound<u32>` renumbers the others.
    let old = json!({
        "type": "object",
        "properties": { "size": { "$ref": "#/$defs/Bound2" } },
        "$defs": { "Bound2": { "anyOf": [{ "type": "number" }, { "type": "string" }] } }
    });
    let new = json!({
        "type": "object",
        "properties": {
            "glow": { "anyOf": [{ "$ref": "#/$defs/Bound2" }, { "type": "null" }] },
            "size": { "$ref": "#/$defs/Bound3" }
        },
        "$defs": {
            "Bound2": { "anyOf": [{ "type": "integer" }, { "type": "string" }] },
            "Bound3": { "anyOf": [{ "type": "number" }, { "type": "string" }] }
        }
    });
    assert_eq!(change(&old, &new), Change::Minor);
    let retyped = json!({
        "type": "object",
        "properties": { "size": { "$ref": "#/$defs/Bound2" } },
        "$defs": { "Bound2": { "anyOf": [{ "type": "integer" }, { "type": "string" }] } }
    });
    assert_eq!(change(&old, &retyped), Change::Major);
}

#[test]
fn a_documented_variant_is_no_change_and_a_new_shape_of_a_union_is_major() {
    // Verifies: ADR-0020
    let plain = json!({ "type": "string", "enum": ["butt", "round"] });
    let documented = json!({ "oneOf": [
        { "type": "string", "const": "butt", "description": "Flat at the end." },
        { "type": "string", "enum": ["round"] }
    ] });
    assert_eq!(change(&plain, &documented), Change::None);
    // A variant with data is an enum value too.
    let with_data = json!({ "oneOf": [
        { "type": "string", "enum": ["butt", "round"] },
        { "type": "object", "properties": { "custom": { "type": "number" } }, "required": ["custom"] }
    ] });
    assert_eq!(change(&plain, &with_data), Change::Minor);
    // An untagged union an older reader tries in turn: a new alternative fails it.
    let union = json!({ "anyOf": [{ "type": "string" }] });
    let wider = json!({ "anyOf": [{ "type": "string" }, { "type": "number" }] });
    assert_eq!(change(&union, &wider), Change::Major);
    // One old alternative cannot stand in for another that was removed.
    let x = json!({ "type": "object", "properties": { "a": {}, "b": {} } });
    let y = json!({ "type": "object", "properties": { "a": {} } });
    let old = json!({ "anyOf": [x.clone(), y] });
    let new = json!({ "anyOf": [x, { "type": "integer" }] });
    assert_eq!(change(&old, &new), Change::Major);
}

#[test]
fn a_release_tag_from_before_the_schema_holds_no_schema_file() {
    let dir = std::env::temp_dir().join(format!("forager-schema-tags-{}", std::process::id()));
    let git = |args: &[&str]| {
        let status = git_in(&dir)
            .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid"])
            .args(args)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    };
    std::fs::create_dir_all(&dir).unwrap();
    git(&["init", "-q"]);
    git(&["commit", "-q", "--allow-empty", "-m", "before the schema"]);
    git(&["tag", "v0.0.1"]);
    std::fs::create_dir_all(dir.join("schema")).unwrap();
    std::fs::write(dir.join("schema/style-document-1.0.json"), "{}\n").unwrap();
    git(&["add", "schema"]);
    git(&["commit", "-q", "-m", "the schema"]);
    git(&["tag", "-a", "v0.1.0", "-m", "release"]);
    let released = released(&dir).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    let found: Vec<_> = released
        .iter()
        .map(|f| (f.tag.as_str(), f.path.as_str()))
        .collect();
    assert_eq!(found, [("v0.1.0", "schema/style-document-1.0.json")]);
}

#[test]
fn the_fixture_repository_is_the_only_one_its_git_touches() {
    // Verifies: ADR-0002
    // The tag test again, in a process whose GIT_DIR names a decoy, as a hook in a linked
    // worktree would set it: the decoy has no new commit and no tag afterwards.
    if std::env::var_os("FORAGER_DECOY").is_some() {
        return;
    }
    let decoy = std::env::temp_dir().join(format!("forager-schema-decoy-{}", std::process::id()));
    let refs = || {
        let output = git_in(&decoy)
            .args(["for-each-ref", "--format=%(refname) %(objectname)"])
            .output()
            .unwrap();
        String::from_utf8(output.stdout).unwrap()
    };
    std::fs::create_dir_all(&decoy).unwrap();
    assert!(
        git_in(&decoy)
            .args(["init", "-q"])
            .status()
            .unwrap()
            .success()
    );
    let commit = [
        "-c",
        "user.name=t",
        "-c",
        "user.email=t@example.invalid",
        "commit",
    ];
    let made = git_in(&decoy)
        .args(commit)
        .args(["-q", "--allow-empty", "-m", "decoy"])
        .status()
        .unwrap();
    assert!(made.success());
    let before = refs();
    let status = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "a_release_tag_from_before_the_schema_holds_no_schema_file",
        ])
        .env("GIT_DIR", decoy.join(".git"))
        .env("FORAGER_DECOY", "1")
        .output()
        .unwrap();
    let after = refs();
    std::fs::remove_dir_all(&decoy).unwrap();
    assert!(
        status.status.success(),
        "the tag test fails under a GIT_DIR"
    );
    // A renamed tag test would leave the filter matching nothing, and pass unseen.
    let stdout = String::from_utf8_lossy(&status.stdout);
    assert!(
        stdout.contains("1 passed"),
        "the tag test did not run:\n{stdout}"
    );
    assert_eq!(
        after, before,
        "the tag test wrote into the repository GIT_DIR names"
    );
}

#[test]
fn a_clone_without_its_history_is_refused_and_one_without_a_release_passes() {
    // Verifies: ADR-0020
    let base = std::env::temp_dir().join(format!("forager-schema-shallow-{}", std::process::id()));
    let origin = base.join("origin");
    let git = |dir: &Path, args: &[&str]| {
        let status = git_in(dir)
            .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid"])
            .args(args)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    };
    std::fs::create_dir_all(&origin).unwrap();
    git(&origin, &["init", "-q"]);
    git(&origin, &["commit", "-q", "--allow-empty", "-m", "one"]);
    git(&origin, &["commit", "-q", "--allow-empty", "-m", "two"]);
    let url = format!("file://{}", origin.display());
    git(&base, &["clone", "-q", "--depth", "1", &url, "shallow"]);
    git(&base, &["clone", "-q", "--no-tags", &url, "full"]);
    let shallow = released(&base.join("shallow"));
    let full = released(&base.join("full"));
    std::fs::remove_dir_all(&base).unwrap();
    let error = shallow.err().expect("a shallow clone is refused");
    assert!(error.contains("git fetch --unshallow --tags"), "{error}");
    assert!(full.unwrap().is_empty());
}

/// Fixture types in two versions, so that the classifier is tested on what schemars writes, not
/// on what a test supposes it writes.
mod derived {
    #![allow(dead_code)]

    use forager_style_ir::Bound;
    use schemars::JsonSchema;
    use serde::Deserialize;

    #[derive(Deserialize, JsonSchema)]
    pub struct V1 {
        pub size: Option<Bound<f64>>,
        pub name: String,
    }

    /// `V1` and an optional field of another `Bound`, which schemars numbers before `size`'s.
    #[derive(Deserialize, JsonSchema)]
    pub struct V2Optional {
        pub glow: Option<Bound<u32>>,
        pub size: Option<Bound<f64>>,
        pub name: String,
    }

    /// `V1` and a required field.
    #[derive(Deserialize, JsonSchema)]
    pub struct V2Required {
        pub size: Option<Bound<f64>>,
        pub name: String,
        pub count: u32,
    }

    #[derive(Deserialize, JsonSchema)]
    #[serde(rename_all = "lowercase")]
    pub enum Cap {
        Butt,
        Round,
    }

    #[derive(Deserialize, JsonSchema)]
    #[serde(rename_all = "lowercase")]
    pub enum CapDocumented {
        /// Flat at the end.
        Butt,
        Round,
    }

    #[derive(Deserialize, JsonSchema)]
    #[serde(rename_all = "lowercase")]
    pub enum CapWider {
        Butt,
        Round,
        Square,
    }

    #[derive(Deserialize, JsonSchema)]
    #[serde(untagged)]
    pub enum Union {
        Number(f64),
    }

    #[derive(Deserialize, JsonSchema)]
    #[serde(untagged)]
    pub enum UnionWider {
        Number(f64),
        Text(String),
    }
}

fn schema<T: schemars::JsonSchema>() -> Value {
    serde_json::to_value(schemars::schema_for!(T)).unwrap()
}

#[test]
fn an_optional_field_of_another_bound_is_minor_in_derived_schemas() {
    // Verifies: ADR-0020
    use derived::*;
    assert_eq!(
        change(&schema::<V1>(), &schema::<V2Optional>()),
        Change::Minor
    );
    assert_eq!(
        change(&schema::<V1>(), &schema::<V2Required>()),
        Change::Major
    );
    assert_eq!(
        change(&schema::<V2Optional>(), &schema::<V1>()),
        Change::Major
    );
}

#[test]
fn a_documented_variant_is_no_change_and_a_new_one_minor_in_derived_schemas() {
    // Verifies: ADR-0020
    use derived::*;
    assert_eq!(
        change(&schema::<Cap>(), &schema::<CapDocumented>()),
        Change::None
    );
    assert_eq!(
        change(&schema::<Cap>(), &schema::<CapWider>()),
        Change::Minor
    );
    assert_eq!(
        change(&schema::<CapWider>(), &schema::<Cap>()),
        Change::Major
    );
}

#[test]
fn a_new_alternative_of_an_untagged_union_is_major_in_derived_schemas() {
    // Verifies: ADR-0020
    use derived::*;
    assert_eq!(
        change(&schema::<Union>(), &schema::<UnionWider>()),
        Change::Major
    );
}
