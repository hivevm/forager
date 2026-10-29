//! The cases of the conformance harness and the list that scopes them. A case is one `RUN_PARMS`
//! line of a mapfile, read as msautotest's own test library reads it (`read_test_directives` and
//! the collection of the tests in `msautotest/pymod/mstestlib.py` at the pin): the mapfiles are
//! the files whose name ends in `.map`, in that case; the line may stand anywhere in the file, its
//! first word, cut to its file name, is the result, and the rest the command; a line of a result
//! alone runs `map2img` plainly, and a mapfile without a line runs that plain command for
//! `<name>.png`, unless it says `SKIP_WHOLE_FILE:`. The options `-l`, `-e`, and `-s` of a line are
//! applied to the style IR as `map2img` applies them; any other keeps the case out. The list,
//! `cases.txt` beside this module, names every case of the directories it covers: in scope with its
//! category and expected outcome, or out of scope with its reason, and marks the cases whose
//! reference is blank.

use std::collections::BTreeMap;
use std::path::Path;

use forager_style_ir::{Extent, Layer, LayerStatus, Map};

#[cfg(test)]
mod tests;

/// The directories of the suite whose cases the list covers.
pub const DIRECTORIES: &[&str] = &["misc", "renderers"];

/// The command of a mapfile without a `RUN_PARMS` line, or of a line that names a result alone.
const PLAIN: &str = "[MAP2IMG] [RENDERER] -m [MAPFILE] -o [RESULT]";

/// One `RUN_PARMS` line of a mapfile, or the line the test library supplies for a mapfile
/// without one.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Case {
    /// The mapfile, inside the suite, as `renderers/line_simple.map`.
    pub mapfile: String,
    /// The file the case writes, compared with the file of that name under `expected`.
    pub result: String,
    /// The command, with the test library's placeholders.
    pub command: String,
}

impl Case {
    /// The case's name in the list: its mapfile and its result.
    pub fn key(&self) -> (String, String) {
        (self.mapfile.clone(), self.result.clone())
    }

    /// Whether the harness can render the case: a `map2img` line that writes a PNG with the
    /// output format `png`, or none, and no option but those [`Case::options`] applies. The error
    /// is the reason it cannot.
    pub fn renderable(&self) -> Result<(), String> {
        self.options().map(drop)
    }

    /// The options of the case's `map2img` line that the harness applies to the style IR, in the
    /// order of the line: `-l`, `-e`, and `-s`, read as `map2img.c` at the pin reads them. The
    /// line is split into words as the shell that `os.system` runs it in splits it. The error is
    /// why the case cannot be rendered: no `map2img` line, no PNG, another output format, an
    /// option short of its arguments, or any other option.
    pub fn options(&self) -> Result<Vec<Applied>, String> {
        let words = shell_words(&self.command)?;
        let mut words = words.iter().map(String::as_str);
        if words.next() != Some("[MAP2IMG]") {
            return Err("not a map2img line".to_owned());
        }
        if !self.result.ends_with(".png") {
            return Err("its result is no PNG".to_owned());
        }
        let mut applied = Vec::new();
        let mut options = Vec::new();
        while let Some(word) = words.next() {
            match word {
                "[RENDERER]" => {}
                "-m" | "-o" => {
                    let placeholder = if word == "-m" {
                        "[MAPFILE]"
                    } else {
                        "[RESULT]"
                    };
                    match words.next() {
                        Some(next) if next == placeholder => {}
                        next => options.extend([word].into_iter().chain(next)),
                    }
                }
                "-i" => match words.next() {
                    Some("png") => {}
                    other => return Err(format!("the output format {}", other.unwrap_or(""))),
                },
                "-l" => {
                    let names = words.next().ok_or("the option -l takes a list of layers")?;
                    applied.push(Applied::Layers(split_names(names)));
                }
                "-e" => {
                    let mut bound = [0.0; 4];
                    for b in &mut bound {
                        *b = words
                            .next()
                            .and_then(|w| w.parse().ok())
                            .ok_or("the option -e takes four numbers")?;
                    }
                    applied.push(Applied::Extent(bound));
                }
                "-s" => {
                    let mut size = [0; 2];
                    for s in &mut size {
                        *s = words
                            .next()
                            .and_then(|w| w.parse().ok())
                            .ok_or("the option -s takes two sizes in pixels")?;
                    }
                    applied.push(Applied::Size(size));
                }
                other => options.push(other),
            }
        }
        if options.is_empty() {
            Ok(applied)
        } else {
            Err(format!("the options {}", options.join(" ")))
        }
    }
}

