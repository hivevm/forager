// The tests pin the behaviour of MapServer's loader and the functions they name, of the release
// pinned in scripts/fetch-msautotest.sh.
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

use forager_ports::{ResourceLoader, ResourcePath, SourceError};
use forager_style_ir::{Bound, LayerStatus, RenderMode, Rgba, Severity, Span};

use super::{Loaded, load};
use crate::test_support::{Files, codes, first_label, first_style, load_files, ready};

const WORLD: &str = r##"MAP
  NAME "world"
  EXTENT -180 -90 180 90
  SIZE 400 200
  IMAGECOLOR 255 255 255
  PROJECTION
    "proj=longlat"
    "datum=WGS84"
  END
  LAYER
    NAME "countries"
    TYPE POLYGON
    STATUS ON
    DATA "countries.shp"
    CLASS
      NAME "all"
      STYLE
        COLOR 200 220 180
        OUTLINECOLOR "#000000"
        WIDTH 0.5
      END
    END
  END
END
"##;

#[test]
fn a_mapfile_translates_into_the_style_ir() {
    // Verifies: G-2
    let loaded = load_files(&[("world.map", WORLD)]);
    assert!(loaded.diagnostics.is_empty(), "{:?}", loaded.diagnostics);
    let map = loaded.document.unwrap().map;
    assert_eq!(map.name.as_deref(), Some("world"));
    assert_eq!(map.size, Some([400, 200]));
    assert_eq!(map.image_color, Some(Rgba::opaque(255, 255, 255)));
    assert_eq!(
        map.projection.as_deref(),
        Some("+proj=longlat +datum=WGS84")
    );
    let layer = &map.layers[0];
    assert_eq!(layer.name.as_deref(), Some("countries"));
    assert_eq!(layer.render_mode, Some(RenderMode::Polygon));
    assert_eq!(layer.status, Some(LayerStatus::On));
    let source = layer.source.as_ref().unwrap();
    assert_eq!(source.kind, "local");
    assert_eq!(source.props["data"], "countries.shp");
    let style = &layer.classes[0].styles[0];
    assert_eq!(style.color, Some(Bound::Value(Rgba::opaque(200, 220, 180))));
    assert_eq!(
        style.outline_color,
        Some(Bound::Value(Rgba::opaque(0, 0, 0)))
    );
    assert_eq!(style.width, Some(Bound::Value(0.5)));
    assert_eq!(style.span, Some(Span::new("world.map", 17, 7)));
}

#[test]
fn keywords_not_yet_supported_and_out_of_scope_are_warnings() {
    let loaded = load_files(&[(
        "m.map",
        "MAP\n  WEB TEMPLATE 'x.html' END\n  LAYER NAME 'a' GROUP 'n' END\nEND\n",
    )]);
    assert!(loaded.document.is_some());
    assert_eq!(codes(&loaded), ["discarded-keyword", "unsupported-keyword"]);
    assert!(
        loaded
            .diagnostics
            .iter()
            .all(|d| d.severity == Severity::Warning)
    );
    assert_eq!(loaded.diagnostics[1].span, Some(Span::new("m.map", 3, 18)));
}

#[test]
fn a_query_layer_is_discarded() {
    let loaded = load_files(&[("m.map", "MAP LAYER NAME 'q' TYPE QUERY END END")]);
    assert!(loaded.document.as_ref().unwrap().map.layers.is_empty());
    assert_eq!(codes(&loaded), ["discarded-layer"]);
}

#[test]
fn an_include_is_spliced_in_with_its_own_spans() {
    let loaded = load_files(&[
        ("m.map", "MAP\n  INCLUDE 'layers/a.inc'\nEND\n"),
        ("layers/a.inc", "LAYER NAME 'a' END\n"),
    ]);
    assert!(loaded.diagnostics.is_empty(), "{:?}", loaded.diagnostics);
    let layer = &loaded.document.unwrap().map.layers[0];
    assert_eq!(layer.span, Some(Span::new("layers/a.inc", 1, 1)));
}

#[test]
fn includes_resolve_against_the_main_mapfiles_directory_inside_the_base_context() {
    // Verifies: Q-8
    let loaded = load_files(&[
        ("maps/m.map", "MAP INCLUDE '../inc/a.inc' END"),
        ("inc/a.inc", "INCLUDE 'b.inc'"),
        ("maps/b.inc", "LAYER NAME 'b' END"),
    ]);
    assert_eq!(codes(&loaded), Vec::<&str>::new());
    let layer = &loaded.document.unwrap().map.layers[0];
    assert_eq!(layer.span, Some(Span::new("maps/b.inc", 1, 1)));

    let loaded = load_files(&[("maps/m.map", "MAP INCLUDE '../../x.inc' END")]);
    assert_eq!(codes(&loaded), ["include-refused"]);
}

#[test]
fn an_include_outside_the_base_context_is_refused() {
    // Verifies: Q-8
    for target in ["../secret.map", "/etc/passwd", "C:\\secret.map"] {
        let text: &'static str = format!("MAP INCLUDE '{target}' END").leak();
        let loaded = load_files(&[("m.map", text)]);
        assert!(loaded.document.is_none(), "{target}");
        assert_eq!(codes(&loaded), ["include-refused"], "{target}");
    }
}

#[test]
fn include_cycles_and_deep_nesting_are_errors() {
    // Verifies: Q-9
    let cycle = load_files(&[("m.map", "MAP INCLUDE 'a' END"), ("a", "INCLUDE 'm.map'")]);
    assert_eq!(codes(&cycle), ["include-cycle"]);

    let deep = load_files(&[
        ("m.map", "MAP INCLUDE '1' END"),
        ("1", "INCLUDE '2'"),
        ("2", "INCLUDE '3'"),
        ("3", "INCLUDE '4'"),
        ("4", "INCLUDE '5'"),
        ("5", "INCLUDE '6'"),
        ("6", "NAME 'x'"),
    ]);
    assert_eq!(codes(&deep), ["include-too-deep"]);

    let five = load_files(&[
        ("m.map", "MAP INCLUDE '1' END"),
        ("1", "INCLUDE '2'"),
        ("2", "INCLUDE '3'"),
        ("3", "INCLUDE '4'"),
        ("4", "INCLUDE '5'"),
        ("5", "NAME 'x'"),
    ]);
    assert!(five.diagnostics.is_empty(), "{:?}", five.diagnostics);
}

#[test]
fn syntax_errors_fail_the_load_with_a_span() {
    let loaded = load_files(&[("m.map", "MAP\n  NAME \"unterminated\n")]);
    assert!(loaded.document.is_none());
    assert_eq!(
        loaded.diagnostics[0].span,
        Some(Span::new("m.map", 2, 8)),
        "{:?}",
        loaded.diagnostics
    );
}

#[test]
fn an_init_projection_without_a_resolver_is_a_load_error() {
    let loaded = load_files(&[("m.map", "MAP PROJECTION 'init=epsg:4326' END END")]);
    assert!(loaded.document.is_none());
    assert_eq!(codes(&loaded), ["projection-unresolved"]);
}

#[test]
fn invalid_values_are_errors() {
    let loaded = load_files(&[("m.map", "MAP SIZE 400 END")]);
    assert!(loaded.document.is_none());
    assert_eq!(codes(&loaded), ["invalid-value"]);
}

