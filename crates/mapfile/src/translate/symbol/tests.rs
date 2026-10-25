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
