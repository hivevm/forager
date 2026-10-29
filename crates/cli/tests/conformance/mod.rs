//! The cases of the conformance harness and the list that scopes them. A case is one `RUN_PARMS`
//! line of a mapfile, read as msautotest's own test library reads it (`read_test_directives` and
//! the collection of the tests in `msautotest/pymod/mstestlib.py` at the pin): the mapfiles are
//! the files whose name ends in `.map`, in that case; the line may stand anywhere in the file, its
//! first word, cut to its file name, is the result, and the rest the command; a line of a result
//! alone runs `map2img` plainly, and a mapfile without a line runs that plain command for
//! `<name>.png`, unless it says `SKIP_WHOLE_FILE:`. The list, `cases.txt` beside this module,
//! names every case of the directories it covers: in scope with its category and expected
//! outcome, or out of scope with its reason, and marks the cases whose reference is blank.

use std::collections::BTreeMap;
use std::path::Path;

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
    /// output format `png`, or none, and no other option. The error is the reason it cannot.
    pub fn renderable(&self) -> Result<(), String> {
        let mut words = self.command.split_whitespace();
        if words.next() != Some("[MAP2IMG]") {
            return Err("not a map2img line".to_owned());
        }
        if !self.result.ends_with(".png") {
            return Err("its result is no PNG".to_owned());
        }
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
                other => options.push(other),
            }
        }
        if options.is_empty() {
            Ok(())
        } else {
            Err(format!("the options {}", options.join(" ")))
        }
    }
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
