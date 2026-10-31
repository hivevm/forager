// The tests pin the behaviour of MapServer's loadStyle, src/mapfile.c, and the functions they
// name, of the release pinned in scripts/fetch-msautotest.sh.
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

use forager_style_ir::{Bound, Rgba};

use crate::test_support::{binding, codes, first_style, load_files};

#[test]
fn style_properties_bind_to_attributes() {
    // Verifies: ADR-0030
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
    // Verifies: ADR-0030
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
    // Verifies: ADR-0030
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

/// The style of `body`, read without a diagnostic.
fn style_of(body: &str) -> forager_style_ir::Style {
    let text: &'static str = format!("MAP LAYER CLASS STYLE {body} END END END END").leak();
    let (loaded, style) = first_style(text);
    assert_eq!(codes(&loaded), Vec::<&str>::new(), "{body}");
    style.unwrap()
}

/// `body` is a load error, `invalid-value`, as loadStyle fails it.
fn refused(body: &str) {
    let text: &'static str = format!("MAP LAYER CLASS STYLE {body} END END END END").leak();
    let loaded = load_files(&[("m.map", text)]);
    assert!(loaded.document.is_none(), "{body}");
    assert_eq!(codes(&loaded), ["invalid-value"], "{body}");
}

#[test]
fn each_number_takes_the_range_loadstyle_checks_at_its_bounds() {
    // MS_NUM_CHECK_RANGE -360..360, GT 0, GTE 0, NONE, at and past each bound.
    let style = style_of("ANGLE -360");
    assert_eq!(style.angle, Some(Bound::Value(-360.0)));
    assert_eq!(style_of("ANGLE 360").angle, Some(Bound::Value(360.0)));
    refused("ANGLE -360.5");
    refused("ANGLE 360.5");
    assert_eq!(style_of("SIZE 0.01").size, Some(Bound::Value(0.01)));
    refused("SIZE -1");
    assert_eq!(style_of("WIDTH 0").width, Some(Bound::Value(0.0)));
    assert_eq!(
        style_of("OUTLINEWIDTH 0").outline_width,
        Some(Bound::Value(0.0))
    );
    refused("OUTLINEWIDTH -0.5");
    assert_eq!(style_of("INITIALGAP 0").initial_gap, Some(0.0));
    assert_eq!(style_of("GAP 0").gap, Some(0.0));
    assert_eq!(
        (
            style_of("MINSIZE 0").min_size,
            style_of("MINWIDTH 0").min_width
        ),
        (Some(0.0), Some(0.0))
    );
    refused("MINSIZE -1");
    refused("MINWIDTH -1");
    assert_eq!(style_of("MINSCALEDENOM 0").min_scale_denom, Some(0.0));
    refused("MINSCALEDENOM -1");
    refused("MAXSCALEDENOM -1");
    assert_eq!(
        style_of("SYMBOL 0").symbol,
        Some(Bound::Value(forager_style_ir::SymbolRef::Index(0)))
    );
    // DATARANGE's NONE check aside, POLAROFFSET and OFFSET take any number.
    let style = style_of("OFFSET -1e6 1e6 POLAROFFSET -3 720");
    assert_eq!(style.offset, Some([Bound::Value(-1e6), Bound::Value(1e6)]));
    assert_eq!(
        style.polar_offset,
        Some([Bound::Value(-3.0), Bound::Value(720.0)])
    );
}

#[test]
fn each_property_takes_the_kinds_of_value_loadstyle_takes() {
    // getSymbol's lists: SIZE alone takes an expression; ANGLE, WIDTH, OPACITY, OUTLINEWIDTH,
    // OFFSET, and POLAROFFSET a number or a binding; the plain numbers neither.
    for body in [
        "ANGLE ([a])",
        "OPACITY ([a])",
        "OUTLINEWIDTH ([a])",
        "OFFSET ([a]) 1",
        "POLAROFFSET 1 ([a])",
        "GAP [a]",
        "INITIALGAP [a]",
        "MINSIZE [a]",
        "MAXWIDTH [a]",
        "LINEJOINMAXSIZE [a]",
        "MINSCALEDENOM [a]",
        "LINECAP [a]",
        "PATTERN [a] 2 END",
        "WIDTH 'thick'",
        "SIZE 'big'",
    ] {
        refused(body);
    }
    let style = style_of("OPACITY [o] OUTLINEWIDTH [ow] POLAROFFSET [d] [a]");
    assert_eq!(style.opacity, binding("o"));
    assert_eq!(style.outline_width, binding("ow"));
    assert_eq!(
        style.polar_offset,
        Some([binding("d").unwrap(), binding("a").unwrap()])
    );
    assert_eq!(style_of("SYMBOL [s]").symbol, binding("s"));
}

#[test]
fn angle_auto_is_the_bare_keyword_only() {
    // The lexer's `auto` is the token MS_AUTO; a quoted 'AUTO' is a string, which getSymbol
    // refuses, and so is the neighbouring keyword `auto2`.
    assert!(style_of("ANGLE auto").auto_angle);
    refused("ANGLE 'AUTO'");
    refused("ANGLE AUTO2");
    // A later number or binding leaves `autoangle` set, as loadStyle leaves it.
    let style = style_of("ANGLE AUTO ANGLE 30");
    assert!(style.auto_angle);
    assert_eq!(style.angle, Some(Bound::Value(30.0)));
}

#[test]
fn opacity_is_truncated_toward_zero_then_clamped() {
    // MS_MAX(MS_MIN((int)msyynumber, 100), 0): the cast truncates toward zero.
    assert_eq!(style_of("OPACITY -5").opacity, Some(Bound::Value(0.0)));
    assert_eq!(style_of("OPACITY -0.9").opacity, Some(Bound::Value(0.0)));
    assert_eq!(style_of("OPACITY 99.99").opacity, Some(Bound::Value(99.0)));
    assert_eq!(style_of("OPACITY 0").opacity, Some(Bound::Value(0.0)));
}

#[test]
fn line_caps_and_joins_are_loadstyles_keywords() {
    use forager_style_ir::{LineCap, LineJoin};
    for (word, cap) in [
        ("butt", LineCap::Butt),
        ("round", LineCap::Round),
        ("square", LineCap::Square),
        ("triangle", LineCap::Triangle),
    ] {
        assert_eq!(style_of(&format!("LINECAP {word}")).line_cap, Some(cap));
    }
    for (word, join) in [
        ("none", LineJoin::None),
        ("round", LineJoin::Round),
        ("miter", LineJoin::Miter),
        ("bevel", LineJoin::Bevel),
    ] {
        assert_eq!(style_of(&format!("LINEJOIN {word}")).line_join, Some(join));
    }
    // A cap is not a join, nor a join a cap; a quoted keyword is a string, which getSymbol
    // refuses.
    refused("LINECAP miter");
    refused("LINEJOIN butt");
    refused("LINECAP 'round'");
}

#[test]
fn a_pattern_holds_only_numbers() {
    // loadStyle's PATTERN loop takes MS_NUMBER until END and fails on anything else.
    refused("PATTERN 1 two END");
    refused("PATTERN 1 [a] END");
    assert_eq!(
        style_of("PATTERN 1 2 3 4 5 6 7 8 9 10 END").pattern.len(),
        10
    );
}

#[test]
fn a_binding_holds_over_a_later_number() {
    // loadStyle sets a binding's item and never clears it for a later number (src/mapfile.c,
    // the cases of loadStyle), and bindStyle (src/maputil.c) draws the binding over the
    // number; only SIZE frees what it held, and only its expression.
    let style = style_of(
        "WIDTH [w] WIDTH 3 COLOR [c] COLOR 1 2 3 OUTLINECOLOR [o] OUTLINECOLOR '#102030'
         ANGLE [a] ANGLE 30 OPACITY [op] OPACITY 50 OUTLINEWIDTH [ow] OUTLINEWIDTH 2
         SYMBOL [s] SYMBOL 0",
    );
    assert_eq!(style.width, binding("w"));
    assert_eq!(style.color, binding("c"));
    assert_eq!(style.outline_color, binding("o"));
    assert_eq!(style.angle, binding("a"));
    assert_eq!(style.opacity, binding("op"));
    assert_eq!(style.outline_width, binding("ow"));
    assert_eq!(style.symbol, binding("s"));
    // A later binding replaces the earlier.
    assert_eq!(style_of("WIDTH [w] WIDTH 3 WIDTH [v]").width, binding("v"));

    // SIZE: an expression is evaluated after the binding and wins over it; a later SIZE frees
    // the expression, not the binding.
    assert_eq!(style_of("SIZE [a] SIZE 5").size, binding("a"));
    assert_eq!(
        style_of("SIZE [a] SIZE ([b] + 1) SIZE 5").size,
        binding("a")
    );
    assert_eq!(
        style_of("SIZE ([b] + 1) SIZE 5").size,
        Some(Bound::Value(5.0))
    );
    assert!(matches!(
        style_of("SIZE [a] SIZE ([b] + 1)").size,
        Some(Bound::Expr(_))
    ));
    assert_eq!(style_of("SIZE ([b] + 1) SIZE [a]").size, binding("a"));

    // OFFSET and POLAROFFSET keep each axis's binding of its own.
    let style = style_of("OFFSET [x] 2 OFFSET 1 [y] OFFSET 4 5 POLAROFFSET 1 [pa] POLAROFFSET 2 3");
    assert_eq!(
        style.offset,
        Some([binding("x").unwrap(), binding("y").unwrap()])
    );
    assert_eq!(
        style.polar_offset,
        Some([Bound::Value(2.0), binding("pa").unwrap()])
    );
}

#[test]
fn a_later_offset_replaces_a_line_offset() {
    use forager_style_ir::LineOffset;
    // loadStyle keeps one offsetx and one offsety; -99 and -999 are values of offsety
    // (msDrawLineSymbol, src/maprendering.c), so a later OFFSET replaces a line offset and the
    // reverse.
    let style = style_of("OFFSET 3 -99 OFFSET 1 2");
    assert_eq!(style.line_offset, None);
    assert_eq!(style.offset, Some([Bound::Value(1.0), Bound::Value(2.0)]));
    let style = style_of("OFFSET 1 2 OFFSET 3 -999");
    assert_eq!(style.offset, None);
    assert_eq!(
        style.line_offset,
        Some(LineOffset {
            distance: Bound::Value(3.0),
            both_sides: true
        })
    );
    // A bound y is drawn over the -99, so it is a shift, not a line offset.
    let style = style_of("OFFSET 3 [y] OFFSET 4 -99");
    assert_eq!(style.line_offset, None);
    assert_eq!(
        style.offset,
        Some([Bound::Value(4.0), binding("y").unwrap()])
    );
    // A bound distance keeps its binding over a later number.
    let style = style_of("OFFSET [d] -99 OFFSET 2 -99");
    assert_eq!(
        style.line_offset,
        Some(LineOffset {
            distance: binding("d").unwrap(),
            both_sides: false
        })
    );
}

#[test]
fn a_geomtransform_name_is_matched_on_its_prefix_as_msstylesetgeomtransform_matches_it() {
    use forager_style_ir::GeomTransform;
    // msStyleSetGeomTransform (src/mapgeomtransform.c) compares the first 5, 3, 8, 4, 8, 9, 11,
    // and 8 characters, without case, with start, end, vertices, bbox, labelpnt, labelpoly,
    // labelcenter, and centroid, in that order.
    for (name, transform) in [
        ("'labelpnt'", GeomTransform::LabelPoint),
        ("LABELPNT", GeomTransform::LabelPoint),
        ("'startpoint'", GeomTransform::Start),
        ("'Ending'", GeomTransform::End),
        ("'vertices2'", GeomTransform::Vertices),
        ("'labelpolygon'", GeomTransform::LabelPoly),
        ("LABELCENTER", GeomTransform::LabelCenter),
        ("'centroids'", GeomTransform::Centroid),
    ] {
        assert_eq!(
            style_of(&format!("GEOMTRANSFORM {name}")).geom_transform,
            Some(transform),
            "{name}"
        );
    }
    // Neither MapServer's name nor a prefix of one: an unknown transform, which
    // msStyleSetGeomTransform resets to none.
    for name in ["'labelpoint'", "'cent'", "'st'"] {
        let text: &'static str =
            format!("MAP LAYER CLASS STYLE GEOMTRANSFORM {name} END END END END").leak();
        let (loaded, style) = first_style(text);
        assert_eq!(codes(&loaded), ["unsupported-value"], "{name}");
        assert_eq!(style.unwrap().geom_transform, None, "{name}");
    }
    // A later unknown transform resets an earlier one.
    let text = "MAP LAYER CLASS STYLE GEOMTRANSFORM 'start' GEOMTRANSFORM 'bbox' END END END END";
    let (_, style) = first_style(text);
    assert_eq!(style.unwrap().geom_transform, None);
}

#[test]
fn a_colour_binding_holds_over_a_later_colour_of_none() {
    let (loaded, style) =
        first_style("MAP LAYER CLASS STYLE COLOR [c] COLOR -1 -1 -1 END END END END");
    assert_eq!(codes(&loaded), Vec::<&str>::new());
    assert_eq!(style.unwrap().color, binding("c"));
}

/// The colour of `body`'s `COLOR`.
fn color_of(body: &str) -> Option<Bound<Rgba>> {
    style_of(&format!("COLOR {body}")).color
}

fn value(r: u8, g: u8, b: u8, a: u8) -> Option<Bound<Rgba>> {
    Some(Bound::Value(Rgba { r, g, b, a }))
}

#[test]
fn each_colour_channel_is_checked_by_load_color_before_it_is_truncated() {
    // loadColor checks the first channel's msyynumber, and getInteger the others' through
    // msCheckNumber, each before the cast to `int`.
    for body in [
        "COLOR 255.5 0 0",
        "COLOR 0 255.5 0",
        "COLOR 0 0 255.5",
        "COLOR -255.5 0 0",
        "COLOR 0 -255.5 0",
        "COLOR 0 0 -256",
    ] {
        refused(body);
    }
    assert_eq!(color_of("255 255 255"), value(255, 255, 255, 255));
    assert_eq!(color_of("0.9 -0.5 255.0"), value(0, 0, 255, 255));
    // The cast truncates toward 0: -1.5 is -1, which MS_VALID_COLOR refuses, in any channel.
    for body in ["-1.5 0 0", "0 -1 0", "0 0 -1.9"] {
        assert_eq!(color_of(body), None, "{body}");
    }
    assert_eq!(color_of("-2.5 0 0"), value(254, 0, 0, 255));
}

#[test]
fn a_hex_colour_takes_either_case_and_an_optional_alpha() {
    // msHexToInt, src/mapstring.cpp, folds a letter to upper case; a seventh and eighth digit
    // are the alpha, else 255.
    assert_eq!(color_of("'#AbCdEf'"), value(0xab, 0xcd, 0xef, 255));
    assert_eq!(color_of("\"#aBcDeF80\""), value(0xab, 0xcd, 0xef, 0x80));
    assert_eq!(color_of("'#00000000'"), value(0, 0, 0, 0));
    assert_eq!(color_of("'#ffffffFF'"), value(255, 255, 255, 255));
}

#[test]
fn a_colour_string_other_than_hash_and_six_or_eight_digits_is_refused() {
    // loadColor: "Invalid hex color" for a string that is not `#` and 7 or 9 characters long,
    // a bare word among them, which MapServer's lexer reads as a string.
    for body in [
        "COLOR '#fff'",
        "COLOR '#ff000'",
        "COLOR '#ff00001'",
        "COLOR '#ff0000aa1'",
        "COLOR 'ff0000'",
        "COLOR 'ff000000'",
        "COLOR ''",
        "COLOR red",
    ] {
        refused(body);
    }
}

#[test]
fn a_colour_is_three_numbers_one_string_or_one_binding() {
    // getSymbol(3, MS_NUMBER, MS_BINDING, MS_STRING), then two getInteger: anything else in the
    // three numbers, or a token after a string or a binding, fails the load.
    for body in [
        "COLOR 1 2",
        "COLOR 1 2 3 4",
        "COLOR 1 [g] 3",
        "COLOR 1 'x' 3",
        "COLOR [c] 1 2",
        "COLOR '#102030' 4",
        "COLOR ([c])",
    ] {
        refused(body);
    }
    assert_eq!(color_of("[c]"), binding("c"));
    assert_eq!(style_of("OUTLINECOLOR [o]").outline_color, binding("o"));
}
