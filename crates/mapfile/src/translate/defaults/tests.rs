// The tests pin MapServer's msApplyDefaultSubstitutions, src/mapfile.c, of the release pinned in
// scripts/fetch-msautotest.sh.
// Copyright (c) 1996-2005 Regents of the University of Minnesota.
// Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
// associated documentation files (the "Software"), to deal in the Software without restriction,
// including without limitation the rights to use, copy, modify, merge, publish, distribute,
// sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions: The above copyright notice and this
// permission notice shall be included in all copies of this Software or works derived from this
// Software. THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED,
// INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE
// AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

use forager_style_ir::Expr;

use crate::Loaded;
use crate::MAX_DEFAULT_BYTES;
use crate::test_support::{codes, json, load_files};

fn load(text: &str) -> Loaded {
    let text: &'static str = text.to_owned().leak();
    load_files(&[("m.map", text)])
}

fn filter(text: &str) -> (Vec<String>, Option<Expr>) {
    let loaded = load(text);
    let codes = codes(&loaded).into_iter().map(str::to_owned).collect();
    let filter = loaded.document.as_ref().unwrap().map.layers[0]
        .filter
        .clone();
    (codes, filter)
}

/// That `tree` is the condition of a filter written as `written`, one that can match.
fn is_as_written(tree: &Option<Expr>, written: &str) {
    let expected = filter(&format!("MAP LAYER FILTER {written} END END")).1;
    assert!(
        expected.is_some() && expected != Some(Expr::Bool(false)),
        "{written}"
    );
    assert_eq!(json(tree), json(&expected), "{written}");
}

fn class_expression(text: &str) -> Option<Expr> {
    let loaded = load(text);
    loaded.document.as_ref().unwrap().map.layers[0].classes[0]
        .expression
        .clone()
}

