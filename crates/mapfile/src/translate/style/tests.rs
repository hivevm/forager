use forager_style_ir::Bound;

use crate::test_support::{binding, codes, first_style, load_files};

#[test]
fn style_properties_bind_to_attributes() {
    // Verifies: ADR-0019
    let (loaded, style) = first_style(
        "MAP LAYER CLASS STYLE
           SIZE [VAL] WIDTH [w] ANGLE [a] COLOR [c] OUTLINECOLOR [o]
         END END END END",
    );
    assert_eq!(codes(&loaded), Vec::<&str>::new());
    let style = style.unwrap();
    assert_eq!(style.size, binding("VAL"));
    assert_eq!(style.width, binding("w"));
    assert_eq!(style.angle, binding("a"));
    assert_eq!(style.color, binding("c"));
    assert_eq!(style.outline_color, binding("o"));
}

#[test]
fn a_size_expression_is_evaluated_as_mapserver_evaluates_it() {
    // Verifies: ADR-0019
    let (loaded, style) = first_style("MAP LAYER CLASS STYLE SIZE ([a] * 2) END END END END");
    assert_eq!(codes(&loaded), Vec::<&str>::new());
    assert_eq!(
        serde_json::to_string(&style.unwrap().size).unwrap(),
        concat!(
            r#"{"op":"to_number","args":[{"op":"tostring","args":["#,
            r#"{"op":"*","args":[{"op":"to_number","args":[{"property":"a"}]},2.0]},"%g"]}]}"#
        )
    );
    let (loaded, style) = first_style("MAP LAYER CLASS STYLE SIZE ([a] =) END END END END");
    assert_eq!(codes(&loaded), ["invalid-expression"]);
    assert_eq!(style.unwrap().size, Some(Bound::Value(0.0)));
}

#[test]
fn bound_style_properties_read_back_equal_from_yaml_and_json() {
    // Verifies: ADR-0019
    let loaded = load_files(&[(
        "m.map",
        "MAP LAYER CLASS STYLE
           SIZE ([a] + 1) WIDTH [w] ANGLE -30 COLOR [c] OUTLINECOLOR '#102030'
         END END END END",
    )]);
    assert_eq!(codes(&loaded), Vec::<&str>::new());
    let document = loaded.document.unwrap();
    let yaml = document.to_yaml().unwrap();
    let json = document.to_json().unwrap();
    assert_eq!(
        forager_style_ir::StyleDocument::from_yaml(&yaml)
            .unwrap()
            .document,
        document
    );
    assert_eq!(
        forager_style_ir::StyleDocument::from_json(&json)
            .unwrap()
            .document,
        document
    );
}

#[test]
fn every_style_property_that_renders_is_carried_over() {
    use forager_style_ir::{GeomTransform, LineCap, LineJoin, SymbolRef};
    let (loaded, style) = first_style(
        "MAP SYMBOL NAME 'circle' TYPE ELLIPSE POINTS 1 1 END END LAYER CLASS STYLE
           SYMBOL 'circle' ANGLE AUTO OFFSET 2 [dy] POLAROFFSET 5 45 GAP -20 INITIALGAP 4
           PATTERN 10 5 END LINECAP BUTT LINEJOIN MITER LINEJOINMAXSIZE 2 OPACITY 150
           OUTLINEWIDTH 1 MINSIZE 2 MAXSIZE 40 MINWIDTH 0.5 MAXWIDTH 8 GEOMTRANSFORM 'vertices'
         END END END END",
    );
    assert_eq!(codes(&loaded), Vec::<&str>::new());
    let style = style.unwrap();
    assert_eq!(style.symbol, Some(Bound::Value(SymbolRef::Index(1))));
    assert!(style.auto_angle);
    assert_eq!(
        style.offset,
        Some([Bound::Value(2.0), binding("dy").unwrap()])
    );
    assert_eq!(
        style.polar_offset,
        Some([Bound::Value(5.0), Bound::Value(45.0)])
    );
    assert_eq!((style.gap, style.initial_gap), (Some(-20.0), Some(4.0)));
    assert_eq!(style.pattern, [10.0, 5.0]);
    assert_eq!(style.line_cap, Some(LineCap::Butt));
    assert_eq!(style.line_join, Some(LineJoin::Miter));
    assert_eq!(style.line_join_max_size, Some(2.0));
    assert_eq!(
        style.opacity,
        Some(Bound::Value(100.0)),
        "clamped as loadStyle clamps"
    );
    assert_eq!(style.outline_width, Some(Bound::Value(1.0)));
    assert_eq!((style.min_size, style.max_size), (Some(2.0), Some(40.0)));
    assert_eq!((style.min_width, style.max_width), (Some(0.5), Some(8.0)));
    assert_eq!(style.geom_transform, Some(GeomTransform::Vertices));

    let (_, style) = first_style("MAP LAYER CLASS STYLE SYMBOL 2 OPACITY 50.7 END END END END");
    let style = style.unwrap();
    assert_eq!(style.symbol, Some(Bound::Value(SymbolRef::Index(2))));
    assert_eq!(style.opacity, Some(Bound::Value(50.0)));
}

#[test]
fn an_offset_of_minus_99_is_a_line_offset() {
    use forager_style_ir::LineOffset;
    let offset = |text: &'static str| first_style(text).1.unwrap();
    let style = offset("MAP LAYER CLASS STYLE OFFSET 3 -99 END END END END");
    assert_eq!(style.offset, None);
    assert_eq!(
        style.line_offset,
        Some(LineOffset {
            distance: Bound::Value(3.0),
            both_sides: false
        })
    );
    let style = offset("MAP LAYER CLASS STYLE OFFSET [d] -999 END END END END");
    assert_eq!(
        style.line_offset,
        Some(LineOffset {
            distance: binding("d").unwrap(),
            both_sides: true
        })
    );
}

#[test]
fn style_values_outside_loadstyles_checks_are_errors() {
    for text in [
        "MAP LAYER CLASS STYLE INITIALGAP -1 END END END END",
        "MAP LAYER CLASS STYLE MAXSIZE 0 END END END END",
        "MAP LAYER CLASS STYLE MAXWIDTH 0 END END END END",
        "MAP LAYER CLASS STYLE LINEJOINMAXSIZE 0 END END END END",
        "MAP LAYER CLASS STYLE LINECAP WAVY END END END END",
        "MAP LAYER CLASS STYLE PATTERN 10 END END END END END",
        "MAP LAYER CLASS STYLE PATTERN 1 2 3 4 5 6 7 8 9 10 11 END END END END END",
        "MAP LAYER CLASS STYLE SYMBOL -1 END END END END",
        "MAP LAYER CLASS STYLE OFFSET 1 END END END END",
        "MAP LAYER SIZEUNITS parsecs END END",
        "MAP RESOLUTION 5 END",
    ] {
        let loaded = load_files(&[("m.map", text)]);
        assert!(loaded.document.is_none(), "{text}");
        assert_eq!(codes(&loaded), ["invalid-value"], "{text}");
    }
    let (_, style) = first_style("MAP LAYER CLASS STYLE GEOMTRANSFORM VERTICES END END END END");
    assert_eq!(
        style.unwrap().geom_transform,
        Some(forager_style_ir::GeomTransform::Vertices)
    );
    let (loaded, style) = first_style("MAP LAYER CLASS STYLE GEOMTRANSFORM bbox END END END END");
    assert_eq!(codes(&loaded), ["unsupported-value"]);
    assert_eq!(style.unwrap().geom_transform, None);
}
