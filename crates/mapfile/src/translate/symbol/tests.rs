use forager_style_ir::Bound;

use crate::test_support::{codes, first_style, load_files};

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

/// The one symbol of a map that defines it inline, and the load's diagnostic codes.
fn inline_symbol(text: &'static str) -> (Vec<String>, Option<forager_style_ir::Symbol>) {
    let loaded = load_files(&[("m.map", text)]);
    let codes = codes(&loaded).into_iter().map(str::to_owned).collect();
    let symbol = loaded.document.map(|d| d.map.symbols[0].clone());
    (codes, symbol)
}

#[test]
fn type_takes_the_seven_symbol_types_bare_and_defaults_to_vector() {
    use forager_style_ir::SymbolKind;
    // loadSymbol, TYPE: getSymbol of the lexer's symbol type tokens.
    for (word, kind) in [
        ("vector", SymbolKind::Vector),
        ("ELLIPSE", SymbolKind::Ellipse),
        ("simple", SymbolKind::Simple),
        ("TrueType", SymbolKind::Truetype),
        ("hatch", SymbolKind::Hatch),
    ] {
        let text = format!("MAP SYMBOL TYPE {word} POINTS 1 1 END END END");
        let (codes, symbol) = inline_symbol(text.leak());
        assert_eq!(codes, Vec::<String>::new(), "{word}");
        assert_eq!(symbol.unwrap().kind, kind, "{word}");
    }
    let (_, symbol) = inline_symbol("MAP SYMBOL TYPE PIXMAP IMAGE 'a.png' END END");
    assert_eq!(symbol.unwrap().kind, SymbolKind::Pixmap);
    let (_, symbol) = inline_symbol("MAP SYMBOL TYPE svg IMAGE 'a.svg' END END");
    assert_eq!(symbol.unwrap().kind, SymbolKind::Svg);
    let (_, symbol) = inline_symbol("MAP SYMBOL POINTS 1 1 END END END");
    assert_eq!(
        symbol.unwrap().kind,
        SymbolKind::Vector,
        "initSymbol's default"
    );
    // A quoted type is a string, which getSymbol does not take; nor a type MapServer lacks.
    for text in [
        "MAP SYMBOL TYPE 'vector' POINTS 1 1 END END END",
        "MAP SYMBOL TYPE cartoline POINTS 1 1 END END END",
    ] {
        assert_eq!(inline_symbol(text).0, ["invalid-value"], "{text}");
    }
}

#[test]
fn filled_takes_true_or_false_and_their_on_and_off() {
    // FILLED: getSymbol(2, MS_TRUE, MS_FALSE), which MS_ON and MS_OFF equal.
    for (word, filled) in [
        ("TRUE", true),
        ("on", true),
        ("false", false),
        ("OFF", false),
    ] {
        let text = format!("MAP SYMBOL TYPE ELLIPSE FILLED {word} POINTS 1 1 END END END");
        let (codes, symbol) = inline_symbol(text.leak());
        assert_eq!(codes, Vec::<String>::new(), "{word}");
        assert_eq!(symbol.unwrap().filled, filled, "{word}");
    }
    let (_, symbol) = inline_symbol("MAP SYMBOL TYPE ELLIPSE POINTS 1 1 END END END");
    assert!(!symbol.unwrap().filled, "initSymbol's default");
    let text = "MAP SYMBOL FILLED 1 POINTS 1 1 END END END";
    assert_eq!(inline_symbol(text).0, ["invalid-value"]);
}

#[test]
fn anchorpoint_takes_two_numbers_from_0_to_1() {
    // ANCHORPOINT: two getDouble with MS_NUM_CHECK_RANGE 0 to 1, bounds included.
    let (codes, symbol) =
        inline_symbol("MAP SYMBOL TYPE ELLIPSE ANCHORPOINT 0 1 POINTS 1 1 END END END");
    assert_eq!(codes, Vec::<String>::new());
    assert_eq!(symbol.unwrap().anchor_point, Some([0.0, 1.0]));
    for text in [
        "MAP SYMBOL TYPE ELLIPSE ANCHORPOINT 1.5 0 POINTS 1 1 END END END",
        "MAP SYMBOL TYPE ELLIPSE ANCHORPOINT 0 -0.1 POINTS 1 1 END END END",
        "MAP SYMBOL TYPE ELLIPSE ANCHORPOINT 0.5 POINTS 1 1 END END END",
        "MAP SYMBOL TYPE ELLIPSE ANCHORPOINT 'a' 0 POINTS 1 1 END END END",
    ] {
        assert_eq!(inline_symbol(text).0, ["invalid-value"], "{text}");
    }
}

