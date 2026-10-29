//! The shapefile adapter on arbitrary files: a `.shp`, a `.shx`, and a `.dbf` opened through an
//! in-memory loader and queried with an arbitrary extent — an error, never a panic or a hang
//! (Q-3, ADR-0024). The target checks no memory bound itself; the memory limit of
//! `scripts/fuzz.sh` fails a run that takes too much.
//!
//! The input is split as follows, every number little-endian and a part the input cuts short
//! taken as far as it goes: the extent as four `f64` (min x, min y, max x, max y), the lengths of
//! the `.shp` and the `.shx` as two `u32`, the `.shp`, the `.shx`, and the rest as the `.dbf`.
// Verifies: Q-3
#![no_main]

use forager_adapter_shapefile::ShapefileSource;
use forager_ports::{FeatureQuery, FeatureSource, ResourceLoader, ResourcePath, SourceError};
use forager_style_ir::Extent;
use libfuzzer_sys::fuzz_target;

/// The three files, each under the extension it is loaded by.
struct Files<'a> {
    shp: &'a [u8],
    shx: &'a [u8],
    dbf: &'a [u8],
}

impl ResourceLoader for Files<'_> {
    async fn load(&self, path: &ResourcePath) -> Result<Vec<u8>, SourceError> {
        match path
            .as_str()
            .rsplit_once('.')
            .map(|(_, extension)| extension)
        {
            Some("shp") => Ok(self.shp.to_vec()),
            Some("shx") => Ok(self.shx.to_vec()),
            Some("dbf") => Ok(self.dbf.to_vec()),
            _ => Err(SourceError::new(format!("{path} not found"))),
        }
    }
}

/// The first `n` bytes of `data`, or all of them, and the rest.
fn take(data: &[u8], n: usize) -> (&[u8], &[u8]) {
    data.split_at(n.min(data.len()))
}

/// The next eight bytes as an `f64`, zero where the input ends.
fn number(data: &mut &[u8]) -> f64 {
    let (bytes, rest) = take(data, 8);
    *data = rest;
    let mut word = [0; 8];
    word[..bytes.len()].copy_from_slice(bytes);
    f64::from_le_bytes(word)
}

/// The next four bytes as a length, zero where the input ends.
fn length(data: &mut &[u8]) -> usize {
    let (bytes, rest) = take(data, 4);
    *data = rest;
    let mut word = [0; 4];
    word[..bytes.len()].copy_from_slice(bytes);
    u32::from_le_bytes(word) as usize
}

fuzz_target!(|data: &[u8]| {
    let mut rest = data;
    let extent = Extent {
        min_x: number(&mut rest),
        min_y: number(&mut rest),
        max_x: number(&mut rest),
        max_y: number(&mut rest),
    };
    let shp_length = length(&mut rest);
    let shx_length = length(&mut rest);
    let (shp, rest) = take(rest, shp_length);
    let (shx, dbf) = take(rest, shx_length);
    let files = Files { shp, shx, dbf };
    let path = ResourcePath::new("data/layer.shp").unwrap();
    let opened = forager_ports::poll_once(ShapefileSource::open(&files, &path))
        .expect("the shapefile adapter waited on a loader that never waits");
    if let Ok(source) = opened {
        let features = forager_ports::poll_once(source.features(&FeatureQuery { extent }))
            .expect("the shapefile source waited although it never waits");
        // Every property of every feature is read, by its name as well, so that the reading of
        // the .dbf's records is fuzzed with the rest (ADR-0026).
        for feature in features.iter().flat_map(|f| &f.features) {
            feature.properties.each(|name, _| {
                let _ = feature.properties.get(name);
            });
        }
    }
});
