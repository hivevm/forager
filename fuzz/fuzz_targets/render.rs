//! The whole render path on arbitrary GeoJSON — the adapter, the engine, the clipping, and
//! agg-rust — through the function the browser bindings call, for a map with a polygon, a line,
//! and a point layer (Q-3, ADR-0015).
// Verifies: Q-3, ADR-0015
#![no_main]

use forager_bindings_wasm::{Viewport, render_map};
use libfuzzer_sys::fuzz_target;

const MAP: &str = r#"MAP
  EXTENT -180 -90 180 90
  SIZE 128 64
  LAYER NAME "a" TYPE POLYGON STATUS ON
    CLASS STYLE COLOR 200 220 180 OUTLINECOLOR 0 0 0 WIDTH 2 END END
  END
  LAYER NAME "b" TYPE LINE STATUS ON CLASS STYLE COLOR 0 0 255 WIDTH 3 END END END
  LAYER NAME "c" TYPE POINT STATUS ON CLASS STYLE COLOR 255 0 0 SIZE 5 END END END
END"#;

fuzz_target!(|geojson: &str| {
    let text = serde_json_string(geojson);
    let sources = format!(r#"{{"a": {text}, "b": {text}, "c": {text}}}"#);
    let _ = render_map(MAP, &sources, &Viewport::default());
});

/// The input as a JSON string literal, so that it reaches the adapter as GeoJSON text.
fn serde_json_string(text: &str) -> String {
    let mut quoted = String::with_capacity(text.len() + 2);
    quoted.push('"');
    for c in text.chars() {
        match c {
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            c if (c as u32) < 0x20 => quoted.push_str(&format!("\\u{:04x}", c as u32)),
            c => quoted.push(c),
        }
    }
    quoted.push('"');
    quoted
}
