// The expected trees follow MapServer's src/maplexer.l and src/mapparser.y of the release pinned
// in scripts/fetch-msautotest.sh.
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

//! The trees are worked by hand from the rules of `src/mapparser.y` at the pin.

use super::*;

fn tree(text: &str) -> TransformTree {
    let translated = geom_transform(text).unwrap_or_else(|e| panic!("{text}: {e:?}"));
    assert_eq!(translated.syntax, None, "{text}");
    translated.tree
}

fn not_drawn(text: &str) -> NotDrawn {
    match tree(text) {
        TransformTree::NotDrawn(reason) => reason,
        other => panic!("{text}: {other:?}"),
    }
}

/// The syntax error of `text`, and the character of the token it is found at.
fn syntax(text: &str) -> (String, usize) {
    let translated = geom_transform(text).unwrap_or_else(|e| panic!("{text}: {e:?}"));
    assert_eq!(
        translated.tree,
        TransformTree::NotDrawn(NotDrawn::Syntax),
        "{text}"
    );
    translated
        .syntax
        .unwrap_or_else(|| panic!("{text}: no syntax error"))
}

fn shape() -> Box<TransformShape> {
    Box::new(TransformShape::Shape)
}

fn num(n: f64) -> TransformNumber {
    TransformNumber::Number(n)
}

fn drawn(shape: TransformShape) -> TransformTree {
    TransformTree::Drawn(shape)
}

#[test]
fn the_expressions_of_msautotest_translate_to_their_trees_or_reasons() {
    // Verifies: ADR-draft-typed-geomtransform
    // Every GEOMTRANSFORM expression of the msautotest suite at 7515cccf (rel-8-6-6), as the
    // mapfile's token holds it.
    assert_eq!(
        tree("(outer([shape]))"),
        drawn(TransformShape::Outer(shape()))
    );
    assert_eq!(
        tree("(inner([shape]))"),
        drawn(TransformShape::Inner(shape()))
    );
    assert_eq!(
        tree("(generalize([shape],50))"),
        drawn(TransformShape::Generalize(shape(), num(50.0)))
    );
    for (text, reason) in [
        ("(centerline([shape]))", NotDrawn::Centerline),
        ("(centerline(densify([shape], 5)))", NotDrawn::Centerline),
        ("(centerline(densify([shape],-5)))", NotDrawn::Centerline),
        (
            "(smoothsia(centerline(densify([shape], 5))))",
            NotDrawn::Centerline,
        ),
        ("(simplify([shape],25))", NotDrawn::Simplify),
        ("(simplify([shape],10))", NotDrawn::Simplify),
        ("(simplifypt([shape],50))", NotDrawn::Simplifypt),
        ("(buffer([shape], 1))", NotDrawn::Buffer),
        ("(buffer([shape], -1))", NotDrawn::Buffer),
    ] {
        assert_eq!(not_drawn(text), reason, "{text}");
    }
    // `labelpoint` is no function of `shape_exp`.
    let (what, at) = syntax("(labelpoint([shape]))");
    assert!(
        what.contains("`l`") && what.contains("where a shape is due"),
        "{what}"
    );
    assert_eq!(at, 1);
}

