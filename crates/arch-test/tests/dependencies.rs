//! The shape of the code (ADR-0009): the dependency edges allowed between workspace crates, and
//! no crate that links C code under the IR, the core, the ports, or the frontend (Q-1).

use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;

use serde_json::Value;

/// The hosts, which may depend on every crate but hosts and tooling, adapters included.
const HOSTS: &[&str] = &["cli", "bindings-wasm", "dev-server", "bindings-wit"];

/// Whether `from` may depend on `to`, by crate name without the `forager-` prefix; `None` when
/// ADR-0009 does not name `from`.
fn may_depend(from: &str, to: &str) -> Option<bool> {
    if HOSTS.contains(&from) {
        return Some(!HOSTS.contains(&to) && to != "arch-test");
    }
    Some(allowed(from)?.contains(&to))
}

/// What a crate other than a host may depend on.
fn allowed(from: &str) -> Option<&'static [&'static str]> {
    const CORE: &[&str] = &[
        "style-ir",
        "expr",
        "proj",
        "render",
        "render-agg",
        "encode",
        "symbolize",
        "label",
        "raster",
        "ports",
    ];
    Some(match from {
        "style-ir" | "arch-test" => &[],
        "expr" | "proj" | "render" | "ports" => &["style-ir"],
        "render-agg" => &["style-ir", "render"],
        "encode" => &["render"],
        "symbolize" | "label" | "raster" => &["style-ir", "expr", "proj", "render"],
        "engine" => CORE,
        "mapfile" => &["style-ir", "ports"],
        adapter if adapter.starts_with("adapter-") => &["style-ir", "ports"],
        _ => return None,
    })
}

/// The crates that must link no C code, directly or transitively.
fn must_be_pure_rust(name: &str) -> bool {
    matches!(
        name,
        "style-ir"
            | "expr"
            | "proj"
            | "render"
            | "render-agg"
            | "encode"
            | "symbolize"
            | "label"
            | "raster"
            | "engine"
            | "ports"
            | "mapfile"
    )
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
        if may_depend(from, "style-ir").is_none() {
            violations.push(format!("{from} is not a crate ADR-0009 names"));
            continue;
        }
        for dep in &ws.deps[id] {
            if let Some(to) = ws.members.get(dep)
                && may_depend(from, to) != Some(true)
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
    for (id, name) in ws.members.iter().filter(|(_, n)| must_be_pure_rust(n)) {
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

#[test]
fn the_rules_reject_a_wrong_edge_and_an_unnamed_crate() {
    // The table itself, so that a test that never fails cannot pass for a check.
    assert!(!allowed("style-ir").unwrap().contains(&"ports"));
    assert!(!allowed("render").unwrap().contains(&"expr"));
    assert!(!allowed("mapfile").unwrap().contains(&"engine"));
    assert!(!allowed("adapter-geojson").unwrap().contains(&"engine"));
    assert!(allowed("engine").unwrap().contains(&"ports"));
    assert!(allowed("server").is_none());
    assert_eq!(may_depend("cli", "adapter-geojson"), Some(true));
    assert_eq!(may_depend("cli", "bindings-wasm"), Some(false));
    assert_eq!(may_depend("bindings-wasm", "arch-test"), Some(false));
    assert!(must_be_pure_rust("mapfile") && !must_be_pure_rust("bindings-wasm"));
}

#[test]
fn only_render_agg_allows_unsafe_code() {
    // Verifies: ADR-0015
    let crates = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut offenders = Vec::new();
    let mut stack = vec![crates.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let relative = path.strip_prefix(&crates).unwrap().to_owned();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs" || e == "toml")
                && !relative.starts_with("render-agg")
                && !relative.starts_with("arch-test")
            {
                let text = std::fs::read_to_string(&path).unwrap();
                if text.contains("allow(unsafe_code)") || text.contains("unsafe_code = \"allow\"") {
                    offenders.push(relative.display().to_string());
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "unsafe code allowed outside render-agg: {offenders:?}"
    );
}
