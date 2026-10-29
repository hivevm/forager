//! The shape of the code (ADR-0009): the dependency edges allowed between workspace crates, and
//! no crate that links C code under the IR, the core, the ports, or the frontend (Q-1).

use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;

use serde_json::Value;

/// The groups of ADR-0009's table.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Group {
    Ir,
    Core,
    Ports,
    Frontend,
    Adapters,
    Hosts,
    Tooling,
}

/// What a crate may depend on: the crates its row names, or for `engine` the IR, every other core
/// crate, and the ports. The groups of hosts and tooling decide over their rows, `OfItsGroup`: a
/// host may depend on every crate outside the hosts and the tooling, the tooling on none; so a
/// crate moved to the hosts needs no second edit.
#[derive(Clone, Copy)]
enum Edges {
    To(&'static [&'static str]),
    TheIrTheCoreAndThePorts,
    OfItsGroup,
}

/// A row of ADR-0009's table: a crate by name without the `forager-` prefix, where a name ending
/// in `*` stands for every crate it prefixes, its group, and its edges.
type Row = (&'static str, Group, Edges);

/// ADR-0009's table, the one place a crate is named; crates not yet created are named too.
const TABLE: &[Row] = &[
    ("style-ir", Group::Ir, Edges::To(&[])),
    ("expr", Group::Core, Edges::To(&["style-ir"])),
    ("proj", Group::Core, Edges::To(&["style-ir"])),
    ("render", Group::Core, Edges::To(&["style-ir"])),
    (
        "render-agg",
        Group::Core,
        Edges::To(&["style-ir", "render"]),
    ),
    ("encode", Group::Core, Edges::To(&["render"])),
    (
        "symbolize",
        Group::Core,
        Edges::To(&["style-ir", "expr", "proj", "render"]),
    ),
    (
        "label",
        Group::Core,
        Edges::To(&["style-ir", "expr", "proj", "render"]),
    ),
    (
        "raster",
        Group::Core,
        Edges::To(&["style-ir", "expr", "proj", "render"]),
    ),
    ("engine", Group::Core, Edges::TheIrTheCoreAndThePorts),
    ("ports", Group::Ports, Edges::To(&["style-ir"])),
    (
        "mapfile",
        Group::Frontend,
        Edges::To(&["style-ir", "ports"]),
    ),
    (
        "adapter-*",
        Group::Adapters,
        Edges::To(&["style-ir", "ports"]),
    ),
    ("cli", Group::Hosts, Edges::OfItsGroup),
    ("bindings-wasm", Group::Hosts, Edges::OfItsGroup),
    ("dev-server", Group::Hosts, Edges::OfItsGroup),
    ("bindings-wit", Group::Hosts, Edges::OfItsGroup),
    ("arch-test", Group::Tooling, Edges::OfItsGroup),
];

/// Whether a row's name names the crate `name`.
fn names(pattern: &str, name: &str) -> bool {
    match pattern.strip_suffix('*') {
        Some(prefix) => name.starts_with(prefix),
        None => name == pattern,
    }
}

/// The row that names `name`, if one does.
fn row<'t>(table: &'t [Row], name: &str) -> Option<&'t Row> {
    table.iter().find(|(pattern, _, _)| names(pattern, name))
}

/// The group of the crate `name`, if the table names it.
fn group(table: &[Row], name: &str) -> Option<Group> {
    row(table, name).map(|&(_, group, _)| group)
}

/// Whether `from` may depend on `to`; `None` when the table does not name `from`.
fn may_depend(table: &[Row], from: &str, to: &str) -> Option<bool> {
    let &(_, from_group, edges) = row(table, from)?;
    let to_group = group(table, to);
    Some(match (from_group, edges) {
        (Group::Hosts, _) => !matches!(to_group, Some(Group::Hosts | Group::Tooling)),
        (Group::Tooling, _) | (_, Edges::OfItsGroup) => false,
        (_, Edges::To(crates)) => crates.contains(&to),
        (_, Edges::TheIrTheCoreAndThePorts) => {
            from != to && matches!(to_group, Some(Group::Ir | Group::Core | Group::Ports))
        }
    })
}

