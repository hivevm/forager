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
use crate::MAX_TOTAL_BYTES;
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
    let (written, expected) = filter("MAP LAYER FILTER ([v] = 3) END END");
    assert_eq!(with_default, written);
    assert_eq!(json(&tree), json(&expected));
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
    assert!(!with_other.is_empty());
    assert_eq!(tree, Some(Expr::Bool(false)));
}

#[test]
fn a_default_that_does_not_parse_is_that_expressions_warning() {
    let (with_default, tree) =
        filter(r#"MAP LAYER VALIDATION "default_v" "3 +" END FILTER ([v] = %v%) END END"#);
    let (written, _) = filter("MAP LAYER FILTER ([v] = 3 +) END END");
    assert_eq!(with_default, written);
    assert!(!with_default.is_empty());
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
    assert!(!codes.is_empty());
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
    assert_eq!(
        json(&reached),
        json(&filter("MAP LAYER FILTER ([v] = 3) END END").1)
    );
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
    let value = "x".repeat(1024 * 1024);
    let tags = "%v%".repeat(MAX_TOTAL_BYTES / value.len() + 1);
    let loaded = load(&format!(
        r#"MAP LAYER VALIDATION "default_v" "{value}" END FILTER ({tags}) END END"#
    ));
    assert_eq!(codes(&loaded), ["mapfile-too-large"]);
    assert!(loaded.document.is_none());
}
