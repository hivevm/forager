//! The fonts a render's labels name, loaded through the resource loader once per render when a
//! label first needs them (ADR-0028).

use std::collections::BTreeMap;
use std::rc::Rc;

use forager_ports::{ResourceLoader, ResourcePath};
use forager_style_ir::{Bound, Diagnostic, Label, Map};

/// The largest font file a render loads (Q-9).
pub const MAX_FONT_BYTES: usize = 32 * 1024 * 1024;

/// The fonts loaded during one render, by their path, each its bytes or why it is not usable.
#[derive(Default)]
pub(crate) struct Fonts {
    loaded: BTreeMap<String, Result<Rc<[u8]>, String>>,
}

impl Fonts {
    /// The font `label` is drawn with: the first usable entry of its `FONT` list in the order
    /// `get_face_for_run` tries them for a Latin line, those prefixed `en:` first, then every
    /// entry in turn, each with its `xx:` prefix stripped first and then as written, and a name
    /// matched without regard to case, as `msLookupHashTable` matches it. An entry the font set
    /// lacks or that does not load is skipped; whether a font has every glyph of the text decides
    /// among the usable ones once labels are drawn. A label with no usable font gets a diagnostic
    /// naming the last entry, its path, and the label, as it cannot be drawn. A font bound to an
    /// attribute is known only per feature and is not loaded here.
    pub(crate) async fn for_label<L: ResourceLoader>(
        &mut self,
        map: &Map,
        label: &Label,
        loader: &L,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<Rc<[u8]>> {
        let Some(Bound::Value(list)) = &label.font else {
            return None;
        };
        let entries: Vec<&str> = list.split(',').collect();
        let english = entries.iter().filter_map(|e| e.strip_prefix("en:"));
        let mut why = String::new();
        for name in english.chain(entries.iter().flat_map(|e| {
            let stripped = e.split_once(':').map(|(_, name)| name);
            stripped.into_iter().chain([*e])
        })) {
            let Some(path) = map
                .fonts
                .iter()
                .find_map(|(key, path)| key.eq_ignore_ascii_case(name).then_some(path))
            else {
                why = format!("the font set has no font `{name}`");
                continue;
            };
            match self.load(path, loader).await {
                Ok(bytes) => return Some(bytes),
                Err(reason) => why = format!("the font `{name}` ({path}) {reason}"),
            }
        }
        diagnostics.push(Diagnostic::warning(
            "label-font-unusable",
            format!(
                "{why}, and no other font of the label's list can be used; the label is not drawn"
            ),
            label.span.clone(),
        ));
        None
    }

    /// The font at `path`, loaded once per render whatever names the font set gives it.
    async fn load<L: ResourceLoader>(
        &mut self,
        path: &str,
        loader: &L,
    ) -> Result<Rc<[u8]>, String> {
        if let Some(loaded) = self.loaded.get(path) {
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
        self.loaded.insert(path.to_owned(), loaded.clone());
        loaded
    }
}
