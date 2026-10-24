//! The mapfile frontend on arbitrary bytes, with arbitrary INCLUDEs resolving to the same bytes:
//! diagnostics, never a panic, a hang, or unbounded memory (Q-3, Q-9).
// Verifies: Q-3
#![no_main]

use std::future::Future;
use std::task::{Context, Poll, Waker};

use forager_ports::{ResourceLoader, ResourcePath, SourceError};
use libfuzzer_sys::fuzz_target;

/// Every path names the same input, so INCLUDEs are followed as deep as the limits allow.
struct Echo<'a>(&'a [u8]);

impl ResourceLoader for Echo<'_> {
    async fn load(&self, _: &ResourcePath) -> Result<Vec<u8>, SourceError> {
        Ok(self.0.to_vec())
    }
}

fuzz_target!(|data: &[u8]| {
    let path = ResourcePath::new("fuzz.map").unwrap();
    let loader = Echo(data);
    let mut future = std::pin::pin!(forager_mapfile::load(&path, &loader));
    let Poll::Ready(_) = future.as_mut().poll(&mut Context::from_waker(Waker::noop())) else {
        panic!("the mapfile frontend waited on a loader that never waits");
    };
});