/// Whether a crate must link no C code, directly or transitively: the IR, the core, the ports,
/// and the frontend (ADR-0009, Q-1).
fn must_be_pure_rust(table: &[Row], name: &str) -> bool {
    matches!(
        group(table, name),
        Some(Group::Ir | Group::Core | Group::Ports | Group::Frontend)
    )
}

/// The crates of the IR and the core, held to the ban on hash maps, clocks, and randomness (Q-2).
fn ir_and_core(table: &[Row]) -> impl Iterator<Item = &'static str> + '_ {
    table
        .iter()
        .filter(|(_, group, _)| matches!(group, Group::Ir | Group::Core))
        .map(|&(name, _, _)| name)
}

struct Workspace {
    /// Short name of each workspace crate, by package id.
    members: BTreeMap<String, String>,
    /// Normal (not dev, not build) dependencies, by package id.
    deps: BTreeMap<String, Vec<String>>,
    /// Package name, by package id.
    names: BTreeMap<String, String>,
}

fn workspace() -> Workspace {
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1", "--locked"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo metadata runs");
    assert!(output.status.success(), "{output:?}");
    let metadata: Value = serde_json::from_slice(&output.stdout).unwrap();
    let names = metadata["packages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| (string(&p["id"]), string(&p["name"])))
        .collect::<BTreeMap<_, _>>();
    let members = metadata["workspace_members"]
        .as_array()
        .unwrap()
        .iter()
        .map(|id| {
            let id = string(id);
            let name = names[&id]
                .strip_prefix("forager-")
                .unwrap_or(&names[&id])
                .to_owned();
            (id, name)
        })
        .collect();
    let deps = metadata["resolve"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|node| {
            let normal = node["deps"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|dep| {
                    dep["dep_kinds"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|k| k["kind"].is_null())
                })
                .map(|dep| string(&dep["pkg"]))
                .collect();
            (string(&node["id"]), normal)
        })
        .collect();
    Workspace {
        members,
        deps,
        names,
    }
}

fn string(value: &Value) -> String {
    value.as_str().unwrap().to_owned()
}

