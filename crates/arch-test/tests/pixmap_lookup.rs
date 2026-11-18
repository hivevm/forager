//! A pixmap's image is looked up in one place of `symbolize`, `SymbolResources::pixmap` in
//! `symbol.rs`, which the canvas draws with and `marker_size_with` sizes a marker with: the texts
//! of why a pixmap has no image stand once in the crate's code outside its tests, so that a copy
//! of the lookup, as the canvas held one until revision-2026-11-16/13, is refused.

#[expect(
    dead_code,
    reason = "this check reads only the code outside tests of the shared reading"
)]
mod module_direction;

use forager_arch_test::{manifest_dir, rust_files};
use module_direction::code_outside_tests;

/// The texts of why a pixmap has no image.
const TEXTS: [&str; 2] = ["names no image", "is not loaded for this render"];

/// Each of `TEXTS` and the times it stands in `sources`, each source's code outside its tests.
fn counts<'s>(sources: impl IntoIterator<Item = &'s str>) -> [(&'static str, usize); 2] {
    let code: Vec<String> = sources.into_iter().map(code_outside_tests).collect();
    TEXTS.map(|text| (text, code.iter().map(|c| c.matches(text).count()).sum()))
}

#[test]
fn a_pixmaps_lookup_is_written_once_in_symbolize() {
    let src = manifest_dir().join("../symbolize/src");
    let sources: Vec<String> = rust_files(&src)
        .iter()
        .filter(|file| file.file_name().is_some_and(|name| name != "tests.rs"))
        .map(|file| std::fs::read_to_string(file).unwrap())
        .collect();
    assert!(
        !sources.is_empty(),
        "no source read under {}",
        src.display()
    );
    assert_eq!(
        counts(sources.iter().map(String::as_str)),
        [("names no image", 1), ("is not loaded for this render", 1)]
    );
}

#[test]
fn the_check_counts_a_text_in_code_and_not_in_tests_or_comments() {
    let source = "fn a() { \"the pixmap symbol names no image\" }\n\
                  // names no image\n\
                  #[cfg(test)]\n\
                  mod tests {\n    \"is not loaded for this render\"\n}\n";
    assert_eq!(
        counts([source, source]),
        [("names no image", 2), ("is not loaded for this render", 0)]
    );
}