#[test]
fn each_function_of_the_tree_translates_with_its_arguments() {
    // Verifies: ADR-draft-typed-geomtransform
    assert_eq!(tree("([shape])"), drawn(TransformShape::Shape));
    assert_eq!(
        tree("(densify(([shape]), 2.5))"),
        drawn(TransformShape::Densify(shape(), num(2.5)))
    );
    let smooth = |size: f64, iterations: f64, preprocessing| {
        drawn(TransformShape::SmoothSia {
            shape: shape(),
            size: num(size),
            iterations: num(iterations),
            preprocessing,
        })
    };
    // msSmoothShapeSIA(s, 3, 1, NULL) and (s, size, 1, NULL) where they are not written.
    assert_eq!(tree("(smoothsia([shape]))"), smooth(3.0, 1.0, None));
    assert_eq!(tree("(smoothsia([shape], 5))"), smooth(5.0, 1.0, None));
    assert_eq!(tree("(smoothsia([shape], 5, 2))"), smooth(5.0, 2.0, None));
    // The preprocessing read as strcasecmp reads it; any other text is none.
    let all = Some(Preprocessing::All);
    let angle = Some(Preprocessing::Angle);
    assert_eq!(
        tree("(smoothsia([shape], 5, 2, 'ALL'))"),
        smooth(5.0, 2.0, all)
    );
    assert_eq!(
        tree("(smoothsia([shape], 5, 2, \"Angle\"))"),
        smooth(5.0, 2.0, angle)
    );
    assert_eq!(
        tree("(smoothsia([shape], 5, 2, ('all')))"),
        smooth(5.0, 2.0, all)
    );
    assert_eq!(
        tree("(smoothsia([shape], 5, 2, 'none'))"),
        smooth(5.0, 2.0, None)
    );
    // Function names in any case, as the lexer is built with `flex -i`.
    assert_eq!(
        tree("(GENERALIZE(Outer([SHAPE]), 1))"),
        drawn(TransformShape::Generalize(
            Box::new(TransformShape::Outer(shape())),
            num(1.0)
        ))
    );
}

#[test]
fn literal_arithmetic_folds_and_map_cellsize_is_kept() {
    // Verifies: ADR-draft-typed-geomtransform
    let tolerance = |text: &str| match tree(&format!("(densify([shape], {text}))")) {
        TransformTree::Drawn(TransformShape::Densify(_, number)) => number,
        other => panic!("{text}: {other:?}"),
    };
    for (text, value) in [
        // `*` before `-`; `-` and `/` to the left, `^` to the right.
        ("9 - 2 * 3", 3.0),
        ("9 - 3 - 3", 3.0),
        ("36 / 6 / 2", 3.0),
        ("3 ^ 1 ^ 2", 3.0),
        ("2 ^ 3 ^ 2", 512.0),
        ("(12 / 2 ^ 2)", 3.0),
        // `%` of the operands cut to C's `int`s: 7.9 % 4.2 is 7 % 4, -7.5 % 2 is -7 % 2.
        ("7.9 % 4.2", 3.0),
        ("-7.5 % 2", -1.0),
        // An operand past an `int` is INT_MIN, as the (int) cast gives it (ADR-0036).
        ("3e9 % 7", -2.0),
    ] {
        assert_eq!(tolerance(text), num(value), "{text}");
    }
    assert_eq!(
        tolerance("[map_cellsize] * (2 + 1)"),
        TransformNumber::Op(
            NumberOp::Mul,
            Box::new(TransformNumber::MapCellsize),
            Box::new(num(3.0))
        )
    );
    assert_eq!(
        tolerance("1 + [MAP_CELLSIZE] / 4"),
        TransformNumber::Op(
            NumberOp::Add,
            Box::new(num(1.0)),
            Box::new(TransformNumber::Op(
                NumberOp::Div,
                Box::new(TransformNumber::MapCellsize),
                Box::new(num(4.0))
            ))
        )
    );
}

#[test]
fn a_unary_minus_leaves_its_operand_un_negated() {
    // Verifies: ADR-draft-typed-geomtransform
    // `'-' math_exp %prec NEG { $$ = $2; }`; `-3` written together is one number token.
    let tolerance = |text: &str| match tree(&format!("(densify([shape], {text}))")) {
        TransformTree::Drawn(TransformShape::Densify(_, number)) => number,
        other => panic!("{text}: {other:?}"),
    };
    assert_eq!(tolerance("- 3"), num(3.0));
    assert_eq!(tolerance("-3"), num(-3.0));
    assert_eq!(tolerance("- - 3"), num(3.0));
    // NEG binds below `^`, above `*`: `- 3 ^ 2` is 9, `- 3 * 2` is 6.
    assert_eq!(tolerance("- 3 ^ 2"), num(9.0));
    assert_eq!(tolerance("2 * - 3"), num(6.0));
    assert_eq!(tolerance("- [map_cellsize]"), TransformNumber::MapCellsize);
}