#[test]
fn transparent_is_a_palette_index_truncated_as_getinteger_does() {
    // TRANSPARENT: getInteger with MS_NUM_CHECK_RANGE 0 to 255, then cast to int.
    for (value, index) in [("0", 0), ("255", 255), ("3.7", 3)] {
        let text = format!("MAP SYMBOL TYPE PIXMAP IMAGE 'a.png' TRANSPARENT {value} END END");
        let (codes, symbol) = inline_symbol(text.leak());
        assert_eq!(codes, Vec::<String>::new(), "{value}");
        assert_eq!(symbol.unwrap().transparent, Some(index), "{value}");
    }
    for value in ["256", "255.5", "-1", "'3'"] {
        let text = format!("MAP SYMBOL TYPE PIXMAP IMAGE 'a.png' TRANSPARENT {value} END END");
        let (codes, _) = inline_symbol(text.leak());
        assert_eq!(codes, ["invalid-value"], "{value}");
    }
}

#[test]
fn image_is_a_string_resolved_beside_the_mapfile_and_the_last_one_counts() {
    // IMAGE: a string token, msBuildPath against the map's path; a later IMAGE replaces it.
    let loaded = load_files(&[(
        "maps/m.map",
        "MAP SYMBOL TYPE PIXMAP IMAGE 'a.png' IMAGE img/b.png END END",
    )]);
    assert_eq!(codes(&loaded), Vec::<&str>::new());
    let symbol = &loaded.document.unwrap().map.symbols[0];
    assert_eq!(symbol.image.as_deref(), Some("maps/img/b.png"));
    let text = "MAP SYMBOL TYPE PIXMAP IMAGE 3 END END";
    assert_eq!(inline_symbol(text).0, ["invalid-value"]);
    // An SVG symbol needs its IMAGE as a pixmap does; a TrueType or hatch symbol needs neither
    // an IMAGE nor POINTS.
    assert_eq!(
        inline_symbol("MAP SYMBOL TYPE PIXMAP END END").0,
        ["invalid-value"]
    );
    for text in [
        "MAP SYMBOL TYPE TRUETYPE CHARACTER 'a' FONT 'f' END END",
        "MAP SYMBOL TYPE HATCH END END",
        "MAP SYMBOL TYPE SIMPLE END END",
    ] {
        let (codes, symbol) = inline_symbol(text);
        assert_eq!(codes, Vec::<String>::new(), "{text}");
        assert_eq!(symbol.unwrap().extent, None, "{text}");
    }
    let (_, symbol) = inline_symbol("MAP SYMBOL TYPE TRUETYPE CHARACTER '&#9733;' FONT f END END");
    let symbol = symbol.unwrap();
    assert_eq!(
        (symbol.character.as_deref(), symbol.font.as_deref()),
        (Some("&#9733;"), Some("f"))
    );
}

#[test]
fn points_are_pairs_of_numbers_at_most_100() {
    // POINTS: an x, then a y by getDouble, until END; the 101st x is "too many points".
    let hundred = format!("MAP SYMBOL POINTS {} END END END", "1 2 ".repeat(100));
    let (codes, symbol) = inline_symbol(hundred.leak());
    assert_eq!(codes, Vec::<String>::new());
    assert_eq!(symbol.unwrap().paths[0].len(), 100);
    let more = format!("MAP SYMBOL POINTS {} END END END", "1 2 ".repeat(101));
    assert_eq!(inline_symbol(more.leak()).0, ["invalid-value"]);
    let text = "MAP SYMBOL POINTS 1 '2' END END END";
    assert_eq!(inline_symbol(text).0, ["invalid-value"]);
}

