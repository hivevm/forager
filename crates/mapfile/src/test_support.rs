//! The helpers the mapfile crate's tests share: files in memory, a load of them, and the
//! parts of a loaded map a test looks at.

use std::collections::BTreeMap;
use std::future::Future;

use forager_ports::{ResourceLoader, ResourcePath, SourceError};
use forager_style_ir::Bound;

use crate::{Loaded, load};

/// Files in memory, keyed by their path inside the base context.
pub(crate) struct Files(pub(crate) BTreeMap<&'static str, &'static str>);

impl ResourceLoader for Files {
    async fn load(&self, path: &ResourcePath) -> Result<Vec<u8>, SourceError> {
        self.0
            .get(path.as_str())
            .map(|text| text.as_bytes().to_vec())
            .ok_or_else(|| SourceError::new("no such file"))
    }
}

pub(crate) fn ready<F: Future>(future: F) -> F::Output {
    forager_ports::poll_once(future).expect("the test files never wait")
}

pub(crate) fn load_files(files: &[(&'static str, &'static str)]) -> Loaded {
    let loader = Files(files.iter().copied().collect());
    ready(load(&ResourcePath::new(files[0].0).unwrap(), &loader, None))
}

pub(crate) fn codes(loaded: &Loaded) -> Vec<&str> {
    loaded.diagnostics.iter().map(|d| d.code.as_str()).collect()
}

pub(crate) fn first_style(text: &'static str) -> (Loaded, Option<forager_style_ir::Style>) {
    let loaded = load_files(&[("m.map", text)]);
    let style = loaded
        .document
        .as_ref()
        .map(|d| d.map.layers[0].classes[0].styles[0].clone());
    (loaded, style)
}

pub(crate) fn binding<T>(name: &str) -> Option<Bound<T>> {
    Some(Bound::Binding {
        binding: name.to_owned(),
    })
}

pub(crate) fn first_label(
    files: &[(&'static str, &'static str)],
) -> (Loaded, Option<forager_style_ir::Label>) {
    let loaded = load_files(files);
    let label = loaded
        .document
        .as_ref()
        .map(|d| d.map.layers[0].classes[0].labels[0].clone());
    (loaded, label)
}

pub(crate) fn json(e: &Option<forager_style_ir::Expr>) -> String {
    serde_json::to_string(e).unwrap()
}
