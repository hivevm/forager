//! The JSON Schema of the current format version, derived from the IR's types, is the one
//! checked in under `schema/` (ADR-0020). `FORAGER_WRITE_SCHEMA=1` writes it instead of
//! comparing, the command the failure names.
#![cfg(feature = "schema")]

use std::path::PathBuf;

use forager_style_ir::{FORMAT_VERSION, StyleDocument};

fn derived() -> String {
    let schema = schemars::schema_for!(StyleDocument);
    serde_json::to_string_pretty(&schema).unwrap() + "\n"
}

fn checked_in() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../schema")
        .join(format!("style-document-{FORMAT_VERSION}.json"))
}

#[test]
fn the_checked_in_schema_is_the_one_the_ir_derives() {
    // Verifies: ADR-0020
    let derived = derived();
    if std::env::var_os("FORAGER_WRITE_SCHEMA").is_some() {
        std::fs::write(checked_in(), &derived).unwrap();
        return;
    }
    let file = std::fs::read_to_string(checked_in()).unwrap_or_default();
    assert!(
        file == derived,
        "{} is not the schema the IR derives; regenerate it with\n  \
         FORAGER_WRITE_SCHEMA=1 cargo test -p forager-style-ir --features schema --test schema",
        checked_in().display()
    );
}
