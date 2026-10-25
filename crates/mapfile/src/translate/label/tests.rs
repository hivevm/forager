// The tests pin MapServer's loadLabel and loadClass, src/mapfile.c, msShapeGetLabelAnnotation
// and bindLabel, src/maputil.c, of the release pinned in scripts/fetch-msautotest.sh.
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

use forager_style_ir::{Align, Bound, Expr, MinFeatureSize, Position, Rgba};

use crate::test_support::{binding, codes, first_label, json, load_files};

#[test]
fn a_labels_text_falls_back_on_the_class_then_the_labelitem() {
    let (loaded, label) = first_label(&[(
        "m.map",
        "MAP LAYER LABELITEM 'name' CLASS LABEL END END END END",
    )]);
    assert_eq!(codes(&loaded), Vec::<&str>::new());
    assert_eq!(
        json(&label.unwrap().text),
        r#"{"op":"to_string","args":[{"property":"name"}]}"#
    );
    let (_, label) = first_label(&[(
        "m.map",
        "MAP LAYER LABELITEM 'name' CLASS LABEL END TEXT 'k' END END END",
    )]);
    assert_eq!(json(&label.unwrap().text), r#""k""#);
}

#[test]
fn a_text_is_evaluated_as_mapserver_evaluates_it() {
    // Verifies: ADR-0019
    let (loaded, label) = first_label(&[(
        "m.map",
        r#"MAP LAYER CLASS LABEL TEXT ("Area: " + tostring([area],"%.2f")) END END END END"#,
    )]);
    assert_eq!(codes(&loaded), Vec::<&str>::new());
    assert_eq!(
        json(&label.unwrap().text),
        concat!(
            r#"{"op":"concat","args":["Area: ",{"op":"tostring","args":["#,
            r#"{"op":"to_number","args":[{"property":"area"}]},"%.2f"]}]}"#
        )
    );
    let (_, label) = first_label(&[(
        "m.map",
        "MAP LAYER CLASS LABEL TEXT '[name] ([pop])' END END END END",
    )]);
    assert_eq!(
        json(&label.unwrap().text),
        concat!(
            r#"{"op":"concat","args":[{"op":"to_string","args":[{"property":"name"}]}," (","#,
            r#"{"op":"to_string","args":[{"property":"pop"}]},")"]}"#
        )
    );
    let (_, label) = first_label(&[(
        "m.map",
        "MAP LAYER CLASS LABEL TEXT ([n] * 2) END END END END",
    )]);
    assert_eq!(
        json(&label.unwrap().text),
        r#"{"op":"tostring","args":[{"op":"*","args":[{"op":"to_number","args":[{"property":"n"}]},2.0]},"%g"]}"#
    );
}

#[test]
fn a_label_carries_what_placement_and_drawing_need() {
    use forager_style_ir::{AngleMode, Force, GeomTransform, MinFeatureSize, Position};
    let (loaded, label) = first_label(&[(
        "m.map",
        "MAP LAYER CLASS LABEL
           SIZE giant COLOR 1 2 3 OUTLINECOLOR [oc] OUTLINEWIDTH 2 SHADOWCOLOR 0 0 0
           SHADOWSIZE 1 1 POSITION auto OFFSET 0 -5 ANGLE follow ALIGN center WRAP ' '
           MAXLENGTH 20 BUFFER 2 MINDISTANCE 100 REPEATDISTANCE 300 MINFEATURESIZE auto
           MAXOVERLAPANGLE 30 PARTIALS false FORCE group PRIORITY 7 MINSIZE 4 MAXSIZE 20
           STYLE COLOR 255 255 255 END
           STYLE GEOMTRANSFORM 'labelpoly' END
         END END END END",
    )]);
    assert_eq!(codes(&loaded), Vec::<&str>::new());
    let label = label.unwrap();
    assert_eq!(label.size, Some(Bound::Value(16.0)), "MS_GIANT");
    assert_eq!(label.outline_color, binding("oc"));
    assert_eq!(label.position, Some(Bound::Value(Position::Auto)));
    assert_eq!(label.angle_mode, Some(AngleMode::Follow));
    assert_eq!(label.wrap.as_deref(), Some(" "));
    assert_eq!(label.min_feature_size, Some(MinFeatureSize::Auto));
    assert_eq!(
        (label.partials, label.force),
        (Some(false), Some(Force::Group))
    );
    assert_eq!(label.priority, Some(Bound::Value(7.0)));
    let transforms: Vec<_> = label.styles.iter().map(|s| s.geom_transform).collect();
    assert_eq!(
        transforms,
        [
            Some(GeomTransform::LabelPoint),
            Some(GeomTransform::LabelPoly)
        ],
        "a label's style draws at the label point unless it says otherwise"
    );
    let loaded = load_files(&[("m.map", "MAP LAYER CLASS LABEL PRIORITY 11 END END END END")]);
    assert_eq!(codes(&loaded), ["invalid-value"]);
}

/// The label of `body`, a label's directives, with the codes of the load.
fn label_of(body: &str) -> (Vec<String>, Option<forager_style_ir::Label>) {
    let text: &'static str = format!("MAP LAYER CLASS LABEL {body} END END END END").leak();
    let (loaded, label) = first_label(&[("m.map", text)]);
    (
        codes(&loaded).into_iter().map(str::to_owned).collect(),
        label,
    )
}

/// The codes of a load of `body`, a label's directives.
fn label_codes(body: &str) -> Vec<String> {
    label_of(body).0
}

#[test]
fn each_number_is_checked_as_load_label_checks_it() {
    // msCheckNumber on the number as read; getInteger casts it after the check.
    for (body, ok) in [
        ("SIZE 0", false),
        ("SIZE 0.5", true),
        ("MINSIZE 0", false),
        ("MINSIZE 0.5", true),
        ("MAXSIZE 0", false),
        ("OUTLINEWIDTH 0", false),
        ("MAXLENGTH 0", false),
        ("MINDISTANCE 0", false),
        ("REPEATDISTANCE 0", false),
        ("BUFFER -3", true),
        ("MAXOVERLAPANGLE -0.5", false),
        ("MAXOVERLAPANGLE 360", true),
        ("MAXOVERLAPANGLE 360.5", false),
        ("MINFEATURESIZE 0", false),
        ("ANGLE -360", true),
        ("ANGLE 360.5", false),
        ("PRIORITY 1", true),
        ("PRIORITY 0.5", false),
        ("PRIORITY 10", true),
        ("PRIORITY 10.5", false),
        ("MINSCALEDENOM -1", false),
        ("MAXSCALEDENOM 0", true),
        ("OFFSET -7 300", true),
        ("SHADOWSIZE -2 0", true),
    ] {
        let codes = label_codes(body);
        assert_eq!(codes.is_empty(), ok, "{body}: {codes:?}");
        if !ok {
            assert_eq!(codes, ["invalid-value"], "{body}");
        }
    }
    let (_, label) = label_of("MINSIZE 0.5 BUFFER 2.7 MINFEATURESIZE 0.5 SIZE 7.5");
    let label = label.unwrap();
    assert_eq!(label.min_size, Some(0.0), "cast after the check");
    assert_eq!(label.buffer, Some(2.0));
    assert_eq!(label.min_feature_size, Some(MinFeatureSize::Pixels(0.0)));
    assert_eq!(label.size, Some(Bound::Value(7.5)), "SIZE is a double");
}

#[test]
fn offset_shadow_size_and_priority_are_cast_to_integers() {
    // `(int)msyynumber` in loadLabel's OFFSET, SHADOWSIZE, and PRIORITY.
    let (codes, label) = label_of("OFFSET 1.7 -2.9 SHADOWSIZE 2.5 [s] PRIORITY 9.9");
    assert!(codes.is_empty(), "{codes:?}");
    let label = label.unwrap();
    assert_eq!(label.offset, Some([Bound::Value(1.0), Bound::Value(-2.0)]));
    assert_eq!(
        label.shadow_size,
        Some([Bound::Value(2.0), binding("s").unwrap()])
    );
    assert_eq!(label.priority, Some(Bound::Value(9.0)));
}

#[test]
fn a_later_value_keeps_an_earlier_binding() {
    // loadLabel frees a binding only for another binding; bindLabel draws it over the value.
    let (codes, label) = label_of(
        "ANGLE [a] ANGLE 30 ALIGN [al] ALIGN left COLOR [c] COLOR 1 2 3
         OUTLINECOLOR [oc] OUTLINECOLOR -1 -1 -1 FONT [f] FONT 'sans' POSITION [p] POSITION ul
         OFFSET [x] 3 OFFSET 1 2 SHADOWSIZE 4 [y] SHADOWSIZE 1 1 PRIORITY [pr] PRIORITY 5",
    );
    assert_eq!(codes, ["font-unknown"], "the set has no font sans");
    let label = label.unwrap();
    assert_eq!(label.angle, binding("a"));
    assert_eq!(label.align, binding("al"));
    assert_eq!(label.color, binding("c"));
    assert_eq!(label.outline_color, binding("oc"), "over no colour too");
    assert_eq!(label.font, binding("f"));
    assert_eq!(label.position, binding("p"));
    assert_eq!(
        label.offset,
        Some([binding("x").unwrap(), Bound::Value(2.0)])
    );
    assert_eq!(
        label.shadow_size,
        Some([Bound::Value(1.0), binding("y").unwrap()])
    );
    assert_eq!(label.priority, binding("pr"));
    let (_, label) = label_of(
        "ANGLE 30 ANGLE [a] ALIGN [x] ALIGN [al] ANGLE auto COLOR 1 2 3 COLOR [c] POSITION ul",
    );
    let label = label.unwrap();
    assert_eq!(label.angle, binding("a"), "the keyword sets the mode only");
    assert_eq!(label.align, binding("al"), "a binding replaces a binding");
    assert_eq!(label.color, binding("c"));
    assert_eq!(label.position, Some(Bound::Value(Position::Ul)));
}

#[test]
fn size_frees_its_binding_and_priority_only_its_expression() {
    let size = |body| label_of(body).1.unwrap().size;
    assert_eq!(size("SIZE [s] SIZE 8"), Some(Bound::Value(8.0)));
    assert_eq!(size("SIZE (2) SIZE [s]"), binding("s"));
    assert_eq!(size("SIZE [s] SIZE large"), Some(Bound::Value(13.0)));
    let priority = |body| label_of(body).1.unwrap().priority;
    assert!(matches!(
        priority("PRIORITY [p] PRIORITY ([n] + 1)"),
        Some(Bound::Expr(_))
    ));
    assert_eq!(
        priority("PRIORITY [p] PRIORITY ([n] + 1) PRIORITY 5"),
        binding("p"),
        "the expression is freed, the binding held"
    );
    assert_eq!(
        priority("PRIORITY ([n] + 1) PRIORITY 5"),
        Some(Bound::Value(5.0))
    );
}

#[test]
fn the_keyword_choices_are_those_of_load_label() {
    let (codes, label) = label_of("ALIGN right FORCE true PARTIALS on TYPE bitmap ANTIALIAS 1");
    assert!(codes.is_empty(), "{codes:?}");
    let label = label.unwrap();
    assert_eq!(label.align, Some(Bound::Value(Align::Right)));
    assert_eq!(label.partials, Some(true));
    let (_, label) = label_of("ANGLE auto2 TYPE truetype");
    assert_eq!(
        label.unwrap().angle_mode,
        Some(forager_style_ir::AngleMode::Auto2)
    );
    for size in ["tiny 5", "small 7", "medium 10", "large 13", "giant 16"] {
        let (word, points) = size.split_once(' ').unwrap();
        let (_, label) = label_of(&format!("SIZE {word}"));
        let points: f64 = points.parse().unwrap();
        assert_eq!(label.unwrap().size, Some(Bound::Value(points)), "{word}");
    }
    // getSymbol takes the keyword token only: a string, a number, or another word fails.
    for body in [
        "TYPE outline",
        "ALIGN 'left'",
        "ALIGN justify",
        "POSITION xy",
        "FORCE 1",
        "PARTIALS yes",
        "ANGLE auto3",
        "SIZE huge",
        "WRAP 1",
    ] {
        assert_eq!(label_codes(body), ["invalid-value"], "{body}");
    }
    let (codes, label) = label_of("WRAP ''");
    assert!(codes.is_empty(), "{codes:?}");
    assert_eq!(label.unwrap().wrap, None, "getCharacter's '\\0', no wrap");
}

#[test]
fn only_the_properties_load_label_binds_take_an_attribute() {
    for body in [
        "MINSIZE [m]",
        "MAXSIZE [m]",
        "OUTLINEWIDTH [w]",
        "BUFFER [b]",
        "MAXLENGTH [m]",
        "MINDISTANCE [m]",
        "REPEATDISTANCE [r]",
        "MAXOVERLAPANGLE [a]",
        "MINFEATURESIZE [m]",
        "PARTIALS [p]",
        "FORCE [f]",
        "WRAP [w]",
        "MINSCALEDENOM [s]",
    ] {
        assert_eq!(label_codes(body), ["invalid-value"], "{body}");
    }
    // An expression only where loadLabel's getSymbol takes MS_EXPRESSION.
    assert_eq!(label_codes("ANGLE ([a] * 2)"), ["invalid-value"]);
    assert_eq!(label_codes("OFFSET ([a]) 1"), ["invalid-value"]);
    let (codes, label) = label_of("SIZE ([s] * 2)\n PRIORITY ([p] + 1)");
    assert!(codes.is_empty(), "{codes:?}");
    assert!(matches!(label.unwrap().size, Some(Bound::Expr(_))));
}

#[test]
fn a_text_is_a_string_or_an_expression_only() {
    // loadLabel and loadClass refuse a TEXT of another type, a regex or a list.
    for text in ["/x/", "/x/i", "{a,b}"] {
        assert_eq!(
            label_codes(&format!("TEXT {text}")),
            ["invalid-value"],
            "{text}"
        );
    }
    let (found, label) = label_of(r#"TEXT "Name"i"#);
    assert!(found.is_empty(), "{found:?}");
    assert_eq!(label.unwrap().text, Some(Expr::String("Name".to_owned())));
    // Each class TEXT is read, the one a later TEXT replaces too.
    let loaded = load_files(&[(
        "m.map",
        "MAP LAYER CLASS TEXT /x/ TEXT 'k' LABEL END END END END",
    )]);
    assert_eq!(codes(&loaded), ["invalid-value"]);
}

#[test]
fn a_labels_own_text_wins_even_when_empty() {
    // msShapeGetLabelAnnotation takes the label's text whenever it has a string, "" included.
    let (_, label) = first_label(&[(
        "m.map",
        "MAP LAYER LABELITEM 'name' CLASS TEXT 'k' LABEL TEXT '' END END END END",
    )]);
    assert_eq!(label.unwrap().text, Some(Expr::String(String::new())));
    let (_, label) = first_label(&[(
        "m.map",
        "MAP LAYER CLASS TEXT 'a' TEXT 'b' LABEL END END END END",
    )]);
    assert_eq!(
        label.unwrap().text,
        Some(Expr::String("b".to_owned())),
        "the last class TEXT"
    );
    let (_, label) = first_label(&[("m.map", "MAP LAYER CLASS LABEL END END END END")]);
    assert_eq!(label.unwrap().text, None, "no text, no label");
}

#[test]
fn a_label_colour_is_read_by_load_color() {
    let (codes, label) = label_of("COLOR '#0a0b0c80' OUTLINECOLOR 255 -255 0 SHADOWCOLOR -1 0 0");
    assert!(codes.is_empty(), "{codes:?}");
    let label = label.unwrap();
    assert_eq!(
        label.color,
        Some(Bound::Value(Rgba {
            r: 10,
            g: 11,
            b: 12,
            a: 128
        }))
    );
    assert_eq!(
        label.outline_color,
        Some(Bound::Value(Rgba::opaque(255, 1, 0)))
    );
    assert_eq!(label.shadow_color, None, "a channel of -1 is no colour");
    assert_eq!(label_codes("COLOR 256 0 0"), ["invalid-value"]);
}