#[test]
fn workspace_crates_depend_only_along_the_allowed_edges() {
    // Verifies: ADR-0009
    let ws = workspace();
    let mut violations = Vec::new();
    for (id, from) in &ws.members {
        if may_depend(TABLE, from, "style-ir").is_none() {
            violations.push(format!("{from} is not a crate ADR-0009 names"));
            continue;
        }
        for dep in &ws.deps[id] {
            if let Some(to) = ws.members.get(dep)
                && may_depend(TABLE, from, to) != Some(true)
            {
                violations.push(format!("{from} -> {to}"));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "edges ADR-0009 does not allow: {violations:?}"
    );
}

#[test]
fn the_ir_core_ports_and_frontend_link_no_c_code() {
    // Verifies: ADR-0009, Q-1
    let ws = workspace();
    let mut violations = BTreeSet::new();
    for (id, name) in ws
        .members
        .iter()
        .filter(|(_, n)| must_be_pure_rust(TABLE, n))
    {
        let mut stack = vec![id.clone()];
        let mut seen = BTreeSet::new();
        while let Some(current) = stack.pop() {
            if !seen.insert(current.clone()) {
                continue;
            }
            if ws.names[&current].ends_with("-sys") {
                violations.insert(format!("{name} -> {}", ws.names[&current]));
            }
            stack.extend(ws.deps[&current].iter().cloned());
        }
    }
    assert!(
        violations.is_empty(),
        "crates that link C code: {violations:?}"
    );
}

/// The workspace crates that depend on the crate named `dependency` directly, by short name.
fn dependents(ws: &Workspace, dependency: &str) -> Vec<String> {
    ws.members
        .iter()
        .filter(|(id, _)| ws.deps[*id].iter().any(|dep| ws.names[dep] == dependency))
        .map(|(_, name)| name.clone())
        .collect()
}

#[test]
fn only_label_reads_fonts_with_skrifa() {
    // Verifies: ADR-0028
    let ws = workspace();
    assert_eq!(dependents(&ws, "skrifa"), ["label"]);
    // The check itself sees a dependency: label depends on forager-render.
    assert!(dependents(&ws, "forager-render").contains(&"label".to_owned()));
}

#[test]
fn only_expr_depends_on_the_regex_crates() {
    // Verifies: ADR-0031
    // The patterns of the style IR are read as PCRE2 reads them and matched in one place; a crate
    // that reached for `regex` itself would match them otherwise.
    let ws = workspace();
    let mut violations = Vec::new();
    for (id, name) in ws.members.iter().filter(|(_, n)| n.as_str() != "expr") {
        for dep in &ws.deps[id] {
            let dep = &ws.names[dep];
            if dep == "regex" || dep.starts_with("regex-") {
                violations.push(format!("{name} -> {dep}"));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "crates other than expr that use regex: {violations:?}"
    );
}

#[test]
fn the_rules_reject_a_wrong_edge_and_an_unnamed_crate() {
    // The table itself, so that a test that never fails cannot pass for a check.
    let t = TABLE;
    assert_eq!(may_depend(t, "style-ir", "ports"), Some(false));
    assert_eq!(may_depend(t, "render", "expr"), Some(false));
    assert_eq!(may_depend(t, "mapfile", "engine"), Some(false));
    assert_eq!(may_depend(t, "adapter-geojson", "engine"), Some(false));
    assert_eq!(may_depend(t, "engine", "ports"), Some(true));
    assert_eq!(may_depend(t, "server", "style-ir"), None);
    assert_eq!(may_depend(t, "cli", "adapter-geojson"), Some(true));
    assert_eq!(may_depend(t, "cli", "bindings-wasm"), Some(false));
    assert_eq!(may_depend(t, "bindings-wasm", "arch-test"), Some(false));
    assert!(must_be_pure_rust(t, "mapfile") && !must_be_pure_rust(t, "bindings-wasm"));
    assert!(must_be_pure_rust(t, "render") && ir_and_core(t).any(|c| c == "render"));
    assert!(must_be_pure_rust(t, "ports") && !ir_and_core(t).any(|c| c == "ports"));

    // Every crate directory has exactly one row, so each rule above reaches it.
    let crates = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    for entry in std::fs::read_dir(&crates).unwrap() {
        let path = entry.unwrap().path();
        if !path.join("Cargo.toml").is_file() {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let rows = TABLE.iter().filter(|(p, _, _)| names(p, &name)).count();
        assert_eq!(rows, 1, "{name} has {rows} rows in the table");
    }

    // A core crate moved to the hosts by its row alone leaves the pure-Rust and the Q-2 sets and
    // may depend on an adapter.
    let moved: Vec<Row> = TABLE
        .iter()
        .map(|&(name, group, edges)| match name {
            "render" => (name, Group::Hosts, edges),
            _ => (name, group, edges),
        })
        .collect();
    assert!(!must_be_pure_rust(&moved, "render"));
    assert!(!ir_and_core(&moved).any(|c| c == "render"));
    assert_eq!(may_depend(&moved, "render", "adapter-geojson"), Some(true));
    assert_eq!(may_depend(&moved, "engine", "render"), Some(false));
}

#[test]
fn only_render_agg_allows_unsafe_code() {
    // Verifies: ADR-0015
    // The workspace denies `unsafe_code`; any mention of the lint in a crate's code or manifest is
    // a way to lift it (`allow`, `expect`, a lint table), so it may appear in render-agg alone,
    // and every crate must take the workspace's lints.
    let crates = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut offenders = Vec::new();
    for entry in std::fs::read_dir(&crates).unwrap() {
        let dir = entry.unwrap().path();
        let name = dir.file_name().unwrap().to_string_lossy().into_owned();
        let manifest = std::fs::read_to_string(dir.join("Cargo.toml")).unwrap_or_default();
        if !manifest.contains("[lints]\nworkspace = true") {
            offenders.push(format!("{name}: does not take the workspace lints"));
        }
        if name == "render-agg" || name == "arch-test" {
            continue;
        }
        let mut stack = vec![dir];
        while let Some(path) = stack.pop() {
            if path.is_dir() {
                stack.extend(std::fs::read_dir(&path).unwrap().map(|e| e.unwrap().path()));
            } else if path.extension().is_some_and(|e| e == "rs" || e == "toml")
                && std::fs::read_to_string(&path)
                    .unwrap_or_default()
                    .contains("unsafe_code")
            {
                offenders.push(path.strip_prefix(&crates).unwrap().display().to_string());
            }
        }
    }
    assert!(offenders.is_empty(), "{offenders:?}");
}

#[test]
fn the_ir_and_the_core_warn_the_lints_that_ban_hash_maps_clocks_and_randomness() {
    // Verifies: Q-2
    // clippy.toml names the banned types and methods and the workspace allows their lints; a crate
    // of the IR or the core turns them on at the top of its lib.rs, which scripts/check-rust.sh
    // shows to bite on a fixture. A new core crate without the line would escape the ban.
    let crates = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let line = "#![warn(clippy::disallowed_types, clippy::disallowed_methods)]";
    let missing: Vec<_> = ir_and_core(TABLE)
        .filter(|name| crates.join(name).is_dir())
        .filter(|name| {
            !std::fs::read_to_string(crates.join(name).join("src/lib.rs"))
                .unwrap_or_default()
                .lines()
                .any(|l| l == line)
        })
        .collect();
    assert!(
        missing.is_empty(),
        "core crates without `{line}`: {missing:?}"
    );
}

/// The one place a core crate lifts the ban, with its reason: a test's time bound.
const LIFTED: &[(&str, &str)] = &[(
    "render-agg/src/lib.rs",
    "    #[expect(
        clippy::disallowed_types,
        clippy::disallowed_methods,
        reason = \"the clock bounds the test's run time and reaches no output (Q-2)\"
    )]",
)];

#[test]
fn no_core_crate_lifts_the_ban_on_hash_maps_clocks_and_randomness() {
    // Verifies: Q-2
    // Any other mention of the two lints in a crate of the IR or the core is a way to lift them
    // (`allow`, `expect`, a lint table), as for `unsafe_code`; the opt-in line and the places named
    // in LIFTED are the only ones.
    let crates = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let line = "#![warn(clippy::disallowed_types, clippy::disallowed_methods)]";
    let mut offenders = Vec::new();
    for name in ir_and_core(TABLE).filter(|name| crates.join(name).is_dir()) {
        let mut stack = vec![crates.join(name)];
        while let Some(path) = stack.pop() {
            if path.is_dir() {
                stack.extend(std::fs::read_dir(&path).unwrap().map(|e| e.unwrap().path()));
                continue;
            }
            if !path.extension().is_some_and(|e| e == "rs" || e == "toml") {
                continue;
            }
            let relative = path.strip_prefix(&crates).unwrap().display().to_string();
            let mut text = std::fs::read_to_string(&path).unwrap_or_default();
            for (file, block) in LIFTED {
                if relative == *file {
                    text = text.replacen(block, "", 1);
                }
            }
            for (number, text) in text.lines().enumerate() {
                if (text.contains("disallowed_types") || text.contains("disallowed_methods"))
                    && !(relative.ends_with("src/lib.rs") && text == line)
                {
                    offenders.push(format!("{relative}:{}", number + 1));
                }
            }
        }
    }
    assert!(offenders.is_empty(), "{offenders:?}");
}

#[test]
fn only_poll_once_polls_with_a_no_op_waker() {
    // A future polled with a waker that wakes nothing is run in one place, `poll_once` of the
    // ports, so that every host and test treats a port that waits alike. The needle is built so
    // that this file holds none, and the ports hold exactly the one in `poll_once`.
    let needle = concat!("Waker", "::noop");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut found = Vec::new();
    let mut stack = vec![root.join("crates"), root.join("fuzz")];
    while let Some(path) = stack.pop() {
        if path.is_dir() {
            if !path.ends_with("target") {
                stack.extend(std::fs::read_dir(&path).unwrap().map(|e| e.unwrap().path()));
            }
        } else if path.extension().is_some_and(|e| e == "rs") {
            let count = std::fs::read_to_string(&path)
                .unwrap_or_default()
                .matches(needle)
                .count();
            for _ in 0..count {
                found.push(path.strip_prefix(&root).unwrap().display().to_string());
            }
        }
    }
    assert_eq!(
        found,
        ["crates/ports/src/lib.rs"],
        "{needle} outside poll_once"
    );
}
