// The defaults of smoothsia follow MapServer's src/mapparser.y of the release pinned in
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

use super::*;

fn num(n: f64) -> TransformNumber {
    TransformNumber::Number(n)
}

fn op(op: NumberOp, left: TransformNumber, right: TransformNumber) -> TransformNumber {
    TransformNumber::Op(op, Box::new(left), Box::new(right))
}

fn shape() -> Box<TransformShape> {
    Box::new(TransformShape::Shape)
}

/// `tree` written as JSON, and read back equal.
fn round_trip(tree: &TransformTree) -> String {
    let json = serde_json::to_string(tree).unwrap();
    assert_eq!(&serde_json::from_str::<TransformTree>(&json).unwrap(), tree);
    json
}

#[test]
fn each_node_and_number_reads_and_writes_in_the_shape_of_cql2_json() {
    // Verifies: ADR-0036
    let cases = [
        (
            TransformTree::Drawn(TransformShape::Shape),
            r#"{"op":"shape","args":[]}"#,
        ),
        (
            TransformTree::Drawn(TransformShape::Generalize(
                shape(),
                op(NumberOp::Mul, TransformNumber::MapCellsize, num(2.5)),
            )),
            r#"{"op":"generalize","args":[{"op":"shape","args":[]},{"op":"*","args":[{"op":"map_cellsize","args":[]},2.5]}]}"#,
        ),
        (
            TransformTree::Drawn(TransformShape::Densify(
                Box::new(TransformShape::Outer(Box::new(TransformShape::Inner(
                    shape(),
                )))),
                num(5.0),
            )),
            r#"{"op":"densify","args":[{"op":"outer","args":[{"op":"inner","args":[{"op":"shape","args":[]}]}]},5.0]}"#,
        ),
        (
            TransformTree::Drawn(TransformShape::SmoothSia {
                shape: shape(),
                size: num(5.0),
                iterations: num(2.0),
                preprocessing: Some(Preprocessing::Angle),
            }),
            r#"{"op":"smoothsia","args":[{"op":"shape","args":[]},5.0,2.0,"angle"]}"#,
        ),
    ];
    for (tree, json) in cases {
        assert_eq!(round_trip(&tree), json);
    }
    // Every operator of a number.
    for (operator, name) in [
        (NumberOp::Add, "+"),
        (NumberOp::Sub, "-"),
        (NumberOp::Mul, "*"),
        (NumberOp::Div, "/"),
        (NumberOp::Mod, "%"),
        (NumberOp::Pow, "^"),
    ] {
        let tree = TransformTree::Drawn(TransformShape::Densify(
            shape(),
            op(operator, TransformNumber::MapCellsize, num(1.0)),
        ));
        let json = round_trip(&tree);
        assert!(
            json.contains(&format!(r#"{{"op":"{name}","args":[{{"op":"map_cellsize""#)),
            "{json}"
        );
    }
}

#[test]
fn every_not_drawn_reason_reads_and_writes_by_its_name() {
    // Verifies: ADR-0036
    let reasons = [
        (NotDrawn::Buffer, "buffer"),
        (NotDrawn::Centerline, "centerline"),
        (NotDrawn::Difference, "difference"),
        (NotDrawn::Simplify, "simplify"),
        (NotDrawn::Simplifypt, "simplifypt"),
        (NotDrawn::Javascript, "javascript"),
        (NotDrawn::Length, "length"),
        (NotDrawn::Area, "area"),
        (NotDrawn::Round, "round"),
        (NotDrawn::Binding, "binding"),
        (NotDrawn::Text, "text"),
        (NotDrawn::Fromtext, "fromtext"),
        (NotDrawn::Literal, "literal"),
        (NotDrawn::Syntax, "syntax"),
    ];
    for (reason, name) in reasons {
        let tree = TransformTree::NotDrawn(reason);
        assert_eq!(round_trip(&tree), format!(r#"{{"not_drawn":"{name}"}}"#));
        assert_eq!(reason.name(), name);
    }
}

#[test]
fn smoothsia_without_its_size_or_iterations_reads_as_mapservers_defaults() {
    // The rules `SMOOTHSIA '(' shape_exp ')'` and `… ',' math_exp ')'` of src/mapparser.y call
    // msSmoothShapeSIA(s, 3, 1, NULL) and (s, size, 1, NULL).
    let read = |json: &str| serde_json::from_str::<TransformTree>(json).unwrap();
    let smooth = |size: f64| {
        TransformTree::Drawn(TransformShape::SmoothSia {
            shape: shape(),
            size: num(size),
            iterations: num(1.0),
            preprocessing: None,
        })
    };
    assert_eq!(
        read(r#"{"op":"smoothsia","args":[{"op":"shape","args":[]}]}"#),
        smooth(3.0)
    );
    assert_eq!(
        read(r#"{"op":"smoothsia","args":[{"op":"shape","args":[]},7]}"#),
        smooth(7.0)
    );
}

#[test]
fn a_node_that_is_none_of_the_trees_does_not_read() {
    for json in [
        r#"{"op":"buffer","args":[{"op":"shape","args":[]},1]}"#,
        r#"{"op":"generalize","args":[{"op":"shape","args":[]}]}"#,
        r#"{"op":"outer","args":[{"op":"shape","args":[]},1]}"#,
        r#"{"op":"densify","args":[1,{"op":"shape","args":[]}]}"#,
        r#"{"op":"densify","args":[{"op":"shape","args":[]},"5"]}"#,
        r#"{"op":"smoothsia","args":[{"op":"shape","args":[]},3,1,"none"]}"#,
        r#"{"op":"densify","args":[{"op":"shape","args":[]},{"op":"+","args":[1]}]}"#,
        r#"{"op":"densify","args":[{"op":"shape","args":[]},{"property":"w"}]}"#,
        r#"{"op":"shape","args":[],"not_drawn":"syntax"}"#,
        r#"{"not_drawn":"anything"}"#,
        r#"{"op":"shape"}"#,
        r#""shape""#,
        "5",
    ] {
        assert!(
            serde_json::from_str::<TransformTree>(json).is_err(),
            "{json}"
        );
    }
}

#[test]
fn depth_counts_the_levels_of_the_tree_as_an_expressions_depth() {
    assert_eq!(TransformTree::NotDrawn(NotDrawn::Syntax).depth(), 1);
    assert_eq!(TransformTree::Drawn(TransformShape::Shape).depth(), 1);
    // generalize, then `+`, then `*`, then its operands: 4 levels.
    let tree = TransformTree::Drawn(TransformShape::Generalize(
        shape(),
        op(
            NumberOp::Add,
            op(NumberOp::Mul, TransformNumber::MapCellsize, num(2.0)),
            num(1.0),
        ),
    ));
    assert_eq!(tree.depth(), 4);
    let smooth = TransformTree::Drawn(TransformShape::SmoothSia {
        shape: Box::new(TransformShape::Outer(shape())),
        size: num(3.0),
        iterations: num(1.0),
        preprocessing: Some(Preprocessing::All),
    });
    assert_eq!(smooth.depth(), 3);
}
