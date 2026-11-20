//! The reader of a shapefile's `.prj` on arbitrary bytes: a declared projection, readable or not,
//! never a panic or a hang (Q-3, API-source-declares-projection).
// Verifies: Q-3
#![no_main]

use forager_adapter_shapefile::declared_projection_of_prj;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = declared_projection_of_prj("fuzz.prj", data);
});