#[test]
fn colours_of_minus_one_mean_no_colour() {
    let loaded = load_files(&[(
        "m.map",
        "MAP LAYER CLASS STYLE COLOR -1 -1 -1 OUTLINECOLOR 1 2 3 END END END END",
    )]);
    let style = &loaded.document.unwrap().map.layers[0].classes[0].styles[0];
    assert_eq!(style.color, None);
    assert_eq!(
        style.outline_color,
        Some(Bound::Value(Rgba::opaque(1, 2, 3)))
    );
}

#[test]
fn the_image_colour_keeps_its_alpha_and_takes_every_channel_as_a_byte() {
    // msPrepareImage, src/mapdraw.c, hands the map's imagecolor to the renderer without
    // MS_VALID_COLOR; AGG's rgba8 keeps each channel in a byte, so -1 is 255. MapServer forces
    // the alpha to 255, where the frontend keeps the alpha a hex colour gives.
    let image_color = |text: &'static str| {
        let loaded = load_files(&[("m.map", text)]);
        assert_eq!(codes(&loaded), Vec::<&str>::new(), "{text}");
        loaded.document.unwrap().map.image_color
    };
    assert_eq!(
        image_color("MAP IMAGECOLOR 255 -1 0 END"),
        Some(Rgba::opaque(255, 255, 0))
    );
    assert_eq!(
        image_color("MAP IMAGECOLOR -1 -1 -1 END"),
        Some(Rgba::opaque(255, 255, 255))
    );
    assert_eq!(
        image_color("MAP IMAGECOLOR -2.5 0 254.9 END"),
        Some(Rgba::opaque(254, 0, 254))
    );
    assert_eq!(
        image_color("MAP IMAGECOLOR '#10203080' END"),
        Some(Rgba {
            r: 0x10,
            g: 0x20,
            b: 0x30,
            a: 0x80
        })
    );
    assert_eq!(
        image_color("MAP IMAGECOLOR '#102030' END"),
        Some(Rgba::opaque(0x10, 0x20, 0x30))
    );
    let loaded = load_files(&[("m.map", "MAP IMAGECOLOR 0 256 0 END")]);
    assert_eq!(codes(&loaded), ["invalid-value"]);
}

#[test]
fn an_include_without_a_string_keeps_the_next_token() {
    let loaded = load_files(&[("m.map", "MAP INCLUDE END")]);
    assert_eq!(codes(&loaded), ["invalid-value"]);
}

#[test]
fn an_unknown_keyword_is_reported_once() {
    let loaded = load_files(&[("m.map", "MAP FROBNICATE 1 END")]);
    assert_eq!(codes(&loaded), ["unknown-keyword"]);
}

#[test]
fn includes_that_multiply_past_the_size_limit_are_an_error() {
    // Verifies: Q-9
    // Each level includes the next 64 times: 64^4 copies of a 32 KiB file exceed the limit long
    // before they are all read.
    let leaf: &'static str = format!("NAME '{}'\n", "x".repeat(32 * 1024)).leak();
    let fan = |next: &str| -> &'static str { format!("INCLUDE '{next}'\n").repeat(64).leak() };
    let started = std::time::Instant::now();
    let loaded = load_files(&[
        ("m.map", "MAP INCLUDE '1' END"),
        ("1", fan("2")),
        ("2", fan("3")),
        ("3", fan("4")),
        ("4", leaf),
    ]);
    assert!(loaded.document.is_none());
    assert_eq!(codes(&loaded), ["mapfile-too-large"]);
    assert!(started.elapsed().as_secs() < 10, "{:?}", started.elapsed());
}

#[test]
fn diagnostics_past_the_limit_are_counted_not_listed() {
    // Verifies: Q-9
    let text: &'static str = format!("MAP\n{}END\n", "@\n".repeat(1500)).leak();
    let loaded = load_files(&[("m.map", text)]);
    assert_eq!(loaded.diagnostics.len(), super::MAX_DIAGNOSTICS + 1);
    let last = loaded.diagnostics.last().unwrap();
    assert_eq!(last.code, "diagnostics-truncated");
    assert!(last.message.starts_with("500 "), "{}", last.message);
}

#[test]
fn diagnostics_are_bounded_while_they_are_collected() {
    // Verifies: Q-9
    // Every INCLUDE names a file that is the input again: tens of thousands of cycle and depth
    // errors, of which the load keeps the first thousand and counts the rest.
    let text: &'static str = format!(
        "MAP\n{}END\n",
        (0..12)
            .map(|i| format!("INCLUDE '{i}'\n"))
            .collect::<String>()
    )
    .leak();
    let files: Vec<(&'static str, &'static str)> = std::iter::once(("m.map", text))
        .chain((0..12).map(|i| (&*format!("{i}").leak(), text)))
        .collect();
    let loaded = load_files(&files);
    assert!(loaded.document.is_none());
    assert_eq!(loaded.diagnostics.len(), super::MAX_DIAGNOSTICS + 1);
    assert_eq!(
        loaded.diagnostics.last().unwrap().code,
        "diagnostics-truncated"
    );
}

/// Knows one projection, as a host's resolver would.
struct OneProjection;

impl forager_ports::ProjectionResolver for OneProjection {
    fn resolve(&self, authority: &str, code: &str) -> Option<String> {
        (authority.eq_ignore_ascii_case("epsg") && code == "4326")
            .then(|| "+proj=longlat +datum=WGS84 +no_defs".to_owned())
    }
}

fn load_with_resolver(text: &'static str) -> Loaded {
    let loader = Files([("m.map", text)].into_iter().collect());
    ready(load(
        &ResourcePath::new("m.map").unwrap(),
        &loader,
        Some(&OneProjection),
    ))
}

#[test]
fn an_init_projection_is_resolved_by_the_hosts_resolver() {
    // Verifies: ADR-0023
    let loaded = load_with_resolver("MAP PROJECTION 'init=epsg:4326' END END");
    assert!(loaded.diagnostics.is_empty(), "{:?}", loaded.diagnostics);
    assert_eq!(
        loaded.document.unwrap().map.projection.as_deref(),
        Some("+proj=longlat +datum=WGS84 +no_defs")
    );
    // Explicit parameters come first, so that they take precedence in PROJ.
    let loaded = load_with_resolver("MAP PROJECTION 'init=epsg:4326' '+over' END END");
    assert_eq!(
        loaded.document.unwrap().map.projection.as_deref(),
        Some("+over +proj=longlat +datum=WGS84 +no_defs")
    );
    // As in MapServer, only a lower-case `init=` is a reference.
    let loaded = load_with_resolver("MAP PROJECTION 'INIT=EPSG:4326' END END");
    assert_eq!(
        loaded.document.unwrap().map.projection.as_deref(),
        Some("+INIT=EPSG:4326")
    );
    let loaded = load_with_resolver("MAP PROJECTION 'init=epsg:4326' 'init=epsg:4326' END END");
    assert_eq!(codes(&loaded), ["invalid-value"]);
}

#[test]
fn an_init_projection_the_resolver_does_not_know_is_a_load_error() {
    // Verifies: ADR-0023
    let loaded = load_with_resolver("MAP PROJECTION 'init=epsg:27700' END END");
    assert!(loaded.document.is_none());
    assert_eq!(codes(&loaded), ["projection-unresolved"]);
    assert!(loaded.diagnostics[0].message.contains("epsg:27700"));
}

