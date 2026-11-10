//! MapServer's label cache as the reader of ADR-0029, `mapserver_labels.c`, writes it, the record
//! of it the list keeps for each text case under `labels/`, and the comparison of Forager's labels
//! with it that G-10 asks for.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use forager_engine::CachedLabel;
use forager_style_ir::Map;

use super::{Case, Category, Entry, Listed};

/// How far each edge of a label's box may lie from MapServer's, in pixels (G-10).
pub const TOLERANCE: f64 = 1.0;

/// How far each edge of a box the reader writes may lie from the record's, in pixels: far below a
/// pixel, so that an update of the unpinned text libraries that moves no label holds (ADR-0029).
pub const NOISE: f64 = 1e-6;

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

/// The directory of the records beside the list.
pub fn records_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/conformance/labels")
}

/// The record of `case`'s labels beside the list: `labels/<directory>/<result>.tsv`.
pub fn record_path(case: &Case) -> PathBuf {
    records_dir().join(record(&case.mapfile, &case.result))
}

/// The record of the case of `mapfile` and `result`, inside the directory of the records:
/// `<mapfile without .map>/<result without .png>.tsv`.
pub fn record(mapfile: &str, result: &str) -> PathBuf {
    Path::new(mapfile.trim_end_matches(".map"))
        .join(format!("{}.tsv", result.trim_end_matches(".png")))
}

/// The files under `dir`, the directory of the records, each relative to it, in their order.
pub fn records(dir: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, at: &Path, found: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(at).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(dir, &path, found);
            } else {
                found.push(path.strip_prefix(dir).unwrap().to_path_buf());
            }
        }
    }
    let mut found = Vec::new();
    walk(dir, dir, &mut found);
    found.sort();
    found
}

/// What does not line up between the text cases in scope of the list, `entries`, and the records
/// under the directory of the records, `found`, each relative to it: each record that no text
/// case in scope reads, and each text case in scope without its record, one line each. Empty
/// when every record is the record of one text case in scope and every such case has its record.
pub fn unmatched(entries: &BTreeMap<(String, String), Listed>, found: &[PathBuf]) -> Vec<String> {
    let found: BTreeSet<&PathBuf> = found.iter().collect();
    let read: BTreeMap<PathBuf, (&String, &String)> = entries
        .iter()
        .filter(|(_, listed)| {
            matches!(
                listed.entry,
                Entry::InScope {
                    category: Category::Text,
                    ..
                }
            )
        })
        .map(|((mapfile, result), _)| (record(mapfile, result), (mapfile, result)))
        .collect();
    let mut problems: Vec<String> = found
        .iter()
        .filter(|path| !read.contains_key(**path))
        .map(|path| {
            format!(
                "labels/{} is the record of no text case in scope of the list",
                path.display()
            )
        })
        .collect();
    problems.extend(read.iter().filter(|(path, _)| !found.contains(path)).map(
        |(path, (mapfile, result))| {
            format!(
                "{mapfile} {result} is a text case in scope without its record labels/{}",
                path.display()
            )
        },
    ));
    problems
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
/// them: by priority, layer, class, and text, the n-th of a key in Forager's walk with the n-th in
/// MapServer's. A pair matches when both are drawn with each edge finite and within
/// [`TOLERANCE`], or when both are discarded; a label without a partner does not match. Empty when every label matches.
pub fn compare(forager: &[CachedLabel], mapserver: &[Recorded]) -> Vec<String> {
    type Key = (usize, usize, String, u8);
    let mut theirs: BTreeMap<Key, Vec<&Recorded>> = BTreeMap::new();
    for label in mapserver {
        let key = (
            label.layer,
            label.class,
            label.texts.join("\n"),
            label.priority,
        );
        theirs.entry(key).or_default().push(label);
    }
    let mut ours: BTreeMap<Key, Vec<&CachedLabel>> = BTreeMap::new();
    for label in forager {
        let key = (
            label.layer,
            label.class,
            label.text.to_string(),
            label.priority,
        );
        ours.entry(key).or_default().push(label);
    }
    let mut keys: Vec<&Key> = theirs.keys().chain(ours.keys()).collect();
    keys.sort();
    keys.dedup();
    let mut differences = Vec::new();
    for key in keys {
        let (layer, class, text, priority) = key;
        let name = format!("label {text:?} of layer {layer}, class {class}, priority {priority}");
        let ms = theirs.get(key).map_or(&[][..], Vec::as_slice);
        let fg = ours.get(key).map_or(&[][..], Vec::as_slice);
        for n in 0..ms.len().max(fg.len()) {
            let nth = if ms.len().max(fg.len()) > 1 {
                format!(" ({} of its text)", n + 1)
            } else {
                String::new()
            };
            let difference = match (ms.get(n).map(|l| l.drawn), fg.get(n).map(|l| l.drawn)) {
                (Some(Some(a)), Some(Some(b))) => unfinite("MapServer's", &a)
                    .or_else(|| unfinite("Forager's", &b))
                    .or_else(|| {
                        let off = distance(&a, &b);
                        (off > TOLERANCE).then(|| {
                            format!(
                                "MapServer draws it at {}, Forager {off:.2} px off",
                                show(&a)
                            )
                        })
                    }),
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

/// What differs between MapServer's labels as the record holds them, `record`, and as the reader
/// writes them in the noise measurement, `output`, one line each, named by its line: ADR-0029
/// holds the priority, the layer's index and name, the class, the drawn state, the texts, and the
/// order exactly, and each edge of each box finite and within [`NOISE`]. Empty when the output
/// holds the record.
pub fn beyond_noise(record: &[Recorded], output: &[Recorded]) -> Vec<String> {
    let mut differences = Vec::new();
    for n in 0..record.len().max(output.len()) {
        let line = n + 1;
        let difference = match (record.get(n), output.get(n)) {
            (Some(a), Some(b)) => {
                let fields = a.priority == b.priority
                    && a.layer == b.layer
                    && a.layer_name == b.layer_name
                    && a.class == b.class
                    && a.texts == b.texts
                    && a.drawn.is_some() == b.drawn.is_some();
                if !fields {
                    Some(format!("the record holds {a:?}, the reader {b:?}"))
                } else if let (Some(x), Some(y)) = (a.drawn, b.drawn) {
                    unfinite("the record's", &x)
                        .or_else(|| unfinite("the reader's", &y))
                        .or_else(|| {
                            let off = distance(&x, &y);
                            (off > NOISE).then(|| {
                                format!(
                                    "the reader's box {y:?} lies {off:e} px from the record's {x:?}"
                                )
                            })
                        })
                } else {
                    None
                }
            }
            (Some(a), None) => Some(format!("only the record holds {a:?}")),
            (None, Some(b)) => Some(format!("only the reader writes {b:?}")),
            (None, None) => None,
        };
        if let Some(difference) = difference {
            differences.push(format!("line {line}: {difference}"));
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

/// The first edge of `bbox` that is no finite number, named as `whose` box's: a NaN edge would
/// drop out of [`distance`], whose fold by `f64::max` returns the other operand, and a pair would
/// match on the other edges, so neither comparison measures a box that is not finite.
fn unfinite(whose: &str, bbox: &[f64; 4]) -> Option<String> {
    const EDGES: [&str; 4] = ["min x", "min y", "max x", "max y"];
    let i = bbox.iter().position(|edge| !edge.is_finite())?;
    Some(format!(
        "{whose} box is not finite in its {}: {}",
        EDGES[i], bbox[i]
    ))
}

/// The largest distance between an edge of `a` and the same edge of `b`, each finite.
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