#[test]
fn a_filter_takes_the_default_of_its_layers_validation() {
    let (with_default, tree) =
        filter(r#"MAP LAYER VALIDATION "default_v" "3" END FILTER ([v] = %v%) END END"#);
    assert_eq!(with_default, Vec::<String>::new());
    assert_eq!(
        json(&tree),
        r#"{"op":"=","args":[{"op":"to_number","args":[{"property":"v"}]},3.0]}"#
    );
    is_as_written(&tree, "([v] = 3)");
}

#[test]
fn a_validation_after_the_filter_holds_as_well() {
    let (codes, tree) =
        filter(r#"MAP LAYER FILTER ([v] = %v%) VALIDATION "default_v" "3" END END END"#);
    assert_eq!(codes, Vec::<String>::new());
    assert_eq!(
        json(&tree),
        json(&filter("MAP LAYER FILTER ([v] = 3) END END").1)
    );
}

#[test]
fn a_name_without_a_default_stays_the_warning_it_is_and_never_matches() {
    let (with_other, tree) =
        filter(r#"MAP LAYER VALIDATION "default_v" "3" END FILTER ([v] = %w%) END END"#);
    let (written, _) = filter("MAP LAYER FILTER ([v] = %w%) END END");
    assert_eq!(with_other, written);
    assert_eq!(with_other, ["invalid-expression"]);
    assert_eq!(tree, Some(Expr::Bool(false)));
}

#[test]
fn a_default_that_does_not_parse_is_that_expressions_warning() {
    let (with_default, tree) =
        filter(r#"MAP LAYER VALIDATION "default_v" "3 +" END FILTER ([v] = %v%) END END"#);
    let (written, _) = filter("MAP LAYER FILTER ([v] = 3 +) END END");
    assert_eq!(with_default, written);
    assert_eq!(with_default, ["invalid-expression"]);
    assert_eq!(tree, Some(Expr::Bool(false)));
}

#[test]
fn the_key_and_the_tag_are_read_without_case_and_every_tag_is_replaced() {
    let (codes, tree) = filter(
        r#"MAP LAYER VALIDATION "DEFAULT_Value" "3" END FILTER (%vALUE% = %value%) END END"#,
    );
    assert_eq!(codes, Vec::<String>::new());
    assert_eq!(
        json(&tree),
        json(&filter("MAP LAYER FILTER (3 = 3) END END").1)
    );
}

#[test]
fn the_last_default_of_a_name_is_the_one_kept() {
    let (_, tree) = filter(
        r#"MAP LAYER VALIDATION "default_v" "3" "DEFAULT_V" "4" END FILTER ([v] = %v%) END END"#,
    );
    assert_eq!(
        json(&tree),
        json(&filter("MAP LAYER FILTER ([v] = 4) END END").1)
    );
}

#[test]
fn a_key_that_is_no_default_gives_no_value() {
    let (codes, tree) =
        filter(r#"MAP LAYER VALIDATION "v" "^[0-9]+$" END FILTER ([v] = %v%) END END"#);
    assert_eq!(codes, ["invalid-expression"]);
    assert_eq!(tree, Some(Expr::Bool(false)));
}

#[test]
fn a_string_a_regular_expression_and_a_list_take_the_default_too() {
    for (written, plain) in [("'%v%'", "'3'"), ("/^%v%$/", "/^3$/"), ("{%v%,4}", "{3,4}")] {
        let of = |value: &str| {
            format!(
                r#"MAP LAYER FILTERITEM 'a' VALIDATION "default_v" "3" END FILTER {value} END END"#
            )
        };
        let (codes, tree) = filter(&of(written));
        assert_eq!(codes, Vec::<String>::new(), "{written}");
        assert_eq!(json(&tree), json(&filter(&of(plain)).1), "{written}");
    }
}

#[test]
fn a_class_takes_its_own_default_before_its_layers_before_the_maps() {
    let text = r#"MAP WEB VALIDATION "default_a" "1" "default_b" "1" "default_c" "1" END END
        LAYER VALIDATION "default_a" "2" "default_b" "2" END
          CLASS VALIDATION "default_a" "3" END EXPRESSION ([x] = %a% + %b% * %c%) END
        END END"#;
    assert_eq!(
        json(&class_expression(text)),
        json(&class_expression(
            "MAP LAYER CLASS EXPRESSION ([x] = 3 + 2 * 1) END END END"
        ))
    );
}

#[test]
fn a_classs_default_does_not_reach_the_filter_and_the_maps_does() {
    let (_, unreached) =
        filter(r#"MAP LAYER FILTER ([v] = %v%) CLASS VALIDATION "default_v" "3" END END END END"#);
    assert_eq!(unreached, Some(Expr::Bool(false)));
    let (codes, reached) =
        filter(r#"MAP WEB VALIDATION "default_v" "3" END END LAYER FILTER ([v] = %v%) END END"#);
    assert!(
        !codes.contains(&"invalid-expression".to_owned()),
        "{codes:?}"
    );
    is_as_written(&reached, "([v] = 3)");
}

#[test]
fn a_value_of_a_class_naming_a_layers_default_takes_it_in_the_layers_pass() {
    let text = r#"MAP LAYER VALIDATION "default_b" "2" END
        CLASS VALIDATION "default_a" "%b%" END EXPRESSION ([x] = %a%) END END END"#;
    assert_eq!(
        json(&class_expression(text)),
        json(&class_expression(
            "MAP LAYER CLASS EXPRESSION ([x] = 2) END END END"
        ))
    );
}

#[test]
fn a_labels_expression_takes_no_default() {
    let loaded = load(
        r#"MAP LAYER VALIDATION "default_v" "3" END
           CLASS LABEL EXPRESSION ([v] = %v%) END END END END"#,
    );
    let label = &loaded.document.as_ref().unwrap().map.layers[0].classes[0].labels[0];
    assert_eq!(label.expression, Some(Expr::Bool(false)));
}

#[test]
fn a_filter_that_defaults_multiply_past_the_limit_fails_the_load() {
    // Verifies: Q-9
    let value = "x".repeat(1024 * 1024);
    let tags = "%v%".repeat(MAX_DEFAULT_BYTES / value.len() + 1);
    let loaded = load(&format!(
        r#"MAP LAYER VALIDATION "default_v" "{value}" END FILTER ({tags}) END END"#
    ));
    assert_eq!(codes(&loaded), ["mapfile-too-large"]);
    assert!(loaded.document.is_none());
}

#[test]
fn the_text_after_the_last_tag_counts_toward_the_limit() {
    // Verifies: Q-9
    let value = "x".repeat(600 * 1024);
    let tail = "y".repeat(15 * 1024 * 1024);
    let loaded = load(&format!(
        r#"MAP LAYER FILTERITEM 'a' VALIDATION "default_v" "{value}" END FILTER '%v%%v%%v%{tail}' END END"#
    ));
    assert_eq!(codes(&loaded), ["mapfile-too-large"]);
    assert!(loaded.document.is_none());
}

#[test]
fn a_default_named_by_many_expressions_counts_each_time_toward_one_limit_reported_once() {
    // Verifies: Q-9
    let value = "x".repeat(1024 * 1024);
    let classes = "CLASS EXPRESSION '%v%' END ".repeat(40);
    let loaded = load(&format!(
        r#"MAP LAYER CLASSITEM 'a' VALIDATION "default_v" "{value}" END {classes} END END"#
    ));
    assert_eq!(codes(&loaded), ["mapfile-too-large"]);
    assert!(loaded.document.is_none());
    let classes = "CLASS EXPRESSION '%v%' END ".repeat(16);
    let loaded = load(&format!(
        r#"MAP LAYER CLASSITEM 'a' VALIDATION "default_v" "{value}" END {classes} END END"#
    ));
    assert_eq!(codes(&loaded), Vec::<&str>::new());
}

/// A `%` that opens no tag leaves the `%` after it free to open one, as msCaseReplaceSubstring,
/// looking for the whole tag, finds it wherever it stands.
#[test]
fn a_percent_that_opens_no_tag_leaves_the_next_free_to() {
    let (codes, tree) =
        filter(r#"MAP LAYER VALIDATION "default_v" "3" END FILTER ([a] % 2 = %v%) END END"#);
    assert_eq!(codes, Vec::<String>::new());
    is_as_written(&tree, "([a] % 2 = 3)");
    let of = |value: &str| {
        format!(
            r#"MAP LAYER FILTERITEM 'a' VALIDATION "default_v" "3" END FILTER '{value}' END END"#
        )
    };
    assert_eq!(json(&filter(&of("%x%v%")).1), json(&filter(&of("%x3")).1));
    assert!(json(&filter(&of("%x%v%")).1).contains("%x3"));
}

#[test]
fn a_percent_never_closed_stays_as_written() {
    let of = |value: &str| {
        format!(
            r#"MAP LAYER FILTERITEM 'a' VALIDATION "default_v" "3" END FILTER '{value}' END END"#
        )
    };
    let (codes, tree) = filter(&of("%v"));
    assert_eq!(codes, Vec::<String>::new());
    assert!(json(&tree).contains(r#""%v""#), "{}", json(&tree));
}

/// `WEB` is read for its `VALIDATION`, so a load reports only what of `WEB` it leaves out.
#[test]
fn a_web_block_warns_only_of_what_it_leaves_out() {
    let validation = r#"VALIDATION "default_x" "1" END"#;
    let layer = "LAYER FILTER ([v] = %x%) END";
    let (codes, tree) = filter(&format!("MAP WEB {validation} END {layer} END"));
    assert_eq!(codes, Vec::<String>::new());
    is_as_written(&tree, "([v] = 1)");
    let loaded = load(&format!(
        "MAP WEB {validation} TEMPLATE 'x.html' IMAGEPATH '/tmp/' METADATA 'a' 'b' END END \
         {layer} END"
    ));
    let warned: Vec<(&str, &str)> = loaded
        .diagnostics
        .iter()
        .map(|d| (d.code.as_str(), d.message.as_str()))
        .collect();
    assert_eq!(
        warned,
        [
            (
                "discarded-keyword",
                "TEMPLATE of WEB is out of Forager's scope and is discarded"
            ),
            (
                "discarded-keyword",
                "IMAGEPATH of WEB is out of Forager's scope and is discarded"
            ),
            (
                "unsupported-keyword",
                "METADATA of WEB is read for labelcache_map_edge_buffer alone; its other keys are not yet \
                 supported and are ignored"
            ),
        ]
    );
}

/// A pair `loadHashTable` fails the load on gives no default and is a warning, and the block's
/// other pairs still hold (the maintainer's choice of 2026-11-11).
#[test]
fn a_pair_that_is_not_two_strings_is_a_warning_and_the_others_hold() {
    for (validation, named) in [
        (r#""default_a" "1" "default_b""#, r#""default_b""#),
        (r#""default_b" 2 "default_a" "1""#, r#""default_b""#),
        (r#"3 "x" "default_a" "1""#, "3"),
    ] {
        let loaded = load(&format!(
            "MAP LAYER VALIDATION {validation} END FILTER ([v] = %a%) END END"
        ));
        assert_eq!(codes(&loaded), ["unsupported-value"], "{validation}");
        let message = &loaded.diagnostics[0].message;
        assert!(message.contains(named), "{validation}: {message}");
        let tree = &loaded.document.as_ref().unwrap().map.layers[0].filter;
        is_as_written(tree, "([v] = 1)");
    }
}

#[test]
fn a_pair_skipped_in_web_or_a_class_is_a_warning_too() {
    for text in [
        r#"MAP WEB VALIDATION "default_b" END END LAYER END END"#,
        r#"MAP LAYER CLASS VALIDATION "default_b" END END END END"#,
    ] {
        assert_eq!(codes(&load(text)), ["unsupported-value"], "{text}");
    }
}
