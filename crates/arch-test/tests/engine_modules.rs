//! The modules of the engine import one another in one direction only, as its `lib.rs` states:
//! ADR-0008 decides the direction between crates, and this test holds the one inside the crate,
//! which `labels` and `label_styles` once broke by importing each other, and its modules by
//! taking helpers from the crate's root.

mod module_direction;

use std::path::Path;

/// Each module of the crate and the modules it may import, in the order of `lib.rs`'s sentence.
const ALLOWED: &[(&str, &[&str])] = &[
    (
        "labels",
        &[
            "classes",
            "fonts",
            "inline",
            "label_binding",
            "label_cache",
            "label_styles",
            "layer",
        ],
    ),
    (
        "label_styles",
        &["bindable", "classes", "label_cache", "layer"],
    ),
    ("label_binding", &["classes"]),
    ("classes", &["bindable", "layer"]),
    ("fonts", &["resource"]),
    ("images", &["resource"]),
    ("bind", &[]),
    ("bindable", &[]),
    ("inline", &[]),
    ("label_cache", &[]),
    ("layer", &[]),
    ("plan", &[]),
    ("resource", &[]),
];

/// The sources of the crate, from the workspace root.
const SOURCES: &str = "crates/engine/src";

#[test]
fn the_modules_of_the_engine_import_one_another_in_one_direction() {
    let problems = module_direction::problems(SOURCES, ALLOWED);
    assert!(
        problems.is_empty(),
        "against the direction of crates/engine/src/lib.rs: {problems:#?}"
    );
}

#[test]
fn an_import_of_labels_in_label_styles_is_refused_naming_both() {
    let (_, allowed) = ALLOWED.iter().find(|(m, _)| *m == "label_styles").unwrap();
    let source = "use crate::classes;\nuse crate::labels::Labels;\n";
    assert_eq!(
        module_direction::imports_against(
            Path::new("label_styles.rs"),
            "label_styles",
            allowed,
            source
        ),
        ["label_styles.rs: the module label_styles imports labels"]
    );
}

#[test]
fn an_item_of_the_root_is_refused_as_no_module() {
    let source = "fn f() { crate::draw_items(r, 0, []); }\n";
    assert_eq!(
        module_direction::imports_against(Path::new("layer.rs"), "layer", &[], source),
        ["layer.rs: the module layer imports draw_items"]
    );
}

#[test]
fn a_glob_of_the_root_outside_tests_is_refused() {
    let source = "use crate::*;\n#[cfg(test)]\nmod tests {\n    use super::*;\n}\n";
    assert_eq!(
        module_direction::imports_against(Path::new("layer.rs"), "layer", &[], source),
        ["layer.rs: the module layer imports * (every item of the root)"]
    );
}
