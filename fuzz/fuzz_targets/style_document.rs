//! The style document readers on arbitrary text: a document or a diagnostic, never a panic
//! (Q-3).
// Verifies: Q-3
#![no_main]

use forager_style_ir::StyleDocument;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|text: &str| {
    let _ = StyleDocument::from_yaml(text);
    let _ = StyleDocument::from_json(text);
});