#[test]
fn a_vector_symbol_is_shifted_only_where_loadsymbol_shifts_it() {
    let shape = |text: &'static str| {
        let (codes, symbol) = inline_symbol(text);
        assert_eq!(codes, Vec::<String>::new(), "{text}");
        let symbol = symbol.unwrap();
        (symbol.paths, symbol.extent.unwrap())
    };
    // Positive points with the anchor at the centre stay where they are.
    assert_eq!(
        shape("MAP SYMBOL POINTS 1 1 3 5 END END END"),
        (vec![vec![[1.0, 1.0], [3.0, 5.0]]], [3.0, 5.0])
    );
    // An explicit centre is the centre.
    assert_eq!(
        shape("MAP SYMBOL ANCHORPOINT 0.5 0.5 POINTS 1 1 3 5 END END END"),
        (vec![vec![[1.0, 1.0], [3.0, 5.0]]], [3.0, 5.0])
    );
    // One negative coordinate shifts both axes by their smallest value.
    assert_eq!(
        shape("MAP SYMBOL POINTS 2 -1 4 1 END END END"),
        (vec![vec![[0.0, 0.0], [2.0, 2.0]]], [2.0, 2.0])
    );
    // The extent starts from 0 before the shift, so all-negative points measure from 0.
    assert_eq!(
        shape("MAP SYMBOL POINTS -3 -3 -1 -1 END END END"),
        (vec![vec![[0.0, 0.0], [2.0, 2.0]]], [3.0, 3.0])
    );
    // The smallest x and y start from the first point even where it lifts the pen.
    assert_eq!(
        shape("MAP SYMBOL POINTS -99 -99 1 1 2 2 END END END"),
        (vec![vec![[100.0, 100.0], [101.0, 101.0]]], [101.0, 101.0])
    );
}

#[test]
fn a_point_with_one_coordinate_of_minus_99_is_drawn_but_never_shifted() {
    // loadSymbol shifts a point only where neither coordinate is -99; the renderers lift the pen
    // only where both are (imageVectorSymbol, renderVectorSymbolCairo).
    let (codes, symbol) = inline_symbol("MAP SYMBOL POINTS -1 0 1 0 5 -99 END END END");
    assert_eq!(codes, Vec::<String>::new());
    let symbol = symbol.unwrap();
    // Shifted by (-1, 0), the smallest x and y of the other points.
    assert_eq!(symbol.paths, [vec![[0.0, 0.0], [2.0, 0.0], [5.0, -99.0]]]);
    // The point counts for the extent, as its x is not -99: 5 - (-1) by 0 - 0.
    assert_eq!(symbol.extent, Some([6.0, 0.0]));
}

#[test]
fn a_second_points_block_adds_its_points_and_measures_the_extent_anew() {
    // POINTS resets sizex and sizey to 0 but appends to the points read before.
    let (codes, symbol) = inline_symbol("MAP SYMBOL POINTS 0 0 8 8 END POINTS 1 1 2 3 END END END");
    assert_eq!(codes, Vec::<String>::new());
    let symbol = symbol.unwrap();
    assert_eq!(
        symbol.paths,
        [vec![[0.0, 0.0], [8.0, 8.0], [1.0, 1.0], [2.0, 3.0]]]
    );
    assert_eq!(symbol.extent, Some([2.0, 3.0]));
    // The limit of 100 points counts both blocks.
    let text = format!(
        "MAP SYMBOL POINTS {} END POINTS 1 1 END END END",
        "1 2 ".repeat(100)
    );
    assert_eq!(inline_symbol(text.leak()).0, ["invalid-value"]);
}

#[test]
fn an_ellipse_is_measured_but_never_shifted() {
    // Only a MS_SYMBOL_VECTOR is shifted at loadSymbol's END; the ellipse renderers draw the
    // extent, sizex by sizey.
    let (codes, symbol) = inline_symbol("MAP SYMBOL TYPE ELLIPSE POINTS -1 2 END END END");
    assert_eq!(codes, Vec::<String>::new());
    let symbol = symbol.unwrap();
    assert_eq!(symbol.paths, [vec![[-1.0, 2.0]]]);
    assert_eq!(symbol.extent, Some([0.0, 2.0]));
    let (_, symbol) =
        inline_symbol("MAP SYMBOL TYPE ELLIPSE ANCHORPOINT 0 0 POINTS 2 3 END END END");
    assert_eq!(
        symbol.unwrap().paths,
        [vec![[2.0, 3.0]]],
        "not for the anchor either"
    );
}

