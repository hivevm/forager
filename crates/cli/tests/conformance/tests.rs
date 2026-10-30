use std::collections::BTreeMap;

use forager_style_ir::{Diagnostic, Extent, Layer, LayerStatus, Map};

use super::{Case, Category, Entry, Verdict, directives, list, unlisted, verdict};

fn case(mapfile: &str, result: &str, command: &str) -> Case {
    Case {
        mapfile: mapfile.to_owned(),
        result: result.to_owned(),
        command: command.to_owned(),
    }
}

#[test]
fn each_run_parms_line_is_a_case_of_its_result_and_command() {
    let text = "# RUN_PARMS: a.png [MAP2IMG] -m [MAPFILE] -i png -o [RESULT]\n\
                #RUN_PARMS: a.pdf [MAP2IMG] -m [MAPFILE] -i pdf -o [RESULT]  \n\
                MAP END\n";
    assert_eq!(
        directives("renderers/a.map", text),
        [
            case(
                "renderers/a.map",
                "a.png",
                "[MAP2IMG] -m [MAPFILE] -i png -o [RESULT]"
            ),
            case(
                "renderers/a.map",
                "a.pdf",
                "[MAP2IMG] -m [MAPFILE] -i pdf -o [RESULT]"
            ),
        ]
    );
}

#[test]
fn a_result_is_cut_to_its_file_name_as_the_test_library_cuts_it() {
    assert_eq!(
        directives(
            "misc/d.map",
            "# RUN_PARMS: sub/d.png [MAP2IMG] -m [MAPFILE] -o [RESULT]\n"
        )[0]
        .result,
        "d.png"
    );
}

#[test]
fn a_mapfile_without_a_line_is_the_plain_case_of_its_name_unless_it_skips_itself() {
    assert_eq!(
        directives("misc/autostyle.map", "MAP END\n"),
        [case(
            "misc/autostyle.map",
            "autostyle.png",
            "[MAP2IMG] [RENDERER] -m [MAPFILE] -o [RESULT]"
        )]
    );
    assert_eq!(
        directives("misc/b.map", "# SKIP_WHOLE_FILE: needs a server\nMAP END\n"),
        []
    );
    assert_eq!(
        directives("misc/c.map", "# RUN_PARMS: c.png\n"),
        [case(
            "misc/c.map",
            "c.png",
            "[MAP2IMG] [RENDERER] -m [MAPFILE] -o [RESULT]"
        )]
    );
}

#[test]
fn only_a_plain_map2img_line_of_a_png_is_renderable() {
    let renderable = |result: &str, command: &str| case("m.map", result, command).renderable();
    assert_eq!(
        renderable("a.png", "[MAP2IMG] -m [MAPFILE] -i png -o [RESULT]"),
        Ok(())
    );
    assert_eq!(
        renderable("a.png", "[MAP2IMG] [RENDERER] -m [MAPFILE] -o [RESULT]"),
        Ok(())
    );
    assert_eq!(
        renderable(
            "a.png",
            "[MAP2IMG] -e 0 0 1 1 -s 10 20 -m [MAPFILE] -l lines -o [RESULT]"
        ),
        Ok(())
    );
    assert_eq!(
        renderable("a.png", "[MAP2IMG] -m [MAPFILE] -c 2 -o [RESULT]"),
        Err("the options -c 2".to_owned())
    );
    assert_eq!(
        renderable("a.png", "[MAP2IMG] -m [MAPFILE] -i cairopng -o [RESULT]"),
        Err("the output format cairopng".to_owned())
    );
    assert_eq!(
        renderable("a.png", "[MAP2IMG] -m other.map -o [RESULT]"),
        Err("the options -m other.map".to_owned()),
        "an option that does not name the placeholder keeps its argument"
    );
    assert!(renderable("a.svg", "[MAP2IMG] -m [MAPFILE] -i svg -o [RESULT]").is_err());
    assert!(renderable("a.png", "[MAPSERV] QUERY_STRING=\"map=[MAPFILE]\"").is_err());
}

#[test]
fn a_case_that_cannot_be_rendered_is_broken_whatever_outcome_the_list_expects() {
    let case = case("m/a.map", "a.png", "[MAP2IMG] -m [MAPFILE] -o [RESULT]");
    let missing = || Err("its reference m/expected/a.png: no such file".to_owned());
    for passes in [true, false] {
        assert_eq!(
            verdict(&case, passes, missing()),
            Some(Verdict::Broken(
                "m/a.map a.png: its reference m/expected/a.png: no such file".to_owned()
            ))
        );
    }
}

