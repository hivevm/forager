// The tests pin the behaviour of MapServer's loadFeature, loadFeaturePoints, the FEATURE case of
// loadLayer, msLayerGetItems, and msINLINELayerNextShape, of the release pinned in
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

use forager_style_ir::{FeatureProperty, InlineFeature, Layer, ShapeType};

use crate::Loaded;
use crate::test_support::{codes, load_files};

fn load_layer(body: &str) -> (Loaded, Option<Layer>) {
    let text: &'static str = format!("MAP LAYER {body} END END").leak();
    let loaded = load_files(&[("m.map", text)]);
    let layer = loaded.document.as_ref().map(|d| d.map.layers[0].clone());
    (loaded, layer)
}

fn features(body: &str) -> Vec<InlineFeature> {
    let (loaded, layer) = load_layer(body);
    assert!(loaded.diagnostics.is_empty(), "{:?}", loaded.diagnostics);
    layer.unwrap().features
}

fn props(pairs: &[(&str, &str)]) -> Vec<FeatureProperty> {
    pairs
        .iter()
        .map(|(name, value)| FeatureProperty {
            name: (*name).to_owned(),
            value: (*value).to_owned(),
        })
        .collect()
}

#[test]
fn inline_features_carry_their_shape_properties_and_text() {
    // Verifies: ADR-0033
    let (loaded, layer) = load_layer(
        r#"NAME "l" TYPE POLYGON PROCESSING "ITEMS=name,size"
           FEATURE POINTS 0 0 10 0 10 10 0 0 END POINTS 2 2 3 2 3 3 2 2 END
             ITEMS "a;7" TEXT "first" END
           FEATURE WKT "LINESTRING(1 2, 3 4)" ITEMS "b" END
           FEATURE ITEMS "x;y" WKT "POINT(5 6)" TEXT "kept" END"#,
    );
    assert!(loaded.diagnostics.is_empty(), "{:?}", loaded.diagnostics);
    let features = layer.unwrap().features;
    assert_eq!(features.len(), 3);
    assert_eq!(features[0].shape_type, ShapeType::Polygon);
    assert_eq!(
        features[0].parts,
        [
            vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 0.0]],
            vec![[2.0, 2.0], [3.0, 2.0], [3.0, 3.0], [2.0, 2.0]],
        ]
    );
    assert_eq!(
        features[0].properties,
        props(&[("name", "a"), ("size", "7")])
    );
    assert_eq!(features[0].text.as_deref(), Some("first"));
    // A WKT gives the shape its own type, whatever the layer's.
    assert_eq!(features[1].shape_type, ShapeType::Line);
    assert_eq!(features[1].parts, [vec![[1.0, 2.0], [3.0, 4.0]]]);
    assert_eq!(
        features[1].properties,
        props(&[("name", "b"), ("size", "")])
    );
    // It replaces what was read before it, the values with the shape.
    assert_eq!(features[2].shape_type, ShapeType::Point);
    assert_eq!(features[2].parts, [vec![[5.0, 6.0]]]);
    assert_eq!(features[2].properties, props(&[("name", ""), ("size", "")]));
    assert_eq!(features[2].text.as_deref(), Some("kept"));

    let (loaded, _) = load_layer(r#"TYPE POINT FEATURE WKT "POINT(1)" END"#);
    assert_eq!(codes(&loaded), ["invalid-wkt"]);
    assert!(loaded.document.is_none());
    assert_eq!(
        loaded.diagnostics[0].span.as_ref().map(|s| s.column),
        Some(30),
        "the error is at the WKT"
    );
}

#[test]
fn a_feature_takes_the_shape_type_of_the_layer_type_before_it() {
    let types = [
        ("POLYGON", ShapeType::Polygon),
        ("LINE", ShapeType::Line),
        ("POINT", ShapeType::Point),
        // Every other type gives points, a CIRCLE's too.
        ("CIRCLE", ShapeType::Point),
        ("ANNOTATION", ShapeType::Point),
    ];
    for (word, expected) in types {
        let (_, layer) = load_layer(&format!("TYPE {word} FEATURE POINTS 1 1 END END"));
        assert_eq!(layer.unwrap().features[0].shape_type, expected, "{word}");
    }
    // A later TYPE leaves an earlier feature as it was read.
    let (_, layer) = load_layer("TYPE LINE FEATURE POINTS 1 1 2 2 END END TYPE POINT");
    assert_eq!(layer.unwrap().features[0].shape_type, ShapeType::Line);
}

