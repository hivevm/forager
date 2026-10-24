use std::collections::BTreeMap;
use std::future::Future;
use std::task::{Context, Poll, Waker};

use forager_ports::{ResourceLoader, ResourcePath, SourceError};
use forager_style_ir::{Bound, LayerStatus, RenderMode, Rgba, Severity, Span};

use super::{Loaded, load};

/// Files in memory, keyed by their path inside the base context.
struct Files(BTreeMap<&'static str, &'static str>);

impl ResourceLoader for Files {
    async fn load(&self, path: &ResourcePath) -> Result<Vec<u8>, SourceError> {
        self.0
            .get(path.as_str())
            .map(|text| text.as_bytes().to_vec())
            .ok_or_else(|| SourceError::new("no such file"))
    }
}

fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
    }
}

fn load_files(files: &[(&'static str, &'static str)]) -> Loaded {
    let loader = Files(files.iter().copied().collect());
    block_on(load(&ResourcePath::new(files[0].0).unwrap(), &loader, None))
}

fn codes(loaded: &Loaded) -> Vec<&str> {
    loaded.diagnostics.iter().map(|d| d.code.as_str()).collect()
}

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
    block_on(load(
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
                r#"{{"op":"<","args":[{{"op":"*","args":[{d},-1.0]}},0.0]}},"#,
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

fn first_style(text: &'static str) -> (Loaded, Option<forager_style_ir::Style>) {
    let loaded = load_files(&[("m.map", text)]);
    let style = loaded
        .document
        .as_ref()
        .map(|d| d.map.layers[0].classes[0].styles[0].clone());
    (loaded, style)
}

fn binding<T>(name: &str) -> Option<Bound<T>> {
    Some(Bound::Binding {
        binding: name.to_owned(),
    })
}

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

const SYMBOLS: &str = "SYMBOLSET
  SYMBOL NAME 'dot' TYPE ELLIPSE FILLED TRUE POINTS 1 1 END END
  SYMBOL NAME 'tick' TYPE VECTOR POINTS -1 0 1 0 -99 -99 0 -2 0 2 END END
  SYMBOL NAME 'flag' TYPE PIXMAP IMAGE 'img/flag.png' TRANSPARENT 3 END
END";

#[test]
fn symbols_come_from_the_map_then_its_symbol_set_in_mapservers_order() {
    use forager_style_ir::{SymbolKind, SymbolRef};
    let loaded = load_files(&[
        (
            "m.map",
            "MAP SYMBOLSET 'sym/set.sym'
               SYMBOL NAME 'box' TYPE VECTOR FILLED TRUE POINTS 0 0 2 0 2 4 0 4 END END
               LAYER CLASS STYLE SYMBOL 'TICK' END STYLE SYMBOL 'box' END
                 STYLE SYMBOL 4 END END END
             END",
        ),
        ("sym/set.sym", SYMBOLS),
    ]);
    assert_eq!(codes(&loaded), Vec::<&str>::new());
    let map = loaded.document.unwrap().map;
    let names: Vec<_> = map
        .symbols
        .iter()
        .map(|s| s.name.as_deref().unwrap())
        .collect();
    assert_eq!(names, ["box", "dot", "tick", "flag"]);
    let refs: Vec<_> = map.layers[0].classes[0]
        .styles
        .iter()
        .map(|s| s.symbol.clone().unwrap())
        .collect();
    let index = |i| Bound::Value(SymbolRef::Index(i));
    assert_eq!(refs, [index(3), index(1), index(4)]);
    let flag = &map.symbols[3];
    assert_eq!(flag.kind, SymbolKind::Pixmap);
    assert_eq!(
        flag.image.as_deref(),
        Some("sym/img/flag.png"),
        "beside the symbol file"
    );
    assert_eq!(flag.transparent, Some(3));
}

#[test]
fn a_vector_symbol_is_shifted_and_measured_as_loadsymbol_does() {
    let loaded = load_files(&[("m.map", "MAP SYMBOLSET 's.sym' END"), ("s.sym", SYMBOLS)]);
    let map = loaded.document.unwrap().map;
    let tick = &map.symbols[1];
    // -1..1 by -2..2 shifted by (-1, -2): two lines, 2 wide and 4 high.
    assert_eq!(
        tick.paths,
        [vec![[0.0, 2.0], [2.0, 2.0]], vec![[1.0, 0.0], [1.0, 4.0]]]
    );
    assert_eq!(tick.extent, Some([2.0, 4.0]));
    let dot = &map.symbols[0];
    assert_eq!((dot.extent, dot.filled), (Some([1.0, 1.0]), true));

    let loaded = load_files(&[(
        "m.map",
        "MAP SYMBOL TYPE VECTOR ANCHORPOINT 0 1 POINTS 1 1 3 5 END END END",
    )]);
    let symbol = &loaded.document.unwrap().map.symbols[0];
    assert_eq!(
        symbol.paths,
        [vec![[0.0, 0.0], [2.0, 4.0]]],
        "shifted for the anchor"
    );
    assert_eq!(symbol.extent, Some([2.0, 4.0]));
}

#[test]
fn a_symbol_set_or_image_outside_the_base_context_is_refused() {
    // Verifies: Q-8
    let loaded = load_files(&[("m.map", "MAP SYMBOLSET '../x.sym' END")]);
    assert_eq!(codes(&loaded), ["symbolset-refused"]);
    let loaded = load_files(&[(
        "m.map",
        "MAP SYMBOL TYPE PIXMAP IMAGE '/etc/passwd' END END",
    )]);
    assert_eq!(codes(&loaded), ["image-refused"]);
    let loaded = load_files(&[("m.map", "MAP SYMBOLSET 's.sym' END"), ("s.sym", "MAP END")]);
    assert_eq!(codes(&loaded), ["expected-symbolset"]);
}

#[test]
fn an_unknown_symbol_is_a_warning() {
    let (loaded, _) = first_style("MAP LAYER CLASS STYLE SYMBOL 'nowhere' END END END END");
    assert_eq!(codes(&loaded), ["symbol-unknown"]);
    let (loaded, _) = first_style("MAP LAYER CLASS STYLE SYMBOL 3 END END END END");
    assert_eq!(codes(&loaded), ["symbol-unknown"]);
    for text in [
        "MAP SYMBOL TYPE VECTOR END END",
        "MAP SYMBOL TYPE SVG END END",
        "MAP SYMBOL TYPE VECTOR POINTS 1 END END END",
    ] {
        let loaded = load_files(&[("m.map", text)]);
        assert_eq!(codes(&loaded), ["invalid-value"], "{text}");
    }
}

#[test]
fn symbols_read_back_equal_from_yaml_and_json() {
    let loaded = load_files(&[("m.map", "MAP SYMBOLSET 's.sym' END"), ("s.sym", SYMBOLS)]);
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

fn first_label(
    files: &[(&'static str, &'static str)],
) -> (Loaded, Option<forager_style_ir::Label>) {
    let loaded = load_files(files);
    let label = loaded
        .document
        .as_ref()
        .map(|d| d.map.layers[0].classes[0].labels[0].clone());
    (loaded, label)
}

fn json(e: &Option<forager_style_ir::Expr>) -> String {
    serde_json::to_string(e).unwrap()
}

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
fn fonts_come_from_the_font_set_and_a_label_names_one() {
    let files = [
        (
            "m.map",
            "MAP FONTSET 'f/fonts.txt' LAYER CLASS LABEL FONT 'sans,serif' END END END END",
        ),
        (
            "f/fonts.txt",
            "# fonts\nsans  DejaVuSans.ttf\n indented skipped.ttf\nmono\tttf/mono.ttf  \n",
        ),
    ];
    let (loaded, label) = first_label(&files);
    assert_eq!(codes(&loaded), ["font-unknown"], "serif is not in the set");
    assert_eq!(
        label.unwrap().font,
        Some(Bound::Value("sans,serif".to_owned()))
    );
    let fonts = loaded.document.unwrap().map.fonts;
    assert_eq!(fonts["sans"], "f/DejaVuSans.ttf", "beside the font set");
    assert_eq!(fonts["mono"], "f/ttf/mono.ttf");
    assert_eq!(fonts.len(), 2);
}

#[test]
fn a_font_set_or_font_outside_the_base_context_is_refused() {
    // Verifies: Q-8
    let loaded = load_files(&[("m.map", "MAP FONTSET '../fonts.txt' END")]);
    assert_eq!(codes(&loaded), ["fontset-refused"]);
    let loaded = load_files(&[
        ("m.map", "MAP FONTSET 'f.txt' END"),
        ("f.txt", "sans /usr/x.ttf\n"),
    ]);
    assert_eq!(codes(&loaded), ["font-refused"]);
    assert!(loaded.document.unwrap().map.fonts.is_empty());
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
    let loaded = block_on(load(
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
            "MAP LAYER PROJECTION 'init=./a:b/epsg2:1' END END END",
        ),
        ("a:b/epsg2", file),
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
    let lines = "  SHAPEPATH \"x\"\n".repeat(super::MAX_DIAGNOSTICS + 500);
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
    let lines = "  SHAPEPATH \"x\"\n".repeat(super::MAX_DIAGNOSTICS + 500);
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
