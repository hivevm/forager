//! The style document readers on arbitrary text, with and without a migration step from an older
//! major: a document or a diagnostic, never a panic (Q-3).
// Verifies: Q-3
#![no_main]

use forager_style_ir::{Step, StyleDocument};
use libfuzzer_sys::fuzz_target;
use serde_json::Value;

/// A step from the test-only major 0 that changes nothing, so that a `0.x` document runs through
/// the chain and is then read as the current version.
const STEPS: &[Step] = &[Step {
    from_major: 0,
    migrate: Ok::<Value, String>,
}];

fuzz_target!(|text: &str| {
    let _ = StyleDocument::from_yaml(text);
    let _ = StyleDocument::from_json(text);
    let _ = StyleDocument::from_yaml_with(text, STEPS);
    let _ = StyleDocument::from_json_with(text, STEPS);
});