#[test]
fn the_first_construct_without_a_node_names_the_reason() {
    // Verifies: ADR-draft-typed-geomtransform
    for (text, reason) in [
        ("(difference([shape], [shape]))", NotDrawn::Difference),
        ("(javascript([shape], 'f.js'))", NotDrawn::Javascript),
        ("(densify([shape], length('abc')))", NotDrawn::Length),
        ("(densify([shape], area([shape])))", NotDrawn::Area),
        ("(densify([shape], round(2.5)))", NotDrawn::Round),
        ("(densify([shape], round(7, 2)))", NotDrawn::Round),
        ("(densify([shape], [width]))", NotDrawn::Binding),
        ("(fromtext('POINT(1 1)'))", NotDrawn::Fromtext),
        (
            "(generalize(fromtext('LINESTRING(0 0, 1 1)'), 2))",
            NotDrawn::Fromtext,
        ),
        // `%` by 0 and INT_MIN % -1 trap in the x86-64 build; a result or a literal past a double
        // is no finite JSON number.
        ("(generalize([shape], 3 % 0.5))", NotDrawn::Literal),
        ("(generalize([shape], 3e9 % -1))", NotDrawn::Literal),
        ("(generalize([shape], 1e300 * 1e300))", NotDrawn::Literal),
        ("(generalize([shape], 1e999))", NotDrawn::Literal),
        (
            "(generalize([shape], 2 ^ 2000 - [map_cellsize]))",
            NotDrawn::Literal,
        ),
        ("(smoothsia([shape], 3, 1, \"[prep]\"))", NotDrawn::Binding),
        ("(smoothsia([shape], 3, 1, 'al' + 'l'))", NotDrawn::Text),
        ("(smoothsia([shape], 3, 1, upper('all')))", NotDrawn::Text),
        (
            "(smoothsia([shape], 3, 1, tostring(1, '%g')))",
            NotDrawn::Text,
        ),
        // In reading order: buffer before round, round before buffer.
        (
            "(generalize(buffer([shape], 1), round(2)))",
            NotDrawn::Buffer,
        ),
        (
            "(difference(generalize([shape], round(2)), buffer([shape], 1)))",
            NotDrawn::Difference,
        ),
        ("(densify([shape], [w] + length('a')))", NotDrawn::Binding),
    ] {
        assert_eq!(not_drawn(text), reason, "{text}");
    }
}

#[test]
fn a_syntax_error_is_warned_with_its_place() {
    // Verifies: ADR-draft-typed-geomtransform
    for (text, found, at) in [
        ("(generalize([shape]))", "`)` where `,` is due", 19),
        ("(outer([shape], 2))", "`,` where `)` is due", 14),
        ("([shape] 3)", "`3` after the shape", 9),
        ("(1 + 2)", "`1` where a shape is due", 1),
        ("(generalize([shape], 2 3))", "`3` where `)` is due", 23),
        // `-2`, one token, follows `[map_cellsize]` as MapServer's lexer reads it.
        (
            "(densify([shape], [map_cellsize]-2))",
            "`-2` where `)` is due",
            32,
        ),
        (
            "(generalize([shape], 'a'))",
            "`'a'` where a number is due",
            21,
        ),
        (
            "(smoothsia([shape], 3, 1, 4))",
            "`4` where a text is due",
            26,
        ),
        (
            "(generalize([geom], 1))",
            "`[geom]` where a shape is due",
            12,
        ),
        ("(generalize([shape], 3 / 0))", "`/` divides by zero", 23),
        ("(fromtext(1))", "`1` where fromtext's text is due", 10),
        (
            "([shape] intersects [shape])",
            "`intersects` after the shape",
            9,
        ),
        (
            "(smoothsia([shape], 3, 1, 'all', 2))",
            "`,` where `)` is due",
            31,
        ),
        (
            "(generalize([shape], 2)",
            "the end of the expression where `)` is due",
            22,
        ),
        ("()", "the end of the expression where a shape is due", 1),
        ("(simplify([shape] 1))", "`1` where `,` is due", 18),
    ] {
        let (what, character) = syntax(text);
        assert!(what.contains(found), "{text}: {what}");
        assert_eq!(character, at, "{text}: {what}");
    }
}