#[test]
fn a_rendered_case_differs_only_when_its_outcome_is_not_the_listed_one() {
    let case = case("m/a.map", "a.png", "[MAP2IMG] -m [MAPFILE] -o [RESULT]");
    let failed = || Ok((false, "9 of 100 pixels differ (0.0900)".to_owned()));
    assert_eq!(verdict(&case, false, failed()), None);
    assert_eq!(
        verdict(&case, true, failed()),
        Some(Verdict::Differs(
            "m/a.map a.png: 9 of 100 pixels differ (0.0900); the list expects it to pass"
                .to_owned()
        ))
    );
    let passed = Ok((true, "0 of 100 pixels differ (0.0000)".to_owned()));
    assert!(matches!(
        verdict(&case, false, passed),
        Some(Verdict::Differs(message)) if message.ends_with("the list expects it to fail")
    ));
}

#[test]
fn the_list_reads_cases_in_scope_and_out_of_scope() {
    let entries = list(
        "# provisional\n\
         renderers/a.map a.png geometry passes\n\
         renderers/a.map a.pdf out: another output format, pdf # a comment\n\
         \n\
         renderers/b.map b.png geometry fails\n",
    )
    .unwrap();
    let get = |m: &str, r: &str| {
        let listed = entries.get(&(m.to_owned(), r.to_owned())).cloned();
        assert!(
            !listed.as_ref().is_some_and(|l| l.blank),
            "{m} {r} is not marked blank"
        );
        listed.map(|l| l.entry)
    };
    assert_eq!(
        get("renderers/a.map", "a.png"),
        Some(Entry::InScope {
            category: Category::Geometry,
            passes: true
        })
    );
    assert_eq!(
        get("renderers/a.map", "a.pdf"),
        Some(Entry::Out("another output format, pdf".to_owned()))
    );
    assert_eq!(
        get("renderers/b.map", "b.png"),
        Some(Entry::InScope {
            category: Category::Geometry,
            passes: false
        })
    );
}

#[test]
fn the_list_marks_a_case_in_scope_or_out_of_it_blank() {
    let entries = list(
        "renderers/empty.map empty.png blank geometry passes
         misc/contour.map contour.png blank out: a raster source, CONTOUR
         misc/blankets.map blankets.png geometry fails
",
    )
    .unwrap();
    let get = |m: &str, r: &str| entries[&(m.to_owned(), r.to_owned())].clone();
    let empty = get("renderers/empty.map", "empty.png");
    assert!(empty.blank);
    assert_eq!(
        empty.entry,
        Entry::InScope {
            category: Category::Geometry,
            passes: true
        }
    );
    let contour = get("misc/contour.map", "contour.png");
    assert!(contour.blank);
    assert_eq!(
        contour.entry,
        Entry::Out("a raster source, CONTOUR".to_owned())
    );
    assert!(!get("misc/blankets.map", "blankets.png").blank);
    assert!(
        list("m.map m.png blank\n").is_err(),
        "a marker is no outcome"
    );
}

#[test]
fn a_malformed_list_is_an_error_naming_its_line() {
    for (text, line) in [
        ("a.map a.png\n", "line 1"),
        ("a.map a.png out:\n", "line 1"),
        ("\na.map a.png text passes\n", "line 2"),
        ("a.map a.png geometry maybe\n", "line 1"),
        ("a.map a.png geometry passes blank\n", "line 1"),
        (
            "a.map a.png out: x\na.map a.png geometry passes\n",
            "line 2",
        ),
    ] {
        let error = list(text).unwrap_err();
        assert!(error.starts_with(line), "{text:?}: {error}");
    }
}

#[test]
fn a_case_the_list_does_not_name_is_named() {
    let cases = [
        case("m/a.map", "a.png", "[MAP2IMG] -m [MAPFILE] -o [RESULT]"),
        case("m/b.map", "b.png", "[MAP2IMG] -m [MAPFILE] -o [RESULT]"),
    ];
    let entries = list("m/a.map a.png geometry passes\n").unwrap();
    let problems = unlisted(&cases, &entries);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].starts_with("m/b.map b.png is not in the list"));
}

#[test]
fn a_listed_case_the_suite_does_not_hold_or_cannot_render_is_named() {
    let cases = [case(
        "m/a.map",
        "a.png",
        "[MAP2IMG] -m [MAPFILE] -d a b.shp -o [RESULT]",
    )];
    let entries = list("m/a.map a.png geometry passes\nm/gone.map gone.png out: x\n").unwrap();
    let problems = unlisted(&cases, &entries);
    assert_eq!(
        problems,
        [
            "m/a.map a.png is in scope but cannot be rendered: the options -d a b.shp",
            "m/gone.map gone.png is in the list but is no case of the suite",
        ]
    );
    assert_eq!(unlisted(&[], &BTreeMap::new()), Vec::<String>::new());
}

