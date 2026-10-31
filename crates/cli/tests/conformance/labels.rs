//! MapServer's label cache as the reader of ADR-0029, `mapserver_labels.c`, writes it, the record
//! of it the list keeps for each text case under `labels/`, and the comparison of Forager's labels
//! with it that G-10 asks for.

use std::collections::BTreeMap;
use std::path::PathBuf;

use forager_engine::CachedLabel;
use forager_style_ir::Map;

use super::Case;

/// How far each edge of a label's box may lie from MapServer's, in pixels (G-10).
pub const TOLERANCE: f64 = 1.0;

/// A label of MapServer's cache, one line of the reader's output.
#[derive(Debug, Clone, PartialEq)]
pub struct Recorded {
    pub priority: u8,
    pub layer: usize,
    pub layer_name: String,
    pub class: usize,
    /// The box it was drawn in, `[min x, min y, max x, max y]`; `None` for a label discarded.
    pub drawn: Option<[f64; 4]>,
    pub texts: Vec<String>,
}

/// The record of `case`'s labels beside the list: `labels/<directory>/<result>.tsv`.
pub fn record_path(case: &Case) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/conformance/labels")
        .join(case.mapfile.trim_end_matches(".map"))
        .join(format!("{}.tsv", case.result.trim_end_matches(".png")))
}

/// Reads the reader's output: one tab-separated line per label, its priority, its layer's index
/// and name, its class's index, 1 for drawn or 0, its box as four numbers for a drawn label and
/// empty otherwise, and its texts; a line of another form is an error naming it.
pub fn read(text: &str) -> Result<Vec<Recorded>, String> {
    let mut labels = Vec::new();
    for (number, line) in text.lines().enumerate() {
        let number = number + 1;
        let fields: Vec<&str> = line.split('\t').collect();
        let [priority, layer, layer_name, class, drawn, bbox, texts @ ..] = fields.as_slice()
        else {
            return Err(format!("line {number}: `{line}` has too few fields"));
        };
        let int = |field: &str, what: &str| {
            field
                .parse::<usize>()
                .map_err(|_| format!("line {number}: `{field}` is no {what}"))
        };
        let drawn = match (*drawn, *bbox) {
            ("1", bbox) => {
                let edges: Vec<f64> = bbox
                    .split(' ')
                    .map(str::parse)
                    .collect::<Result<_, _>>()
                    .map_err(|_| format!("line {number}: `{bbox}` is no box"))?;
                let [x0, y0, x1, y1] = edges[..] else {
                    return Err(format!("line {number}: `{bbox}` is no box"));
                };
                Some([x0, y0, x1, y1])
            }
            ("0", "") => None,
            _ => {
                return Err(format!(
                    "line {number}: `{drawn}` `{bbox}` is no drawn state"
                ));
            }
        };
        labels.push(Recorded {
            priority: u8::try_from(int(priority, "priority")?)
                .map_err(|_| format!("line {number}: `{priority}` is no priority"))?,
            layer: int(layer, "layer index")?,
            layer_name: unescape(layer_name),
            class: int(class, "class index")?,
            drawn,
            texts: texts.iter().map(|t| unescape(t)).collect(),
        });
    }
    Ok(labels)
}

/// A field without the reader's escapes of a tab, a line feed, a carriage return, and a backslash.
fn unescape(field: &str) -> String {
    let mut out = String::with_capacity(field.len());
    let mut chars = field.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

/// What differs between Forager's labels and MapServer's, one line per label, as ADR-0029 pairs
/// them: by layer, class, and text, the n-th of a key in Forager's walk with the n-th in
/// MapServer's. A pair matches when both are drawn with each edge within [`TOLERANCE`], or when
/// both are discarded; a label without a partner does not match. Empty when every label matches.
pub fn compare(forager: &[CachedLabel], mapserver: &[Recorded]) -> Vec<String> {
    type Key = (usize, usize, String);
    let mut theirs: BTreeMap<Key, Vec<&Recorded>> = BTreeMap::new();
    for label in mapserver {
        let key = (label.layer, label.class, label.texts.join("\n"));
        theirs.entry(key).or_default().push(label);
    }
    let mut ours: BTreeMap<Key, Vec<&CachedLabel>> = BTreeMap::new();
    for label in forager {
        let key = (label.layer, label.class, label.text.clone());
        ours.entry(key).or_default().push(label);
    }
    let mut keys: Vec<&Key> = theirs.keys().chain(ours.keys()).collect();
    keys.sort();
    keys.dedup();
    let mut differences = Vec::new();
    for key in keys {
        let (layer, class, text) = key;
        let name = format!("label {text:?} of layer {layer}, class {class}");
        let ms = theirs.get(key).map_or(&[][..], Vec::as_slice);
        let fg = ours.get(key).map_or(&[][..], Vec::as_slice);
        for n in 0..ms.len().max(fg.len()) {
            let nth = if ms.len().max(fg.len()) > 1 {
                format!(" ({} of its text)", n + 1)
            } else {
                String::new()
            };
            let difference = match (ms.get(n).map(|l| l.drawn), fg.get(n).map(|l| l.drawn)) {
                (Some(Some(a)), Some(Some(b))) => {
                    let off = distance(&a, &b);
                    (off > TOLERANCE).then(|| {
                        format!(
                            "MapServer draws it at {}, Forager {off:.2} px off",
                            show(&a)
                        )
                    })
                }
                (Some(None), Some(None)) => None,
                (Some(Some(a)), Some(None)) => Some(format!(
                    "MapServer draws it at {}, Forager discards it",
                    show(&a)
                )),
                (Some(None), Some(Some(b))) => Some(format!(
                    "Forager draws it at {}, MapServer discards it",
                    show(&b)
                )),
                (Some(_), None) => Some("only MapServer caches it".to_owned()),
                (None, Some(_)) => Some("only Forager caches it".to_owned()),
                (None, None) => None,
            };
            if let Some(difference) = difference {
                differences.push(format!("{name}{nth}: {difference}"));
            }
        }
    }
    differences
}

/// The layers of `map` whose labels the comparison does not see, one line each: a layer of
/// `LABELCACHE OFF` draws its labels at once, and neither Forager's `Rendered.labels` nor the
/// reader of MapServer's cache, which holds only the cache, lists them.
pub fn uncompared(map: &Map) -> Vec<String> {
    map.layers
        .iter()
        .enumerate()
        .filter(|(_, layer)| !layer.label_cache)
        .filter(|(_, layer)| layer.classes.iter().any(|class| !class.labels.is_empty()))
        .map(|(index, layer)| {
            format!(
                "the labels of layer {index} ({}) are not compared: LABELCACHE OFF draws them at \
                 once, outside the label cache",
                layer.name.as_deref().unwrap_or("unnamed")
            )
        })
        .collect()
}

/// The largest distance between an edge of `a` and the same edge of `b`.
fn distance(a: &[f64; 4], b: &[f64; 4]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f64::max)
}

fn show(bbox: &[f64; 4]) -> String {
    format!(
        "[{:.2} {:.2} {:.2} {:.2}]",
        bbox[0], bbox[1], bbox[2], bbox[3]
    )
}
