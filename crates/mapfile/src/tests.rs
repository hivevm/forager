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
        "MAP\n  WEB TEMPLATE 'x.html' END\n  LAYER NAME 'a' LABELITEM 'n' END\nEND\n",
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
    // Verifies: ADR-0018
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
    // Verifies: ADR-0018
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
        forager_style_ir::StyleDocument::from_yaml(&yaml).unwrap(),
        document
    );
    assert_eq!(
        forager_style_ir::StyleDocument::from_json(&json).unwrap(),
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
    assert_eq!(codes(&loaded), ["unsupported-value"]);
    assert_eq!(style.unwrap().angle, None);
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
        forager_style_ir::StyleDocument::from_yaml(&yaml).unwrap(),
        document
    );
    assert_eq!(
        forager_style_ir::StyleDocument::from_json(&json).unwrap(),
        document
    );
}