#[test]
fn a_feature_before_the_layer_type_is_refused() {
    let (loaded, _) = load_layer("FEATURE POINTS 1 1 END END TYPE POINT");
    assert_eq!(codes(&loaded), ["feature-before-type"]);
    assert!(loaded.document.is_none());
}

#[test]
fn points_are_read_in_pairs_each_block_a_part() {
    let features =
        features("TYPE LINE FEATURE POINTS 1 2 3 4 END POINTS END POINTS -1.5 1e1 END END");
    assert_eq!(
        features[0].parts,
        [vec![[1.0, 2.0], [3.0, 4.0]], vec![], vec![[-1.5, 10.0]]]
    );
    for points in ["1 2 3", "1 'a'", "[x] 1"] {
        let (loaded, _) = load_layer(&format!("TYPE LINE FEATURE POINTS {points} END END"));
        assert_eq!(codes(&loaded), ["invalid-value"], "{points}");
    }
}

#[test]
fn values_split_at_semicolons_keep_empty_ones() {
    let values = |items: &str| {
        let body = format!(
            r#"TYPE POINT PROCESSING "ITEMS=a,b,c,d" FEATURE POINTS 1 1 END ITEMS "{items}" END"#
        );
        features(&body)[0]
            .properties
            .iter()
            .map(|p| p.value.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(values("1;;3"), ["1", "", "3", ""]);
    assert_eq!(values(";2;"), ["", "2", "", ""]);
    assert_eq!(values(""), ["", "", "", ""]);
    // A value past the names is dropped.
    assert_eq!(values("1;2;3;4;5"), ["1", "2", "3", "4"]);
    // A later ITEMS replaces an earlier one.
    let body = r#"TYPE POINT PROCESSING "ITEMS=a" FEATURE ITEMS "1" ITEMS "2" END"#;
    assert_eq!(features(body)[0].properties, props(&[("a", "2")]));
}

#[test]
fn item_names_come_from_the_first_items_key_split_at_commas() {
    let names = |processing: &str| {
        let body = format!(r#"TYPE POINT {processing} FEATURE POINTS 1 1 END END"#);
        let (_, layer) = load_layer(&body);
        layer.unwrap().features[0]
            .properties
            .iter()
            .map(|p| p.name.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(names(r#"PROCESSING "items:a,,b,""#), ["a", "b", ""]);
    assert_eq!(
        names(r#"PROCESSING "ITEMSX=z" PROCESSING "Items=x,Y" PROCESSING "ITEMS=w""#),
        ["x", "Y"]
    );
    // Wherever the directive stands.
    let (_, layer) = load_layer(r#"TYPE POINT FEATURE POINTS 1 1 END END PROCESSING "ITEMS=late""#);
    assert_eq!(
        layer.unwrap().features[0].properties,
        props(&[("late", "")])
    );
    // Without one, a feature's values are named by nothing.
    let body = r#"TYPE POINT FEATURE ITEMS "1;2" END"#;
    assert!(features(body)[0].properties.is_empty());
}

#[test]
fn processing_items_is_carried_only_for_an_inline_layer() {
    let (loaded, _) = load_layer(r#"TYPE POINT PROCESSING "ITEMS=a" FEATURE POINTS 1 1 END END"#);
    assert!(loaded.diagnostics.is_empty(), "{:?}", loaded.diagnostics);
    let (loaded, _) = load_layer(r#"TYPE POINT PROCESSING "ITEMS=a" DATA "x""#);
    assert_eq!(codes(&loaded), ["unsupported-keyword"]);
    let body = r#"TYPE POINT PROCESSING "LABEL_NO_CLIP=1" FEATURE POINTS 1 1 END END"#;
    let (loaded, _) = load_layer(body);
    assert_eq!(codes(&loaded), ["unsupported-keyword"]);
}

#[test]
fn a_layer_without_features_carries_none() {
    let (loaded, layer) = load_layer(r#"TYPE POINT DATA "x""#);
    assert!(loaded.diagnostics.is_empty(), "{:?}", loaded.diagnostics);
    assert!(layer.unwrap().features.is_empty());
}

#[test]
fn an_unreadable_wkt_is_quoted_in_part() {
    let wkt = format!("LINESTRING({})", "1 2 3, ".repeat(100));
    let (loaded, _) = load_layer(&format!(r#"TYPE LINE FEATURE WKT "{wkt}" END"#));
    assert_eq!(codes(&loaded), ["invalid-wkt"]);
    let message = &loaded.diagnostics[0].message;
    assert!(message.contains(&format!("`{}…`", &wkt[..80])), "{message}");
    assert!(message.len() < 200, "{message}");
}