fn layer(name: &str, status: Option<LayerStatus>) -> Layer {
    Layer {
        name: Some(name.to_owned()),
        status,
        ..Layer::default()
    }
}

fn statuses(map: &Map) -> Vec<Option<LayerStatus>> {
    map.layers.iter().map(|l| l.status).collect()
}

/// The command is split into words as the shell `os.system` hands it to splits it, quotes and
/// all, so that `-l "a b"` is one argument naming two layers.
#[test]
fn a_quoted_argument_is_one_word_as_the_shell_reads_it() {
    let options = case(
        "m.map",
        "a.png",
        "[MAP2IMG] -m [MAPFILE] -i png -l \"centroid vertex1\" -o [RESULT]",
    )
    .options()
    .unwrap();
    assert_eq!(options.len(), 2);
    assert_eq!(super::image_type(&options), Some("png"));
    let mut map = Map {
        layers: vec![
            layer("vertex1", Some(LayerStatus::On)),
            layer("vertex2", Some(LayerStatus::On)),
            layer("centroid", Some(LayerStatus::On)),
        ],
        ..Map::default()
    };
    super::apply(&options, &mut map, &[]).unwrap();
    assert_eq!(
        statuses(&map),
        [
            Some(LayerStatus::On),
            Some(LayerStatus::Off),
            Some(LayerStatus::On)
        ]
    );
}

/// map2img.c at the pin: `-l` turns every layer off that is not DEFAULT, then on each whose name
/// is one of the list's, compared without regard to case; a DEFAULT layer stays drawn.
#[test]
fn the_layers_option_turns_on_the_named_layers_and_off_the_others_but_default_ones() {
    let mut map = Map {
        layers: vec![
            layer("base", Some(LayerStatus::Default)),
            layer("Outlines1", Some(LayerStatus::Off)),
            layer("outlines3", Some(LayerStatus::On)),
            layer("unset", None),
            Layer {
                status: Some(LayerStatus::On),
                ..Layer::default()
            },
        ],
        ..Map::default()
    };
    let options = case(
        "m.map",
        "a.png",
        "[MAP2IMG] -m [MAPFILE] -o [RESULT] -l outlines1",
    )
    .options()
    .unwrap();
    super::apply(&options, &mut map, &[]).unwrap();
    assert_eq!(
        statuses(&map),
        [
            Some(LayerStatus::Default),
            Some(LayerStatus::On),
            Some(LayerStatus::Off),
            Some(LayerStatus::Off),
            Some(LayerStatus::Off),
        ],
        "a layer without a name is turned off, as map2img skips a NULL name"
    );
}

/// The style IR holds no `GROUP`, so `-l` on a mapfile whose load did not carry a group over is
/// an error rather than a drawing that leaves out the layers of the group.
#[test]
fn a_layers_option_on_a_mapfile_with_a_group_is_an_error() {
    let mut map = Map {
        layers: vec![layer("a", Some(LayerStatus::On))],
        ..Map::default()
    };
    let options = case("m.map", "a.png", "[MAP2IMG] -m [MAPFILE] -l a -o [RESULT]")
        .options()
        .unwrap();
    let group = Diagnostic::warning(
        "unsupported-keyword",
        "GROUP is not yet supported and is ignored",
        None,
    );
    let error = super::apply(&options, &mut map, &[group]).unwrap_err();
    assert!(error.contains("GROUP"), "{error}");
    let size = case(
        "m.map",
        "a.png",
        "[MAP2IMG] -m [MAPFILE] -s 10 10 -o [RESULT]",
    )
    .options()
    .unwrap();
    let group = Diagnostic::warning(
        "unsupported-keyword",
        "GROUP is not yet supported and is ignored",
        None,
    );
    super::apply(&size, &mut map, &[group]).expect("without -l a group changes nothing");
}

/// map2img exits without an image when a name of `-l` is no layer's or group's; the style IR
/// holds no group, so a name that is no layer's is an error the harness reports.
#[test]
fn a_layers_option_naming_no_layer_is_an_error() {
    let mut map = Map {
        layers: vec![layer("a", Some(LayerStatus::On))],
        ..Map::default()
    };
    let options = case(
        "m.map",
        "a.png",
        "[MAP2IMG] -m [MAPFILE] -l \"a  grp\" -o [RESULT]",
    )
    .options()
    .unwrap();
    let error = super::apply(&options, &mut map, &[]).unwrap_err();
    assert!(error.contains("`grp`"), "{error}");
}

