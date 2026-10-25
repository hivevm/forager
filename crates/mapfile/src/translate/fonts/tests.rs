use forager_style_ir::Bound;

use crate::test_support::{codes, first_label, load_files};

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