#[test]
fn a_syntax_error_takes_precedence_over_an_earlier_construct_without_a_node() {
    // Verifies: ADR-draft-typed-geomtransform
    // MapServer refuses the whole expression, so `buffer` before the error names nothing.
    let (what, at) = syntax("(buffer([shape],1) 2)");
    assert!(what.contains("`2` after the shape"), "{what}");
    assert_eq!(at, 19);
}

#[test]
fn data_cellsize_folds_to_0_as_on_a_layer_that_is_no_contour() {
    // Verifies: ADR-draft-typed-geomtransform
    // msGeomTransformShape sets p.dblval2 = 0 and reads `__data_cellsize__` only for a contour
    // layer (src/mapgeomtransform.c:285-291).
    assert_eq!(
        tree("(generalize([shape],[data_cellsize]))"),
        drawn(TransformShape::Generalize(shape(), num(0.0)))
    );
    assert_eq!(
        tree("(densify([shape], [DATA_CELLSIZE] + 2))"),
        drawn(TransformShape::Densify(shape(), num(2.0)))
    );
}

#[test]
fn a_diagnostic_quotes_the_expression_bounded() {
    let long = "a".repeat(40);
    let (what, _) = syntax(&format!("(generalize([shape], '{long}'))"));
    assert!(what.contains('\u{2026}'), "{what}");
    assert!(!what.contains(&long), "{what}");
}

#[test]
fn a_chain_of_25_operators_is_refused_as_too_deep_without_overflowing_the_stack() {
    // Verifies: ADR-draft-typed-geomtransform, Q-9
    // densify (1), its number's chain of `+` below it, its last operands one level further:
    // 22 operators nest 24 levels and translate; 23 and 25 pass the bound.
    let chain = |operators: usize| {
        format!(
            "(densify([shape], {}))",
            vec!["[map_cellsize]"; operators + 1].join(" + ")
        )
    };
    let at_limit = tree(&chain(22));
    assert_eq!(at_limit.depth(), MAX_EXPRESSION_DEPTH);
    let error = geom_transform(&chain(25)).unwrap_err();
    assert_eq!(error.code, "expression-too-deep");
    assert!(error.error);
    let error = geom_transform(&chain(23)).unwrap_err();
    assert_eq!(error.code, "expression-too-deep");
    // A literal chain folds to one number, and is still counted.
    let literal = format!("(densify([shape], {}))", vec!["1"; 26].join(" + "));
    assert_eq!(
        geom_transform(&literal).unwrap_err().code,
        "expression-too-deep"
    );
    // A chain far past the bound, of numbers, texts, or nested shapes, and parentheses that nest
    // no node, are refused the same way.
    let long = format!("(densify([shape], {}))", vec!["1"; 100_000].join(" * "));
    assert_eq!(
        geom_transform(&long).unwrap_err().code,
        "expression-too-deep"
    );
    let texts = format!(
        "(smoothsia([shape], 3, 1, {}))",
        vec!["'a'"; 100_000].join(" + ")
    );
    assert_eq!(
        geom_transform(&texts).unwrap_err().code,
        "expression-too-deep"
    );
    let nested = format!("({}[shape]{})", "outer(".repeat(10_000), ")".repeat(10_000));
    assert_eq!(
        geom_transform(&nested).unwrap_err().code,
        "expression-too-deep"
    );
    let parens = format!("({}[shape]{})", "(".repeat(10_000), ")".repeat(10_000));
    assert_eq!(
        geom_transform(&parens).unwrap_err().code,
        "expression-too-deep"
    );
    let minus = format!("(densify([shape], {}1))", "- ".repeat(10_000));
    assert_eq!(
        geom_transform(&minus).unwrap_err().code,
        "expression-too-deep"
    );
}