/// The names of `-l` are split at spaces as `msStringSplit` splits them: a run of spaces is one
/// separator, a space at either end leaves an empty name.
#[test]
fn the_layer_names_are_split_as_mapserver_splits_them() {
    assert_eq!(super::split_names("a  b"), ["a", "b"]);
    assert_eq!(super::split_names("a"), ["a"]);
    assert_eq!(super::split_names(" a"), ["", "a"]);
    assert_eq!(super::split_names("a "), ["a", ""]);
}

#[test]
fn the_extent_and_the_size_options_set_those_of_the_map() {
    let mut map = Map {
        extent: Some(Extent {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 1.0,
            max_y: 1.0,
        }),
        size: Some([400, 300]),
        ..Map::default()
    };
    let options = case(
        "m.map",
        "a.png",
        "[MAP2IMG] -m [MAPFILE] -i png -e -7 -17.5 -4 -14 -s 50 60 -o [RESULT]",
    )
    .options()
    .unwrap();
    super::apply(&options, &mut map, &[]).unwrap();
    assert_eq!(
        map.extent,
        Some(Extent {
            min_x: -7.0,
            min_y: -17.5,
            max_x: -4.0,
            max_y: -14.0
        })
    );
    assert_eq!(map.size, Some([50, 60]));
}

#[test]
fn an_option_short_of_its_arguments_is_not_renderable() {
    for (command, reason) in [
        (
            "[MAP2IMG] -m [MAPFILE] -o [RESULT] -e 0 1",
            "-e takes four numbers",
        ),
        (
            "[MAP2IMG] -m [MAPFILE] -s 640 -o [RESULT]",
            "-s takes two sizes",
        ),
        (
            "[MAP2IMG] -m [MAPFILE] -o [RESULT] -l",
            "-l takes a list of layers",
        ),
    ] {
        let error = case("m.map", "a.png", command).renderable().unwrap_err();
        assert!(error.contains(reason), "{command}: {error}");
    }
}

#[test]
fn the_words_of_a_line_are_split_as_the_shell_splits_them() {
    assert_eq!(
        super::shell_words(r#"a "b c" 'd "e"' f\ g "h\"i" x>y"#).unwrap(),
        ["a", "b c", "d \"e\"", "f g", "h\"i", "x>y"]
    );
    assert_eq!(
        super::shell_words(r#"-l "a b"#).unwrap_err(),
        "the line leaves a quote open"
    );
    assert_eq!(super::shell_words("  ").unwrap(), Vec::<String>::new());
    assert_eq!(super::shell_words(r#"-l """#).unwrap(), ["-l", ""]);
}

/// MapServer is run as the test library runs it: in the mapfile's directory, without a renderer,
/// and writing where the measurement asks.
#[test]
fn map2img_is_handed_the_line_as_the_test_library_hands_it_over() {
    let case = case(
        "renderers/sub/m.map",
        "a.png",
        "[MAP2IMG] [RENDERER] -m [MAPFILE] -l \"a b\" -o [RESULT]",
    );
    assert_eq!(
        case.map2img_arguments(std::path::Path::new("/out/a.png"))
            .unwrap(),
        ["-m", "m.map", "-l", "a b", "-o", "/out/a.png"]
    );
    let other = super::Case {
        command: "[MAP2IMG] -m [MAPFILE] -c 2 -o [RESULT]".to_owned(),
        ..case
    };
    assert!(
        other
            .map2img_arguments(std::path::Path::new("/out/a.png"))
            .is_err(),
        "a case the harness does not render is not run either"
    );
}

#[test]
fn the_imagetype_of_i_goes_before_the_end_of_the_map() {
    let text = "MAP\n  IMAGETYPE png24\n  LAYER NAME 'end' END\nEND # the END of MAP\n# END\n";
    assert_eq!(
        super::with_image_type(text, "png").unwrap(),
        "MAP\n  IMAGETYPE png24\n  LAYER NAME 'end' END\nIMAGETYPE \"png\" END # the END of MAP\n# END\n"
    );
    assert_eq!(
        super::with_image_type("map end", "png").unwrap(),
        "map IMAGETYPE \"png\" end\n"
    );
    assert!(super::with_image_type("MAP # END", "png").is_err());
}
