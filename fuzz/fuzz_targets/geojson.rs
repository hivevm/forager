//! The browser path on arbitrary GeoJSON text — the bindings, the adapter, and the engine — for
//! a map with a polygon, a line, and a point layer (Q-3). The renderer itself is fuzzed with
//! structured input by the `render` target.
// Verifies: Q-3
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
    let text = serde_json::to_string(geojson).expect("a string serializes");
    let sources = format!(r#"{{"a": {text}, "b": {text}, "c": {text}}}"#);
    let _ = render_map(MAP, &sources, &Viewport::default());
});