/// An option of a `map2img` line that the harness applies to the style IR before it renders.
#[derive(Debug, Clone, PartialEq)]
pub enum Applied {
    /// `-l`: the layers to draw, by name.
    Layers(Vec<String>),
    /// `-e`: the extent, as `minx miny maxx maxy`.
    Extent([f64; 4]),
    /// `-s`: the width and height of the image.
    Size([u32; 2]),
}

/// Applies `options` to `map` in their order, as `map2img.c` at the pin applies them to the
/// `mapObj` before `msDrawMap`. `-e` sets the map's extent and `-s` its size (`msMapSetSize`).
/// `-l` turns every layer whose status is not DEFAULT off, and on again each whose name is one of
/// the list's, compared without regard to ASCII case as `strcasecmp` compares it. `map2img` also
/// turns on the layers whose `GROUP` is one of the list's, and exits without an image when a name
/// is neither a layer's nor a group's; the style IR holds no group, so a name that is no layer's
/// is the error.
pub fn apply(options: &[Applied], map: &mut Map) -> Result<(), String> {
    for option in options {
        match option {
            Applied::Layers(names) => {
                let named = |layer: &Layer, name: &str| {
                    layer
                        .name
                        .as_deref()
                        .is_some_and(|n| n.eq_ignore_ascii_case(name))
                };
                if let Some(name) = names
                    .iter()
                    .find(|name| !map.layers.iter().any(|layer| named(layer, name)))
                {
                    return Err(format!(
                        "the option -l names `{name}`, which is no layer's name; map2img would \
                         look for a GROUP of that name, which the style IR does not hold"
                    ));
                }
                for layer in &mut map.layers {
                    if layer.status != Some(LayerStatus::Default) {
                        let on = names.iter().any(|name| named(layer, name));
                        layer.status = Some(if on {
                            LayerStatus::On
                        } else {
                            LayerStatus::Off
                        });
                    }
                }
            }
            Applied::Extent([min_x, min_y, max_x, max_y]) => {
                map.extent = Some(Extent {
                    min_x: *min_x,
                    min_y: *min_y,
                    max_x: *max_x,
                    max_y: *max_y,
                });
            }
            Applied::Size(size) => map.size = Some(*size),
        }
    }
    Ok(())
}

/// The names of a `-l` argument, split at spaces as `msStringSplit` splits them: a run of spaces
/// separates two names, and a space at either end leaves an empty name there.
fn split_names(names: &str) -> Vec<String> {
    let mut split = vec![String::new()];
    let mut last = None;
    for c in names.chars() {
        if c == ' ' {
            if last != Some(' ') {
                split.push(String::new());
            }
        } else if let Some(name) = split.last_mut() {
            name.push(c);
        }
        last = Some(c);
    }
    split
}