#[test]
fn a_name_resolves_to_the_first_symbol_of_that_name_and_an_index_is_kept() {
    use forager_style_ir::SymbolRef;
    // msGetSymbolIndex: from index 1, the first name equal without regard to case; a map's
    // symbol comes before the symbol file's of the same name.
    let loaded = load_files(&[
        (
            "m.map",
            "MAP SYMBOLSET 's.sym'
               SYMBOL NAME 'Dot' TYPE ELLIPSE POINTS 2 2 END END
               SYMBOL NAME 'dot' TYPE ELLIPSE POINTS 3 3 END END
               LAYER CLASS
                 STYLE SYMBOL 'DOT' END
                 STYLE SYMBOL 'flag' END
                 STYLE SYMBOL 0 END
                 STYLE SYMBOL 5 END
                 STYLE SYMBOL 1.9 END
               END END
             END",
        ),
        ("s.sym", SYMBOLS),
    ]);
    assert_eq!(codes(&loaded), Vec::<&str>::new());
    let map = loaded.document.unwrap().map;
    let refs: Vec<_> = map.layers[0].classes[0]
        .styles
        .iter()
        .map(|s| s.symbol.clone().unwrap())
        .collect();
    let index = |i| Bound::Value(SymbolRef::Index(i));
    // 0 is the default symbol, 5 the last of the five; (int)1.9 is 1.
    assert_eq!(refs, [index(1), index(5), index(0), index(5), index(1)]);
}

#[test]
fn an_index_past_the_symbols_or_a_quoted_number_warns_and_a_negative_one_is_refused() {
    use forager_style_ir::SymbolRef;
    let one = "MAP SYMBOL TYPE ELLIPSE POINTS 1 1 END END LAYER CLASS STYLE SYMBOL ";
    let (loaded, style) = first_style(format!("{one} 2 END END END END").leak());
    assert_eq!(codes(&loaded), ["symbol-unknown"]);
    assert_eq!(
        style.unwrap().symbol,
        Some(Bound::Value(SymbolRef::Index(2)))
    );
    // A quoted number is a name (MS_STRING), which no symbol has.
    let (loaded, style) = first_style(format!("{one} '1' END END END END").leak());
    assert_eq!(codes(&loaded), ["symbol-unknown"]);
    assert_eq!(
        style.unwrap().symbol,
        Some(Bound::Value(SymbolRef::Name("1".to_owned())))
    );
    // "Invalid SYMBOL id, must be greater than or equal to 0" (loadStyle).
    let (loaded, _) = first_style(format!("{one} -1 END END END END").leak());
    assert_eq!(codes(&loaded), ["invalid-value"]);
}

#[test]
fn the_last_symbolset_is_the_one_read() {
    // SYMBOLSET: getString replaces the file named before.
    let loaded = load_files(&[
        ("m.map", "MAP SYMBOLSET 'a.sym' SYMBOLSET 'b.sym' END"),
        ("b.sym", "SYMBOLSET SYMBOL NAME 'b' TYPE HATCH END END"),
    ]);
    assert_eq!(codes(&loaded), Vec::<&str>::new());
    let map = loaded.document.unwrap().map;
    assert_eq!(map.symbols.len(), 1);
    assert_eq!(map.symbols[0].name.as_deref(), Some("b"));
}

#[test]
fn a_symbol_file_reads_nested_and_successive_symbolsets_in_order() {
    // loadSymbolSet counts SYMBOLSET and END levels and reads to the end of the file: a set
    // nested in a set, and a set after the first, add their symbols in the order they stand.
    let loaded = load_files(&[
        ("m.map", "MAP SYMBOLSET 's.sym' END"),
        (
            "s.sym",
            "SYMBOLSET
               SYMBOL NAME 'a' TYPE HATCH END
               SYMBOLSET SYMBOL NAME 'b' TYPE HATCH END END
               SYMBOL NAME 'c' TYPE HATCH END
             END
             SYMBOLSET SYMBOL NAME 'd' TYPE HATCH END END",
        ),
    ]);
    assert_eq!(codes(&loaded), Vec::<&str>::new());
    let map = loaded.document.unwrap().map;
    let names: Vec<_> = map
        .symbols
        .iter()
        .map(|s| s.name.as_deref().unwrap())
        .collect();
    assert_eq!(names, ["a", "b", "c", "d"]);
}
