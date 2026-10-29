use std::collections::BTreeMap;

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
        renderable("a.png", "[MAP2IMG] -m [MAPFILE] -l lines -o [RESULT]"),
        Err("the options -l lines".to_owned())
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
        "[MAP2IMG] -m [MAPFILE] -e 0 0 1 1 -o [RESULT]",
    )];
    let entries = list("m/a.map a.png geometry passes\nm/gone.map gone.png out: x\n").unwrap();
    let problems = unlisted(&cases, &entries);
    assert_eq!(
        problems,
        [
            "m/a.map a.png is in scope but cannot be rendered: the options -e 0 0 1 1",
            "m/gone.map gone.png is in the list but is no case of the suite",
        ]
    );
    assert_eq!(unlisted(&[], &BTreeMap::new()), Vec::<String>::new());
}