/// The words of `command` as a POSIX shell splits them: at unquoted blanks, with single quotes
/// taking what they enclose literally, double quotes and a backslash outside quotes escaping
/// the next character. Nothing else of the shell is read: a redirection or a substitution stays
/// a word, which no option of the harness accepts. An unclosed quote is an error.
fn shell_words(command: &str) -> Result<Vec<String>, String> {
    let mut words = Vec::new();
    let mut word: Option<String> = None;
    let mut chars = command.chars();
    while let Some(c) = chars.next() {
        match c {
            ' ' | '\t' | '\n' => words.extend(word.take()),
            '\'' => {
                let w = word.get_or_insert_with(String::new);
                loop {
                    match chars.next() {
                        Some('\'') => break,
                        Some(c) => w.push(c),
                        None => return Err("the line leaves a quote open".to_owned()),
                    }
                }
            }
            '"' => {
                let w = word.get_or_insert_with(String::new);
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            Some(c @ ('"' | '\\' | '$' | '`')) => w.push(c),
                            Some(c) => w.extend(['\\', c]),
                            None => return Err("the line leaves a quote open".to_owned()),
                        },
                        Some(c) => w.push(c),
                        None => return Err("the line leaves a quote open".to_owned()),
                    }
                }
            }
            '\\' => word.get_or_insert_with(String::new).extend(chars.next()),
            c => word.get_or_insert_with(String::new).push(c),
        }
    }
    words.extend(word);
    Ok(words)
}

/// The cases of the mapfiles in `dirs` of the suite, in the order of their mapfiles' names and,
/// within one, of their lines.
pub fn cases(suite: &Path, dirs: &[&str]) -> Vec<Case> {
    let mut cases = Vec::new();
    for dir in dirs {
        let mut mapfiles: Vec<_> = std::fs::read_dir(suite.join(dir))
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.is_file() && p.to_str().is_some_and(|p| p.ends_with(".map")))
            .collect();
        mapfiles.sort();
        for path in mapfiles {
            let name = path.file_name().unwrap().to_str().unwrap();
            let text = String::from_utf8_lossy(&std::fs::read(&path).unwrap()).into_owned();
            cases.extend(directives(&format!("{dir}/{name}"), &text));
        }
    }
    cases
}

/// The cases of the mapfile `mapfile` whose text is `text`, as `read_test_directives` reads them.
pub fn directives(mapfile: &str, text: &str) -> Vec<Case> {
    let mut cases = Vec::new();
    let mut skip_whole_file = false;
    for line in text.lines() {
        if let Some(at) = line.find("RUN_PARMS:") {
            let rest = line[at + "RUN_PARMS:".len()..].trim_start();
            let (result, command) = rest
                .split_once(char::is_whitespace)
                .map_or((rest, ""), |(r, c)| (r, c.trim()));
            // `os.path.basename` of the result, as the test library takes it.
            let result = &result[result.rfind('/').map_or(0, |i| i + 1)..];
            if !result.is_empty() {
                let command = if command.is_empty() { PLAIN } else { command };
                cases.push(Case {
                    mapfile: mapfile.to_owned(),
                    result: result.to_owned(),
                    command: command.to_owned(),
                });
            }
        }
        if line.contains("SKIP_WHOLE_FILE:") {
            skip_whole_file = true;
        }
    }
    if cases.is_empty() && !skip_whole_file {
        let stem = &mapfile[mapfile.rfind('/').map_or(0, |i| i + 1)..mapfile.len() - 4];
        cases.push(Case {
            mapfile: mapfile.to_owned(),
            result: format!("{stem}.png"),
            command: PLAIN.to_owned(),
        });
    }
    cases
}

/// What the list says of a case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    /// The harness renders the case; it compares within the category's metric as `passes` says.
    InScope { category: Category, passes: bool },
    /// The harness does not render the case, for this reason.
    Out(String),
}

/// One line of the list: what it says of the case, and whether it marks the case's reference
/// blank.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    pub entry: Entry,
    pub blank: bool,
}

/// The categories of G-6 the list knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    /// Geometry and symbols.
    Geometry,
}

