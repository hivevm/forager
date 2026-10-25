//! The binding of a host's feature sources to a map's layers by name, written once for every host
//! so that each reports the same diagnostics for the same inputs.

use std::collections::BTreeMap;

use forager_style_ir::{Diagnostic, Map};

/// Binds the sources a host names by layer to the layers of `map`, in the order of their names,
/// whatever order the host gives them in. A name no layer carries is reported as
/// `source-unbound` and its source is never read; a source `read` refuses is reported as
/// `source-unreadable` and left unbound, so the engine skips its layer (G-8). A name given twice
/// keeps its last source.
///
/// `read` turns what the host holds for a layer, such as a file path or a text, into a feature
/// source; its error names what could not be read.
pub fn bind_sources<T, S>(
    map: &Map,
    sources: impl IntoIterator<Item = (String, T)>,
    mut read: impl FnMut(T) -> Result<S, String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> BTreeMap<String, S> {
    let named: BTreeMap<String, T> = sources.into_iter().collect();
    let mut bound = BTreeMap::new();
    for (layer, held) in named {
        if !map
            .layers
            .iter()
            .any(|l| l.name.as_deref() == Some(layer.as_str()))
        {
            let message = format!("no layer is named `{layer}`; its source is unused");
            diagnostics.push(Diagnostic::warning("source-unbound", message, None));
            continue;
        }
        match read(held) {
            Ok(source) => {
                bound.insert(layer, source);
            }
            Err(e) => {
                let message = format!("source of layer `{layer}`: {e}");
                diagnostics.push(Diagnostic::warning("source-unreadable", message, None));
            }
        }
    }
    bound
}

#[cfg(test)]
mod tests {
    use forager_style_ir::Layer;

    use super::*;

    fn map(names: &[&str]) -> Map {
        Map {
            layers: names
                .iter()
                .map(|name| Layer {
                    name: Some((*name).to_owned()),
                    ..Layer::default()
                })
                .collect(),
            ..Map::default()
        }
    }

    #[test]
    fn sources_bind_by_name_in_the_order_of_their_names() {
        // Verifies: G-8
        let mut diagnostics = Vec::new();
        let mut read_ones = Vec::new();
        let bound = bind_sources(
            &map(&["areas", "roads"]),
            [("roads", "{"), ("area", "x"), ("areas", "ok")].map(|(n, t)| (n.to_owned(), t)),
            |text| {
                read_ones.push(text);
                if text == "ok" {
                    Ok(1)
                } else {
                    Err("broken".to_owned())
                }
            },
            &mut diagnostics,
        );
        assert_eq!(
            bound.into_iter().collect::<Vec<_>>(),
            [("areas".to_owned(), 1)]
        );
        assert_eq!(read_ones, ["ok", "{"], "an unbound source is never read");
        let codes: Vec<_> = diagnostics.iter().map(|d| d.code.as_str()).collect();
        assert_eq!(codes, ["source-unbound", "source-unreadable"]);
        assert!(diagnostics[0].message.contains("`area`"));
        assert!(diagnostics[1].message.contains("`roads`: broken"));
    }
}
