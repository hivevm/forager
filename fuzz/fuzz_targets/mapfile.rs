//! The mapfile frontend on arbitrary bytes, with arbitrary INCLUDEs resolving to the same bytes:
//! diagnostics, never a panic, a hang, or unbounded memory (Q-3, Q-9). Every document it loads
//! reads back equal from YAML and from JSON (G-3).
// Verifies: Q-3
#![no_main]

use std::future::Future;
use std::task::{Context, Poll, Waker};

use forager_ports::{ProjectionResolver, ResourceLoader, ResourcePath, SourceError};
use forager_style_ir::StyleDocument;
use libfuzzer_sys::fuzz_target;

/// Every path names the same input, so INCLUDEs are followed as deep as the limits allow.
struct Echo<'a>(&'a [u8]);

impl ResourceLoader for Echo<'_> {
    async fn load(&self, _: &ResourcePath) -> Result<Vec<u8>, SourceError> {
        Ok(self.0.to_vec())
    }
}

/// Resolves every `init=` reference, so that inputs get past the projection.
struct AnyProjection;

impl ProjectionResolver for AnyProjection {
    fn resolve(&self, _: &str, _: &str) -> Option<String> {
        Some("+proj=longlat +datum=WGS84 +no_defs".to_owned())
    }
}

fuzz_target!(|data: &[u8]| {
    let path = ResourcePath::new("fuzz.map").unwrap();
    let loader = Echo(data);
    let mut future = std::pin::pin!(forager_mapfile::load(&path, &loader, Some(&AnyProjection)));
    let Poll::Ready(loaded) = future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    else {
        panic!("the mapfile frontend waited on a loader that never waits");
    };
    if let Some(document) = loaded.document {
        let yaml = document
            .to_yaml()
            .expect("a loaded document writes as YAML");
        let json = document
            .to_json()
            .expect("a loaded document writes as JSON");
        assert_eq!(
            StyleDocument::from_yaml(&yaml)
                .map(|read| read.document)
                .as_ref(),
            Ok(&document),
            "{yaml}"
        );
        assert_eq!(
            StyleDocument::from_json(&json)
                .map(|read| read.document)
                .as_ref(),
            Ok(&document),
            "{json}"
        );
    }
});