/// Reads the list: a line `<mapfile> <result> [blank] geometry passes|fails` names a case in
/// scope, a line `<mapfile> <result> [blank] out: <reason>` one out of scope, `blank` marking a
/// case whose reference is blank; `#` begins a comment, and blank lines are ignored. A case named
/// twice, a line of another form, or an empty reason is an error naming its line.
pub fn list(text: &str) -> Result<BTreeMap<(String, String), Listed>, String> {
    let mut entries = BTreeMap::new();
    for (number, line) in text.lines().enumerate() {
        let number = number + 1;
        let line = line.split_once('#').map_or(line, |(l, _)| l).trim();
        if line.is_empty() {
            continue;
        }
        let mut words = line.splitn(3, char::is_whitespace);
        let (Some(mapfile), Some(result), Some(rest)) = (words.next(), words.next(), words.next())
        else {
            return Err(format!("line {number}: `{line}` names no outcome"));
        };
        let rest = rest.trim();
        let (blank, rest) = match rest.strip_prefix("blank") {
            Some(after) if after.starts_with(char::is_whitespace) => (true, after.trim_start()),
            _ => (false, rest),
        };
        let entry = if let Some(reason) = rest.strip_prefix("out:") {
            let reason = reason.trim();
            if reason.is_empty() {
                return Err(format!(
                    "line {number}: an out-of-scope case needs its reason"
                ));
            }
            Entry::Out(reason.to_owned())
        } else {
            let words: Vec<_> = rest.split_whitespace().collect();
            let category = match words.first() {
                Some(&"geometry") => Category::Geometry,
                _ => return Err(format!("line {number}: `{rest}` is no category")),
            };
            let passes = match words.get(1..) {
                Some(["passes"]) => true,
                Some(["fails"]) => false,
                _ => return Err(format!("line {number}: `{rest}` is no outcome")),
            };
            Entry::InScope { category, passes }
        };
        if entries
            .insert(
                (mapfile.to_owned(), result.to_owned()),
                Listed { entry, blank },
            )
            .is_some()
        {
            return Err(format!("line {number}: {mapfile} {result} is named twice"));
        }
    }
    Ok(entries)
}

/// What is wrong with a case in scope after the harness tried it.
#[derive(Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The case was rendered and compared, and its outcome is not the one the list expects.
    Differs(String),
    /// The case could not be rendered or compared: its reference is missing or broken, or its
    /// mapfile does not load or renders no image. That is a problem of the list or the harness,
    /// whatever outcome the list expects, not a failing case.
    Broken(String),
}

/// What the harness makes of `case`, which the list expects to pass or not as `passes` says,
/// from `outcome`: whether it passed with what the comparison said, or why it could not be
/// rendered or compared. `None` is a case that holds its outcome.
pub fn verdict(
    case: &Case,
    passes: bool,
    outcome: Result<(bool, String), String>,
) -> Option<Verdict> {
    match outcome {
        Err(why) => Some(Verdict::Broken(format!(
            "{} {}: {why}",
            case.mapfile, case.result
        ))),
        Ok((passed, _)) if passed == passes => None,
        Ok((_, said)) => {
            let expected = if passes { "pass" } else { "fail" };
            Some(Verdict::Differs(format!(
                "{} {}: {said}; the list expects it to {expected}",
                case.mapfile, case.result
            )))
        }
    }
}

/// What a check of the cases against the list finds wrong with the list itself: the cases it
/// does not name, those it names that the suite does not hold, and those in scope that the
/// harness cannot render, each with why.
pub fn unlisted(cases: &[Case], entries: &BTreeMap<(String, String), Listed>) -> Vec<String> {
    let mut problems = Vec::new();
    let mut found = BTreeMap::new();
    for case in cases {
        found.insert(case.key(), case);
        match entries.get(&case.key()).map(|listed| &listed.entry) {
            None => problems.push(format!(
                "{} {} is not in the list: `{}`",
                case.mapfile, case.result, case.command
            )),
            Some(Entry::InScope { .. }) => {
                if let Err(reason) = case.renderable() {
                    problems.push(format!(
                        "{} {} is in scope but cannot be rendered: {reason}",
                        case.mapfile, case.result
                    ));
                }
            }
            Some(Entry::Out(_)) => {}
        }
    }
    for (mapfile, result) in entries.keys() {
        if !found.contains_key(&(mapfile.clone(), result.clone())) {
            problems.push(format!(
                "{mapfile} {result} is in the list but is no case of the suite"
            ));
        }
    }
    problems
}
