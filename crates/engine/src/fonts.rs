//! The fonts a render's labels name, loaded through the resource loader once per render when a
//! label first needs them (ADR-0028).

use std::collections::BTreeMap;
use std::rc::Rc;

use forager_ports::{ResourceLoader, ResourcePath};
use forager_style_ir::{Bound, Diagnostic, Label, Map};

/// The largest font file a render loads (Q-9).
pub const MAX_FONT_BYTES: usize = 32 * 1024 * 1024;

/// The fonts loaded during one render, by the name the font set gives them, each its bytes or why
/// it is not usable.
#[derive(Default)]
pub(crate) struct Fonts {
    loaded: BTreeMap<String, Result<Rc<[u8]>, String>>,
}

impl Fonts {
    /// The usable fonts of `label`'s `FONT` list, in the order `get_face_for_run` tries them: each
    /// entry with its `xx:` prefix stripped first, then as written, one the font set lacks or that
    /// does not load skipped. A label whose last entry cannot be used gets a diagnostic naming
    /// the font, its path, and the label, as it cannot be drawn. A font bound to an attribute is
    /// known only per feature and is not loaded here.
    pub(crate) async fn for_label<L: ResourceLoader>(
        &mut self,
        map: &Map,
        label: &Label,
        loader: &L,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Vec<Rc<[u8]>> {
        let Some(Bound::Value(list)) = &label.font else {
            return Vec::new();
        };
        let entries: Vec<&str> = list.split(',').collect();
        let mut usable = Vec::new();
        for (i, entry) in entries.iter().enumerate() {
            let stripped = entry.split_once(':').map(|(_, name)| name);
            let mut found = None;
            let mut why = format!("the font set has no font `{entry}`");
            for name in stripped.into_iter().chain([*entry]) {
                let Some(path) = map.fonts.get(name) else {
                    continue;
                };
                match self.load(name, path, loader).await {
                    Ok(bytes) => {
                        found = Some(bytes);
                        break;
                    }
                    Err(reason) => why = format!("the font `{name}` ({path}) {reason}"),
                }
            }
            match found {
                Some(bytes) => usable.push(bytes),
                None if i + 1 == entries.len() => diagnostics.push(Diagnostic::warning(
                    "label-font-unusable",
                    format!("{why}; the label is not drawn"),
                    label.span.clone(),
                )),
                None => {}
            }
        }
        usable
    }

    /// The font `name` at `path`, loaded once per render.
    async fn load<L: ResourceLoader>(
        &mut self,
        name: &str,
        path: &str,
        loader: &L,
    ) -> Result<Rc<[u8]>, String> {
        if let Some(loaded) = self.loaded.get(name) {
            return loaded.clone();
        }
        let loaded = match ResourcePath::new(path) {
            Err(e) => Err(format!("is no path inside the base context: {e}")),
            Ok(resource) => match loader.load(&resource).await {
                Err(e) => Err(format!("does not load: {e}")),
                Ok(bytes) if bytes.len() > MAX_FONT_BYTES => Err(format!(
                    "exceeds the limit of {MAX_FONT_BYTES} bytes for a font"
                )),
                Ok(bytes) => Ok(Rc::from(bytes)),
            },
        };
        self.loaded.insert(name.to_owned(), loaded.clone());
        loaded
    }
}