/// The CQL2-JSON of the expression of each class of the first layer, `null` where it has none,
/// and of the layer's filter, after loading `text` as a mapfile.
fn expressions(text: &'static str) -> (Loaded, Vec<String>, String) {
    let loaded = load_files(&[("m.map", text)]);
    let json = |e: Option<&forager_style_ir::Expr>| serde_json::to_string(&e).unwrap();
    let (classes, filter) = match &loaded.document {
        Some(document) => {
            let layer = &document.map.layers[0];
            (
                layer
                    .classes
                    .iter()
                    .map(|c| json(c.expression.as_ref()))
                    .collect(),
                json(layer.filter.as_ref()),
            )
        }
        None => (Vec::new(), String::new()),
    };
    (loaded, classes, filter)
}

#[test]
fn the_four_kinds_of_class_expression_compare_against_the_classitem() {
    // Verifies: ADR-0019
    let (loaded, classes, _) = expressions(
        r#"MAP LAYER
             CLASS EXPRESSION "road" END
             CLASS EXPRESSION "Road"i END
             CLASS EXPRESSION /^r.*d$/i END
             CLASS EXPRESSION /x/ END
             CLASS EXPRESSION {a,b,c} END
             CLASSITEM "type"
           END END"#,
    );
    assert_eq!(codes(&loaded), Vec::<&str>::new());
    let item = r#"{"op":"to_string","args":[{"property":"type"}]}"#;
    assert_eq!(
        classes,
        [
            format!(r#"{{"op":"=","args":[{item},"road"]}}"#),
            format!(
                r#"{{"op":"=","args":[{{"op":"casei","args":[{item}]}},{{"op":"casei","args":["Road"]}}]}}"#
            ),
            format!(r#"{{"op":"regexp_like","args":[{item},"^r.*d$","i"]}}"#),
            format!(r#"{{"op":"regexp_like","args":[{item},"x",""]}}"#),
            format!(r#"{{"op":"in","args":[{item},["a","b","c"]]}}"#),
        ]
    );
}

#[test]
fn a_logical_expression_states_the_type_of_each_attribute() {
    // Verifies: ADR-0019
    let (loaded, classes, _) = expressions(
        r#"MAP LAYER CLASS EXPRESSION ([pop] >= 1000 AND "[name]" =* "x") END END END"#,
    );
    assert_eq!(codes(&loaded), Vec::<&str>::new());
    assert_eq!(
        classes,
        [concat!(
            r#"{"op":"and","args":["#,
            r#"{"op":">=","args":[{"op":"to_number","args":[{"property":"pop"}]},1000.0]},"#,
            r#"{"op":"=","args":[{"op":"casei","args":[{"op":"to_string","args":[{"property":"name"}]}]},"#,
            r#"{"op":"casei","args":["x"]}]}]}"#
        )]
    );
}

#[test]
fn operators_bind_as_in_mapserver() {
    // Verifies: ADR-0019
    let (_, classes, _) = expressions(
        r#"MAP LAYER
             CLASS EXPRESSION ([a] + 2 * 3 ^ 2 ^ 1 > 5 OR NOT [b] = 1 AND [c]) END
             CLASS EXPRESSION ("A" + "B" ~* "^a") END
             CLASS EXPRESSION (-[d] < 0 && [e] eq 2 || ! true) END
           END END"#,
    );
    let n = |p: &str| format!(r#"{{"op":"to_number","args":[{{"property":"{p}"}}]}}"#);
    assert_eq!(
        classes[0],
        format!(
            concat!(
                r#"{{"op":"or","args":["#,
                r#"{{"op":">","args":[{{"op":"+","args":[{a},{{"op":"*","args":[2.0,"#,
                r#"{{"op":"^","args":[3.0,{{"op":"^","args":[2.0,1.0]}}]}}]}}]}},5.0]}},"#,
                r#"{{"op":"and","args":[{{"op":"not","args":[{{"op":"=","args":[{b},1.0]}}]}},"#,
                r#"{{"op":"<>","args":[{c},0.0]}}]}}]}}"#
            ),
            a = n("a"),
            b = n("b"),
            c = n("c")
        )
    );
    assert_eq!(
        classes[1],
        r#"{"op":"regexp_like","args":[{"op":"concat","args":["A","B"]},"^a","i"]}"#
    );
    assert_eq!(
        classes[2],
        format!(
            concat!(
                r#"{{"op":"or","args":[{{"op":"and","args":["#,
                r#"{{"op":"<","args":[{d},0.0]}},"#,
                r#"{{"op":"=","args":[{e},2.0]}}]}},{{"op":"not","args":[true]}}]}}"#
            ),
            d = n("d"),
            e = n("e")
        )
    );
}

#[test]
fn a_layer_filter_splits_in_lists_at_load_and_compares_times() {
    // Verifies: ADR-0019
    let (loaded, _, filter) = expressions(
        r#"MAP LAYER FILTER ('[a]' in 'A,B,C' AND `[t]` > `2010-12-01` AND [n] IN "1,2") END END"#,
    );
    assert_eq!(codes(&loaded), Vec::<&str>::new());
    assert_eq!(
        filter,
        concat!(
            r#"{"op":"and","args":["#,
            r#"{"op":"in","args":[{"op":"to_string","args":[{"property":"a"}]},["A","B","C"]]},"#,
            r#"{"op":"t_after","args":[{"op":"to_timestamp","args":[{"property":"t"}]},{"date":"2010-12-01"}]},"#,
            r#"{"op":"in","args":[{"op":"to_number","args":[{"property":"n"}]},[1.0,2.0]]}]}"#
        )
    );
    let (_, _, filter) = expressions(r#"MAP LAYER FILTER "x" FILTERITEM "kind" END END"#);
    assert_eq!(
        filter,
        r#"{"op":"=","args":[{"op":"to_string","args":[{"property":"kind"}]},"x"]}"#
    );
}

#[test]
fn an_expression_outside_the_subset_is_a_warning_and_never_matches() {
    // Verifies: ADR-0019
    for text in [
        "MAP LAYER CLASS EXPRESSION (intersects([shape], fromtext('POINT(1 1)'))) END END END",
        r#"MAP LAYER CLASS EXPRESSION ("a" IN ("b" + "c")) END END END"#,
        "MAP LAYER CLASS EXPRESSION (`[t]` > `2004/01/01`) END END END",
    ] {
        let (loaded, classes, _) = expressions(text);
        assert_eq!(codes(&loaded), ["unsupported-expression"], "{text}");
        assert_eq!(loaded.diagnostics[0].severity, Severity::Warning);
        assert_eq!(classes, ["false"], "{text}");
    }
}

#[test]
fn an_expression_mapserver_cannot_parse_is_a_warning_and_never_matches() {
    for text in [
        r#"MAP LAYER CLASS EXPRESSION ([a] = "x") END END END"#,
        "MAP LAYER CLASS EXPRESSION ([a] =! 1) END END END",
        "MAP LAYER CLASS EXPRESSION ([a] -1) END END END",
        r#"MAP LAYER CLASS EXPRESSION "x" END END END"#,
    ] {
        let (loaded, classes, _) = expressions(text);
        assert_eq!(codes(&loaded), ["invalid-expression"], "{text}");
        assert_eq!(classes, ["false"], "{text}");
    }
}

#[test]
fn an_expression_nested_past_the_limit_fails_the_load() {
    // Verifies: Q-9, ADR-0019
    let depth = forager_style_ir::MAX_EXPRESSION_DEPTH;
    let nots = |n: usize| -> &'static str {
        format!(
            "MAP LAYER CLASS EXPRESSION ({}true) END END END",
            "NOT ".repeat(n)
        )
        .leak()
    };
    let (loaded, _, _) = expressions(nots(depth - 1));
    assert_eq!(codes(&loaded), Vec::<&str>::new());
    for past in [
        nots(depth),
        format!(
            "MAP LAYER CLASS EXPRESSION ({}1{}) END END END",
            "(".repeat(200),
            ")".repeat(200)
        )
        .leak(),
    ] {
        let (loaded, _, _) = expressions(past);
        assert!(loaded.document.is_none());
        assert_eq!(codes(&loaded), ["expression-too-deep"]);
    }
}

#[test]
fn every_expression_reads_back_equal_from_yaml_and_json() {
    // Verifies: ADR-0019
    let loaded = load_files(&[(
        "m.map",
        r#"MAP LAYER
             FILTER ([a] > 1.5 OR '[b]' ~ "x\"y" AND `[t]` = `2010-12-01T10:00:00Z`)
             CLASS EXPRESSION {1000,true,,x} END
             CLASS EXPRESSION (tostring([p], "%.2f") + commify("12") = upper("a")) END
             CLASS EXPRESSION (round([q], 2) + length("ab") = 4 AND [x] % 2 = 1) END
             CLASSITEM "k"
           END END"#,
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

/// The codes and the CQL2-JSON of a class whose `EXPRESSION` is `expression`, in a layer whose
/// `CLASSITEM` is `k`.
fn class_expression(expression: &str) -> (Vec<String>, String) {
    let text = format!(r#"MAP LAYER CLASSITEM "k" CLASS EXPRESSION {expression} END END END"#);
    let (loaded, classes, _) = expressions(text.leak());
    let codes = codes(&loaded).into_iter().map(str::to_owned).collect();
    (codes, classes.join(","))
}

const A: &str = r#"{"op":"to_number","args":[{"property":"a"}]}"#;
const K: &str = r#"{"op":"to_string","args":[{"property":"k"}]}"#;

#[test]
fn an_empty_expression_matches_every_feature() {
    // msEvalExpression returns true for an empty expression before it reads the item, so an
    // empty string, list, or expression matches even without a CLASSITEM or FILTERITEM.
    for expression in [r#""""#, "''", "{}", "()"] {
        assert_eq!(class_expression(expression), (vec![], "true".to_owned()));
    }
    let (loaded, _, filter) = expressions(r#"MAP LAYER FILTER "" END END"#);
    assert_eq!(codes(&loaded), Vec::<&str>::new());
    assert_eq!(filter, "true");
}

#[test]
fn unary_minus_leaves_its_operand_as_mapserver_does() {
    // MapServer's grammar: `'-' math_exp %prec NEG { $$ = $2; }`; `-1` is a number token.
    assert_eq!(
        class_expression("(-[a] = 1)").1,
        format!(r#"{{"op":"=","args":[{A},1.0]}}"#)
    );
    assert_eq!(
        class_expression("(- 2 ^ [a] > -1)").1,
        format!(r#"{{"op":">","args":[{{"op":"^","args":[2.0,{A}]}},-1.0]}}"#)
    );
    assert_eq!(class_expression(r#"(-"x")"#).0, ["invalid-expression"]);
}

#[test]
fn the_outer_parentheses_are_the_tokens_and_not_part_of_the_expression() {
    // The lexer's rule `\(.*\)` runs to the last `)` on the line and loadExpression keeps what
    // is between the first `(` and that `)`: of `([a] = 1) OR ([a] = 2)` it parses
    // `[a] = 1) OR ([a] = 2`, which the grammar does not accept.
    assert_eq!(
        class_expression("([a] = 1) OR ([a] = 2)"),
        (vec!["invalid-expression".to_owned()], "false".to_owned())
    );
    assert_eq!(
        class_expression("(([a] = 1) OR ([a] = 2))").1,
        format!(
            r#"{{"op":"or","args":[{{"op":"=","args":[{A},1.0]}},{{"op":"=","args":[{A},2.0]}}]}}"#
        )
    );
}

#[test]
fn a_single_bar_or_a_newline_ends_the_expression_as_the_lexer_does() {
    // The lexer's rule `<EXPRESSION_STRING>[\r|\n|\0]` returns 0, the end of the token list.
    assert_eq!(
        class_expression(r#"([a] = 1 | ] "unterminated)"#).1,
        format!(r#"{{"op":"=","args":[{A},1.0]}}"#)
    );
    assert_eq!(
        class_expression("([a] = 1 || [a] = 2)").1,
        format!(
            r#"{{"op":"or","args":[{{"op":"=","args":[{A},1.0]}},{{"op":"=","args":[{A},2.0]}}]}}"#
        )
    );
    let text = forager_style_ir::Expr::call(
        forager_style_ir::Op::ToNumber,
        vec![forager_style_ir::Expr::call(
            forager_style_ir::Op::Tostring,
            vec![
                forager_style_ir::Expr::Number(1.0),
                forager_style_ir::Expr::String("%g".to_owned()),
            ],
        )],
    );
    assert_eq!(crate::expression::number_value("(1\n+ x)"), Ok(text));
}

#[test]
fn keywords_are_read_by_the_lexers_longest_match() {
    // The expression state has a rule for each keyword and none for a word.
    assert_eq!(
        class_expression("([a] = 1 AnDnOt [a] = 2)").1,
        format!(
            r#"{{"op":"and","args":[{{"op":"=","args":[{A},1.0]}},{{"op":"not","args":[{{"op":"=","args":[{A},2.0]}}]}}]}}"#
        )
    );
    assert_eq!(
        class_expression("(nottrue)").1,
        r#"{"op":"not","args":[true]}"#
    );
    assert_eq!(
        class_expression("([a] = 1 xor true)").0,
        ["invalid-expression"]
    );
    for (word, op) in [
        ("eq", "="),
        ("ne", "<>"),
        ("lt", "<"),
        ("gt", ">"),
        ("le", "<="),
        ("ge", ">="),
    ] {
        assert_eq!(
            class_expression(&format!("([a] {word} 1)")).1,
            format!(r#"{{"op":"{op}","args":[{A},1.0]}}"#),
            "{word}"
        );
    }
}

#[test]
fn a_binding_runs_to_the_last_bracket_the_lexers_longest_match_reaches() {
    // The rules `\[(?:\]\]|[^\]])*\]`, `"\[(?:""|[^"])*\]"`, and `` `\[(?:``|[^`])*\]` ``, the
    // doubled character read as one by msStringUnescape.
    let s = |name: &str| format!(r#"{{"op":"to_string","args":[{{"property":"{name}"}}]}}"#);
    assert_eq!(
        class_expression(r#"("[a]b]" = "x")"#).1,
        format!(r#"{{"op":"=","args":[{},"x"]}}"#, s("a]b"))
    );
    assert_eq!(
        class_expression(r#"('[a''b]' = "x")"#).1,
        format!(r#"{{"op":"=","args":[{},"x"]}}"#, s("a'b"))
    );
    assert_eq!(
        class_expression(r#"('[a]' = "[b] c")"#).1,
        format!(r#"{{"op":"=","args":[{},"[b] c"]}}"#, s("a"))
    );
    assert_eq!(
        class_expression("([a]]b] = 1)").1,
        r#"{"op":"=","args":[{"op":"to_number","args":[{"property":"a]b"}]},1.0]}"#
    );
    assert_eq!(
        class_expression("(`[t``x]` = `2010-12-01`)").1,
        r#"{"op":"t_equals","args":[{"op":"to_timestamp","args":[{"property":"t`x"}]},{"date":"2010-12-01"}]}"#
    );
    assert_eq!(
        class_expression("([a]] = 1 OR [b] = 2)").1,
        r#"{"op":"=","args":[{"op":"to_number","args":[{"property":"a] = 1 OR [b"}]},2.0]}"#
    );
    assert_eq!(
        class_expression("([SHAPE] = 1)").0,
        ["unsupported-expression"]
    );
}

#[test]
fn a_number_the_ir_cannot_hold_is_a_warning() {
    // atof reads `1e999` as infinity, which JSON cannot carry.
    for expression in ["([a] > 1e999)", r#"([a] IN "1,1e999")"#] {
        assert_eq!(
            class_expression(expression),
            (
                vec!["unsupported-expression".to_owned()],
                "false".to_owned()
            )
        );
    }
}

#[test]
fn numbers_take_the_lexers_forms_and_lists_split_without_trimming() {
    // `-?[0-9]+`, `-?[0-9]+\.[0-9]*`, `-?\.[0-9]*`, each with an exponent; `.` is atof's 0.
    assert_eq!(
        class_expression("([a] IN '.5, 2x,' AND [a] < 1. AND [a] = -1.e2 AND [a] = .)").1,
        format!(
            r#"{{"op":"and","args":[{{"op":"in","args":[{A},[0.5,2.0,0.0]]}},{{"op":"<","args":[{A},1.0]}},{{"op":"=","args":[{A},-100.0]}},{{"op":"=","args":[{A},0.0]}}]}}"#
        )
    );
}

#[test]
fn a_whole_expression_is_true_by_its_type() {
    // The grammar's `input` rule: a number is true when not 0, a string always.
    assert_eq!(
        class_expression("([a] - 1)").1,
        format!(r#"{{"op":"<>","args":[{{"op":"-","args":[{A},1.0]}},0.0]}}"#)
    );
    assert_eq!(class_expression(r#"("[k]")"#), (vec![], "true".to_owned()));
    assert_eq!(class_expression("(`2010-12-01`)").0, ["invalid-expression"]);
    assert_eq!(
        class_expression("(true = ([a] > 1))").1,
        format!(r#"{{"op":"=","args":[true,{{"op":">","args":[{A},1.0]}}]}}"#)
    );
}

#[test]
fn each_comparison_takes_the_types_mapserver_has_a_rule_for() {
    // A comparison of conditions has only `=`; `=*` is `strcasecmp` on strings, `==` on numbers
    // and times; `~` and `IN` take a string on the left, `IN` a number too.
    assert_eq!(
        class_expression("(true != false)").0,
        ["invalid-expression"]
    );
    assert_eq!(
        class_expression("(true =* false)").0,
        ["invalid-expression"]
    );
    assert_eq!(
        class_expression("([a] =* 2)").1,
        format!(r#"{{"op":"=","args":[{A},2.0]}}"#)
    );
    let t = r#"{"op":"to_timestamp","args":[{"property":"t"}]}"#;
    let d = r#"{"date":"2010-12-01"}"#;
    for (op, cql) in [
        ("=*", format!(r#"{{"op":"t_equals","args":[{t},{d}]}}"#)),
        (
            "!=",
            format!(r#"{{"op":"not","args":[{{"op":"t_equals","args":[{t},{d}]}}]}}"#),
        ),
        (
            "<=",
            format!(r#"{{"op":"not","args":[{{"op":"t_after","args":[{t},{d}]}}]}}"#),
        ),
        (
            ">=",
            format!(r#"{{"op":"not","args":[{{"op":"t_before","args":[{t},{d}]}}]}}"#),
        ),
        ("<", format!(r#"{{"op":"t_before","args":[{t},{d}]}}"#)),
    ] {
        assert_eq!(
            class_expression(&format!("(`[t]` {op} `2010-12-01`)")).1,
            cql,
            "{op}"
        );
    }
    for expression in [
        "([a] ~ 'x')",
        "('x' IN [a])",
        "(`[t]` IN 'x')",
        "([a] = '1')",
    ] {
        assert_eq!(
            class_expression(expression).0,
            ["invalid-expression"],
            "{expression}"
        );
    }
    assert_eq!(
        class_expression(r#"("[k]" !~ "x")"#).0,
        ["invalid-expression"]
    );
}

#[test]
fn strings_in_an_expression_take_the_lexers_escapes() {
    // MSSTRING: `\'`, `\"`, `\\` escape, any other backslash stays, and `"i` closes a string.
    assert_eq!(
        class_expression(r#"('a\'b\q\\' = "x"i)"#).1,
        r#"{"op":"=","args":["a'b\\q\\","x"]}"#
    );
    assert_eq!(
        class_expression(r#"("a" + 'it''s' = "x")"#).0,
        ["invalid-expression"]
    );
}

#[test]
fn functions_take_the_arguments_of_their_rules() {
    assert_eq!(
        class_expression("(round([a]) = round([a], 5))").1,
        format!(
            r#"{{"op":"=","args":[{{"op":"round","args":[{A}]}},{{"op":"round","args":[{A},5.0]}}]}}"#
        )
    );
    for expression in [
        "(round([a], 1, 2) = 1)",
        "(length([a]) = 1)",
        r#"(tostring("x", "%g") = "x")"#,
        r#"(tostring([a]) = "x")"#,
        r#"(upper "x" = "x")"#,
    ] {
        assert_eq!(
            class_expression(expression).0,
            ["invalid-expression"],
            "{expression}"
        );
    }
    assert_eq!(
        class_expression("(area([shape]) > 1)").0,
        ["unsupported-expression"]
    );
}

#[test]
fn the_last_expression_and_the_last_item_hold_wherever_they_stand() {
    // loadClass and loadLayer overwrite each; msEvalExpression reads the items when it draws.
    let (loaded, classes, filter) = expressions(
        r#"MAP LAYER
             FILTERITEM "f1"
             FILTER /x/
             CLASS EXPRESSION "a" EXPRESSION {b,c} END
             CLASSITEM "k1"
             CLASSITEM "k2"
             FILTERITEM "f2"
           END END"#,
    );
    assert_eq!(codes(&loaded), Vec::<&str>::new());
    assert_eq!(
        classes,
        [r#"{"op":"in","args":[{"op":"to_string","args":[{"property":"k2"}]},["b","c"]]}"#]
    );
    assert_eq!(
        filter,
        r#"{"op":"regexp_like","args":[{"op":"to_string","args":[{"property":"f2"}]},"x",""]}"#
    );
    let (loaded, _, filter) = expressions(r#"MAP LAYER CLASSITEM "k" FILTER {a} END END"#);
    assert_eq!(codes(&loaded), ["invalid-expression"]);
    assert_eq!(filter, "false");
    assert_eq!(
        class_expression("{ a,b }").1,
        format!(r#"{{"op":"in","args":[{K},[" a","b "]]}}"#)
    );
    assert_eq!(
        class_expression("road").1,
        format!(r#"{{"op":"=","args":[{K},"road"]}}"#)
    );
    assert_eq!(
        class_expression("5"),
        (vec!["invalid-value".to_owned()], String::new())
    );
}

#[test]
fn colours_take_mapservers_range_and_a_negative_channel_wraps() {
    let colour = |text: &'static str| {
        let (loaded, style) = first_style(text);
        (codes(&loaded).join(","), style.and_then(|s| s.color))
    };
    let value = |r, g, b| Some(Bound::Value(Rgba::opaque(r, g, b)));
    assert_eq!(
        colour("MAP LAYER CLASS STYLE COLOR -255 -255 -255 END END END END"),
        (String::new(), value(1, 1, 1))
    );
    assert_eq!(
        colour("MAP LAYER CLASS STYLE COLOR -2 0 254.9 END END END END"),
        (String::new(), value(254, 0, 254))
    );
    assert_eq!(
        colour("MAP LAYER CLASS STYLE COLOR 255 -1 0 END END END END"),
        (String::new(), None)
    );
    let (loaded, _) = first_style("MAP LAYER CLASS STYLE COLOR 300 0 0 END END END END");
    assert_eq!(codes(&loaded), ["invalid-value"]);
}

#[test]
fn style_numbers_outside_mapservers_ranges_are_errors() {
    for text in [
        "MAP LAYER CLASS STYLE SIZE 0 END END END END",
        "MAP LAYER CLASS STYLE WIDTH -1 END END END END",
        "MAP LAYER CLASS STYLE ANGLE 400 END END END END",
        "MAP LAYER CLASS STYLE WIDTH ([a]) END END END END",
        "MAP IMAGECOLOR [c] END",
    ] {
        let loaded = load_files(&[("m.map", text)]);
        assert!(loaded.document.is_none(), "{text}");
        assert_eq!(codes(&loaded), ["invalid-value"], "{text}");
    }
    let (loaded, style) = first_style("MAP LAYER CLASS STYLE ANGLE AUTO END END END END");
    assert_eq!(codes(&loaded), Vec::<&str>::new());
    let style = style.unwrap();
    assert_eq!(style.angle, None);
    assert!(style.auto_angle);
}

#[test]
fn lengths_are_carried_as_written_with_the_factors_that_scale_them() {
    use forager_style_ir::Units;
    let loaded = load_files(&[(
        "m.map",
        "MAP RESOLUTION 144 DEFRESOLUTION 96
           LAYER SIZEUNITS meters CLASS STYLE SIZE 10 WIDTH 2 PATTERN 10 5 END END END END
         END",
    )]);
    assert_eq!(codes(&loaded), Vec::<&str>::new());
    let map = loaded.document.unwrap().map;
    assert_eq!(
        (map.resolution, map.def_resolution),
        (Some(144.0), Some(96.0))
    );
    assert_eq!(map.layers[0].size_units, Some(Units::Meters));
    let style = &map.layers[0].classes[0].styles[0];
    assert_eq!(style.size, Some(Bound::Value(10.0)));
    assert_eq!(style.width, Some(Bound::Value(2.0)));
    assert_eq!(style.pattern, [10.0, 5.0]);
}

#[test]
fn true_and_on_false_and_off_stand_for_each_other() {
    use forager_style_ir::Force;
    let (_, label) = first_label(&[(
        "m.map",
        "MAP LAYER CLASS LABEL FORCE true PARTIALS ON END END END END",
    )]);
    let label = label.unwrap();
    assert_eq!((label.force, label.partials), (Some(Force::On), Some(true)));
    let loaded = load_files(&[("m.map", "MAP LAYER STATUS FALSE END END")]);
    assert_eq!(
        loaded.document.unwrap().map.layers[0].status,
        Some(LayerStatus::Off)
    );
}

#[test]
fn a_layers_projection_resolves_as_the_maps_does() {
    // Verifies: ADR-0023
    use forager_style_ir::LayerProjection;
    let text = "MAP LAYER PROJECTION 'init=epsg:4326' END END END";
    let loaded = load_with_resolver(text);
    assert_eq!(codes(&loaded), Vec::<&str>::new());
    assert_eq!(
        loaded.document.unwrap().map.layers[0].projection,
        Some(LayerProjection::Proj4(
            "+proj=longlat +datum=WGS84 +no_defs".to_owned()
        ))
    );
    let loaded = load_files(&[("m.map", text)]);
    assert!(loaded.document.is_none());
    assert_eq!(codes(&loaded), ["projection-unresolved"]);
    // MapServer reads the projection of an AUTO layer from its source (msSHPLayerOpen,
    // msDrawRasterLayerLow).
    let loaded = load_files(&[("m.map", "MAP LAYER PROJECTION auto END END END")]);
    assert_eq!(codes(&loaded), Vec::<&str>::new());
    assert_eq!(
        loaded.document.unwrap().map.layers[0].projection,
        Some(LayerProjection::FromSource)
    );
    // MapServer compares the whole first parameter, so this one goes to PROJ.
    let loaded = load_files(&[("m.map", "MAP LAYER PROJECTION 'auto over' END END END")]);
    assert_eq!(
        loaded.document.unwrap().map.layers[0].projection,
        Some(LayerProjection::Proj4("+auto +over".to_owned()))
    );
    // A map has no source to declare one.
    let loaded = load_files(&[("m.map", "MAP PROJECTION AUTO END END")]);
    assert_eq!(codes(&loaded), ["unsupported-value"]);
    assert_eq!(loaded.document.unwrap().map.projection, None);
}

#[test]
fn scale_ranges_load_on_layers_classes_labels_and_styles() {
    let loaded = load_files(&[(
        "m.map",
        "MAP UNITS dd
           LAYER MINSCALEDENOM 1000 MAXSCALE 50000 SYMBOLSCALE 25000
             CLASS MINSCALE 1000 MAXSCALEDENOM 50000
               STYLE SIZE 10 MINSIZE 4 MAXSIZE 20 MINSCALEDENOM 10 MAXSCALEDENOM 20 END
               LABEL MINSCALEDENOM 30 MAXSCALEDENOM 40 END
             END
           END
         END",
    )]);
    assert_eq!(codes(&loaded), Vec::<&str>::new());
    let document = loaded.document.unwrap();
    let map = &document.map;
    assert_eq!(map.units, Some(forager_style_ir::Units::Degrees));
    let layer = &map.layers[0];
    let class = &layer.classes[0];
    let ranges = [
        (layer.min_scale_denom, layer.max_scale_denom),
        (class.min_scale_denom, class.max_scale_denom),
        (
            class.styles[0].min_scale_denom,
            class.styles[0].max_scale_denom,
        ),
        (
            class.labels[0].min_scale_denom,
            class.labels[0].max_scale_denom,
        ),
    ];
    assert_eq!(
        ranges,
        [
            (Some(1000.0), Some(50000.0)),
            (Some(1000.0), Some(50000.0)),
            (Some(10.0), Some(20.0)),
            (Some(30.0), Some(40.0)),
        ],
        "the old spellings load as the new"
    );
    assert_eq!(layer.symbol_scale_denom, Some(25000.0));
    let style = &class.styles[0];
    assert_eq!(
        (style.size.clone(), style.min_size, style.max_size),
        (Some(Bound::Value(10.0)), Some(4.0), Some(20.0))
    );
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
fn scale_values_out_of_mapservers_range_are_errors() {
    for text in [
        "MAP LAYER MINSCALEDENOM -1 END END",
        "MAP LAYER SYMBOLSCALEDENOM 0.5 END END",
        "MAP LAYER CLASS STYLE MAXSCALEDENOM -1 END END END END",
        "MAP UNITS pixels END",
    ] {
        let loaded = load_files(&[("m.map", text)]);
        assert_eq!(codes(&loaded), ["invalid-value"], "{text}");
    }
}

const EPSG2: &str = "# A custom projection, as msautotest's wxs/data/epsg2 holds it.
<42304> +proj=lcc +lat_1=49 +lat_2=77 +units=m +no_defs  no_defs <>
<1> +proj=longlat +nadgrids=ntv1_can.dat <>
";

fn layer_projection(files: &[(&'static str, &'static str)]) -> Loaded {
    load_files(files)
}

#[test]
fn an_init_file_named_by_a_path_is_read_inside_the_base_context() {
    // Verifies: ADR-0023
    use forager_style_ir::LayerProjection;
    let lcc = "+proj=lcc +lat_1=49 +lat_2=77 +units=m +no_defs +no_defs";
    for (map, init) in [
        ("m.map", "./data/epsg2"),
        ("m.map", "data/epsg2"),
        ("maps/m.map", "../data/epsg2"),
    ] {
        let text = format!("MAP LAYER PROJECTION 'init={init}:42304' END END END");
        let text: &'static str = Box::leak(text.into_boxed_str());
        let loaded = layer_projection(&[(map, text), ("data/epsg2", EPSG2)]);
        assert_eq!(codes(&loaded), Vec::<&str>::new(), "{init}");
        assert_eq!(
            loaded.document.unwrap().map.layers[0].projection,
            Some(LayerProjection::Proj4(lcc.to_owned())),
            "{init}"
        );
    }
    // The map's projection reads one the same way, and explicit parameters still come first.
    let loaded = layer_projection(&[
        (
            "m.map",
            "MAP PROJECTION 'init=./data/epsg2:42304' '+over' END END",
        ),
        ("data/epsg2", EPSG2),
    ]);
    assert_eq!(
        loaded.document.unwrap().map.projection.as_deref(),
        Some(format!("+over {lcc}").as_str())
    );
}

#[test]
fn an_init_file_outside_the_base_context_or_without_the_code_is_a_load_error() {
    // Verifies: ADR-0023
    for (text, named) in [
        (
            "MAP LAYER PROJECTION 'init=../epsg2:42304' END END END",
            "../epsg2",
        ),
        (
            "MAP LAYER PROJECTION 'init=/etc/epsg2:42304' END END END",
            "/etc/epsg2",
        ),
        (
            "MAP LAYER PROJECTION 'init=./data/missing:42304' END END END",
            "data/missing",
        ),
        (
            "MAP LAYER PROJECTION 'init=./data/epsg2:4' END END END",
            "`4`",
        ),
        (
            "MAP LAYER PROJECTION 'init=./data/epsg2:1' END END END",
            "grid",
        ),
    ] {
        let loaded = layer_projection(&[("m.map", text), ("data/epsg2", EPSG2)]);
        assert!(loaded.document.is_none(), "{text}");
        assert_eq!(codes(&loaded), ["projection-unresolved"], "{text}");
        assert!(
            loaded.diagnostics[0].message.contains(named),
            "{text}: {}",
            loaded.diagnostics[0].message
        );
    }
    // A name without a `/` is an authority, which only a resolver knows.
    let loaded = layer_projection(&[
        (
            "m.map",
            "MAP LAYER PROJECTION 'init=epsg2:42304' END END END",
        ),
        ("epsg2", EPSG2),
    ]);
    assert_eq!(codes(&loaded), ["projection-unresolved"]);
    assert!(loaded.diagnostics[0].message.contains("resolver"));
}

#[test]
fn an_init_file_is_read_once_and_counts_toward_the_size_limit() {
    // Verifies: ADR-0023
    let text = "MAP LAYER PROJECTION 'init=./data/epsg2:42304' END END
      LAYER PROJECTION 'init=data/epsg2:42304' END END END";
    let reads = std::cell::RefCell::new(Vec::new());
    struct Counting<'a>(Files, &'a std::cell::RefCell<Vec<String>>);
    impl ResourceLoader for Counting<'_> {
        async fn load(&self, path: &ResourcePath) -> Result<Vec<u8>, SourceError> {
            self.1.borrow_mut().push(path.as_str().to_owned());
            self.0.load(path).await
        }
    }
    let files = Files(
        [("m.map", text), ("data/epsg2", EPSG2)]
            .into_iter()
            .collect(),
    );
    let loaded = ready(load(
        &ResourcePath::new("m.map").unwrap(),
        &Counting(files, &reads),
        None,
    ));
    assert_eq!(codes(&loaded), Vec::<&str>::new());
    assert_eq!(*reads.borrow(), ["m.map", "data/epsg2"]);
    let big: &'static str = Box::leak("#".repeat(super::MAX_TOTAL_BYTES).into_boxed_str());
    let loaded = layer_projection(&[("m.map", text), ("data/epsg2", big)]);
    assert_eq!(codes(&loaded), ["mapfile-too-large"]);
}

#[test]
fn an_init_file_is_read_as_proj_reads_it() {
    // Verifies: ADR-0023
    use forager_style_ir::LayerProjection;
    let file = "<1> +proj=first\n  +ellps=GRS80 # a comment\n <>\n<1> +proj=second <>\n";
    let loaded = layer_projection(&[
        (
            "m.map",
            "MAP LAYER PROJECTION 'init=./d/a:b/epsg2:1' END END END",
        ),
        ("d/a:b/epsg2", file),
    ]);
    assert_eq!(codes(&loaded), Vec::<&str>::new());
    assert_eq!(
        loaded.document.unwrap().map.layers[0].projection,
        Some(LayerProjection::Proj4(
            "+proj=first +ellps=GRS80".to_owned()
        )),
        "the code follows the last `:`, a definition runs across lines, and the first one holds"
    );
    let loaded = layer_projection(&[(
        "m.map",
        "MAP LAYER PROJECTION 'init=C:/x/epsg2:1' END END END",
    )]);
    assert_eq!(codes(&loaded), ["projection-unresolved"]);
    assert!(
        loaded.diagnostics[0].message.contains("refused"),
        "{}",
        loaded.diagnostics[0].message
    );
}

#[test]
fn an_init_file_reads_the_format_cases_both_readers_share() {
    // Verifies: ADR-0023
    let format = include_str!("../../adapter-epsg/tests/fixtures/init-file-format.txt");
    let file = crate::init_file::InitFile::new(ResourcePath::new("epsg2").unwrap(), format);
    for line in format.lines() {
        let Some(expect) = line.strip_prefix("# expect ") else {
            continue;
        };
        let (code, outcome) = expect.split_once(' ').unwrap();
        match outcome.strip_prefix("= ") {
            Some(definition) => assert_eq!(file.definition(code), Ok(definition), "{code}"),
            None => {
                let reason = outcome.strip_prefix("refused ").unwrap();
                let why = file.definition(code).unwrap_err();
                assert!(why.contains(reason), "{code}: {why}");
            }
        }
    }
}

#[test]
fn an_init_file_tag_that_is_no_code_is_skipped_as_proj_skips_it() {
    let file = crate::init_file::InitFile::new(
        ResourcePath::new("epsg2").unwrap(),
        "<1 +proj=longlat\n<2> +proj=merc <>\n< 3 > +proj=merc <>\n<4> +proj=longlat <5> +proj=merc <>\n",
    );
    assert_eq!(
        file.definition("2"),
        Ok("+proj=merc"),
        "an unclosed tag does not swallow the next"
    );
    assert!(file.definition("1").is_err());
    assert!(
        file.definition("3").is_err(),
        "the code is compared as written"
    );
    assert_eq!(file.definition("4"), Ok("+proj=longlat"));
    assert!(
        file.definition("5").is_err(),
        "a code inside a line is none"
    );
}

#[test]
fn the_translations_warnings_past_the_limit_are_counted() {
    // Verifies: Q-9
    let lines = "  ANGLE 0\n".repeat(super::MAX_DIAGNOSTICS + 500);
    let text: &'static str = Box::leak(format!("MAP\n{lines}END\n").into_boxed_str());
    let loaded = load_files(&[("m.map", text)]);
    assert_eq!(loaded.diagnostics.len(), super::MAX_DIAGNOSTICS + 1);
    let last = loaded.diagnostics.last().unwrap();
    assert_eq!(last.code, "diagnostics-truncated");
    assert!(last.message.starts_with("500 more"), "{}", last.message);
}

#[test]
fn an_error_past_the_limit_still_fails_the_load() {
    // Verifies: Q-9
    let lines = "  ANGLE 0\n".repeat(super::MAX_DIAGNOSTICS + 500);
    let text = format!("MAP\n{lines}  PROJECTION 'init=epsg:4326' END\nEND\n");
    let text: &'static str = Box::leak(text.into_boxed_str());
    let loaded = load_files(&[("m.map", text)]);
    assert!(
        loaded.document.is_none(),
        "the dropped error still fails the load"
    );
    assert!(
        loaded
            .diagnostics
            .last()
            .unwrap()
            .message
            .starts_with("501 more")
    );
}

#[test]
fn an_auto_projection_names_no_init_file() {
    // Verifies: ADR-0023
    use forager_style_ir::LayerProjection;
    let loaded = layer_projection(&[(
        "m.map",
        "MAP LAYER PROJECTION 'AUTO' 'init=./data/epsg2:1' END END END",
    )]);
    assert_eq!(
        codes(&loaded),
        Vec::<&str>::new(),
        "{:?}",
        loaded.diagnostics
    );
    assert_eq!(
        loaded.document.unwrap().map.layers[0].projection,
        Some(LayerProjection::FromSource)
    );
    // Nor is the file read, or counted toward the size limit.
    let reads = std::cell::RefCell::new(Vec::new());
    struct Counting<'a>(Files, &'a std::cell::RefCell<Vec<String>>);
    impl ResourceLoader for Counting<'_> {
        async fn load(&self, path: &ResourcePath) -> Result<Vec<u8>, SourceError> {
            self.1.borrow_mut().push(path.as_str().to_owned());
            self.0.load(path).await
        }
    }
    let text = "MAP LAYER PROJECTION 'AUTO' 'init=./data/epsg2:1' END END END";
    let files = Files(
        [("m.map", text), ("data/epsg2", EPSG2)]
            .into_iter()
            .collect(),
    );
    ready(load(
        &ResourcePath::new("m.map").unwrap(),
        &Counting(files, &reads),
        None,
    ));
    assert_eq!(*reads.borrow(), ["m.map"]);
}

#[test]
fn every_named_file_past_the_size_limit_is_refused_at_what_names_it() {
    // Verifies: Q-9
    let big: &'static str = Box::leak("#".repeat(super::MAX_TOTAL_BYTES).into_boxed_str());
    for (text, file, line, column) in [
        ("MAP\n  SYMBOLSET 's.sym'\nEND\n", "s.sym", 2, 3),
        ("MAP\n  FONTSET 'f.lst'\nEND\n", "f.lst", 2, 3),
        (
            "MAP\n  PROJECTION 'init=./e/epsg2:1' END\nEND\n",
            "e/epsg2",
            2,
            14,
        ),
    ] {
        let loaded = load_files(&[("m.map", text), (file, big)]);
        assert_eq!(codes(&loaded), ["mapfile-too-large"], "{file}");
        let span = loaded.diagnostics[0].span.as_ref().unwrap();
        assert_eq!((span.line, span.column), (line, column), "{file}");
    }
    // Past the bound, a file named later is refused unread: one error, not one per file.
    let loaded = load_files(&[
        (
            "m.map",
            "MAP\n  SYMBOLSET 's.sym'\n  FONTSET 'missing.lst'\nEND\n",
        ),
        ("s.sym", big),
    ]);
    assert_eq!(codes(&loaded), ["mapfile-too-large"]);
}

/// The `data` of the map's first layer, as the frontend resolves it.
fn first_data(loaded: &Loaded) -> Option<String> {
    let layer = &loaded.document.as_ref().unwrap().map.layers[0];
    layer.source.as_ref().unwrap().props.get("data").cloned()
}

#[test]
fn a_local_layers_data_lies_beneath_shapepath_in_the_base_context() {
    // msBuildPath3: the main mapfile's directory, then SHAPEPATH, then DATA. The map's last
    // SHAPEPATH holds, wherever it stands.
    let text = "MAP SHAPEPATH 'x' LAYER NAME 'l' DATA 'poly' END SHAPEPATH 'data/' END";
    let loaded = load_files(&[("maps/m.map", text)]);
    assert_eq!(codes(&loaded), Vec::<&str>::new());
    assert_eq!(first_data(&loaded).as_deref(), Some("maps/data/poly"));
    // Without SHAPEPATH, beside the mapfile; a `..` that stays inside the base context is kept.
    let loaded = load_files(&[(
        "renderers/m.map",
        "MAP SHAPEPATH '../misc/data' LAYER NAME 'l' DATA 'poly.shp' END END",
    )]);
    assert_eq!(first_data(&loaded).as_deref(), Some("misc/data/poly.shp"));
    let loaded = load_files(&[("m.map", "MAP LAYER NAME 'l' DATA 'poly' END END")]);
    assert_eq!(first_data(&loaded).as_deref(), Some("poly"));
}

#[test]
fn a_data_path_that_leaves_the_base_context_is_refused_and_the_layer_keeps_none() {
    // Verifies: Q-8
    for text in [
        "MAP LAYER NAME 'l' DATA '../poly' END END",
        "MAP SHAPEPATH '..' LAYER NAME 'l' DATA 'poly' END END",
        "MAP LAYER NAME 'l' DATA '/etc/poly' END END",
        "MAP SHAPEPATH '/data' LAYER NAME 'l' DATA 'poly' END END",
        "MAP SHAPEPATH 'x' LAYER NAME 'l' DATA '/etc/poly' END END",
        "MAP SHAPEPATH 'x' LAYER NAME 'l' DATA 'C:/etc/poly' END END",
    ] {
        let leaked: &'static str = text;
        let loaded = load_files(&[("m.map", leaked)]);
        assert_eq!(codes(&loaded), ["data-refused"], "{text}");
        let refusal = &loaded.diagnostics[0];
        assert!(
            refusal.message.contains("is refused"),
            "{}",
            refusal.message
        );
        assert_eq!(first_data(&loaded), None, "{text}");
    }
}

#[test]
fn a_data_path_refused_beneath_shapepath_names_it() {
    // The SHAPEPATH may be the part that leaves the base context, as here.
    let loaded = load_files(&[(
        "m.map",
        "MAP SHAPEPATH '..' LAYER NAME 'l' DATA 'poly' END END",
    )]);
    assert_eq!(codes(&loaded), ["data-refused"]);
    let message = &loaded.diagnostics[0].message;
    assert!(
        message.starts_with("DATA `poly` beneath SHAPEPATH `..` is refused: "),
        "{message}"
    );
    // A DATA that ignores SHAPEPATH is refused without it.
    let loaded = load_files(&[(
        "m.map",
        "MAP SHAPEPATH 'x' LAYER NAME 'l' DATA '/etc/poly' END END",
    )]);
    let message = &loaded.diagnostics[0].message;
    assert!(
        message.starts_with("DATA `/etc/poly` is refused: "),
        "{message}"
    );
}

#[test]
fn the_data_of_a_connection_is_not_a_path() {
    let text =
        "MAP SHAPEPATH 'x' LAYER NAME 'l' CONNECTIONTYPE POSTGIS DATA 'the_geom from t' END END";
    let loaded = load_files(&[("m.map", text)]);
    assert_eq!(first_data(&loaded).as_deref(), Some("the_geom from t"));
}
