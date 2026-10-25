use forager_style_ir::Bound;

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
