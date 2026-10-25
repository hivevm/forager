//! The base context of `--base` and the file loader probed as an attacker would (Q-8, Q-9): every
//! path a mapfile names, and every way the loader could be led to read outside the base context.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

/// A directory of its own for one test, removed when the test ends.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("forager-base-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir.canonicalize().unwrap())
    }

    fn write(&self, name: &str, text: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// `forager read` with `args`, killed after a time no load of these fixtures comes near, so that
/// a loader that waits on a file fails the test instead of hanging it.
fn forager(args: &[&Path]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_forager"))
        .arg("read")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the forager binary runs");
    let start = Instant::now();
    while child.try_wait().unwrap().is_none() {
        if start.elapsed() > Duration::from_secs(20) {
            child.kill().unwrap();
            panic!("forager read {args:?} did not finish");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    child.wait_with_output().unwrap()
}

fn with_base(mapfile: &Path, base: &Path) -> Output {
    forager(&[mapfile, Path::new("--base"), base])
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// The load failed with `expected` among its diagnostics, not with a crash.
fn assert_refused(output: &Output, expected: &str) {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(stderr(output).contains(expected), "{}", stderr(output));
}

#[test]
fn each_path_a_mapfile_names_is_refused_outside_the_base_context() {
    // Verifies: Q-8
    let dir = Scratch::new("names");
    dir.write("secret.inc", "NAME 'secret'\n");
    dir.write("secret.sym", "SYMBOLSET END\n");
    dir.write("secret.fonts", "f secret.ttf\n");
    dir.write("base/fonts.txt", "f ../../secret.ttf\n");
    let base = dir.0.join("base");
    let cases = [
        ("INCLUDE '../secret.inc'", "error [include-refused]"),
        ("INCLUDE 'a/../../secret.inc'", "error [include-refused]"),
        ("INCLUDE '..\\secret.inc'", "error [include-refused]"),
        ("INCLUDE '/etc/passwd'", "error [include-refused]"),
        (
            "INCLUDE '\\\\server\\share\\x.inc'",
            "error [include-refused]",
        ),
        ("INCLUDE 'C:\\secret.inc'", "error [include-refused]"),
        ("SYMBOLSET '../secret.sym'", "error [symbolset-refused]"),
        ("FONTSET '../secret.fonts'", "error [fontset-refused]"),
        (
            "PROJECTION 'init=../data/epsg2:42304' END",
            "error [projection-unresolved]",
        ),
        (
            "SYMBOL NAME 's' TYPE PIXMAP IMAGE '../secret.png' END",
            "error [image-refused]",
        ),
    ];
    for (directive, expected) in cases {
        let mapfile = dir.write("base/m.map", &format!("MAP\n  {directive}\nEND\n"));
        assert_refused(&with_base(&mapfile, &base), expected);
    }
    // A font is needed only when drawn, so its refusal is a warning that leaves it out.
    let mapfile = dir.write("base/m.map", "MAP\n  FONTSET 'fonts.txt'\nEND\n");
    let output = with_base(&mapfile, &base);
    assert!(output.status.success(), "{output:?}");
    assert!(stderr(&output).contains("warning [font-refused]"));
}

#[test]
fn an_include_whose_dots_uncover_a_drive_prefix_is_refused() {
    // Verifies: Q-8
    let dir = Scratch::new("drive");
    let mapfile = dir.write("m.map", "MAP\n  INCLUDE 'a/../C:/secret.inc'\nEND\n");
    let output = forager(&[&mapfile]);
    assert_refused(&output, "error [include-refused]");
    assert!(stderr(&output).contains("absolute"), "{}", stderr(&output));
}

#[test]
fn a_mapfile_named_through_dots_out_of_the_base_is_refused() {
    let dir = Scratch::new("outside");
    dir.write("outside.map", "MAP END\n");
    std::fs::create_dir_all(dir.0.join("base")).unwrap();
    let mapfile = dir.0.join("base/../outside.map");
    let output = with_base(&mapfile, &dir.0.join("base"));
    assert_refused(&output, "outside the base context");
}

#[test]
fn a_base_that_is_the_mapfile_itself_is_an_error_not_a_panic() {
    let dir = Scratch::new("self");
    let mapfile = dir.write("m.map", "MAP END\n");
    assert_refused(&with_base(&mapfile, &mapfile), "the path is empty");
}

#[test]
fn a_missing_base_is_an_error_naming_it() {
    let dir = Scratch::new("missing");
    let mapfile = dir.write("m.map", "MAP END\n");
    assert_refused(&with_base(&mapfile, &dir.0.join("nowhere")), "--base");
}

#[test]
fn an_include_of_a_directory_or_a_very_long_name_is_an_error_not_a_panic() {
    let dir = Scratch::new("odd");
    std::fs::create_dir_all(dir.0.join("sub")).unwrap();
    let mapfile = dir.write("m.map", "MAP\n  INCLUDE 'sub/'\nEND\n");
    assert_eq!(forager(&[&mapfile]).status.code(), Some(1));
    let long = "a/".repeat(3000) + "x.inc";
    let mapfile = dir.write("m.map", &format!("MAP\n  INCLUDE '{long}'\nEND\n"));
    assert_eq!(forager(&[&mapfile]).status.code(), Some(1));
}

#[cfg(unix)]
#[test]
fn a_directory_link_out_of_the_base_is_refused() {
    // Verifies: Q-8
    let dir = Scratch::new("dirlink");
    dir.write("outside/secret.inc", "NAME 'secret'\n");
    std::fs::create_dir_all(dir.0.join("base")).unwrap();
    std::os::unix::fs::symlink(dir.0.join("outside"), dir.0.join("base/inc")).unwrap();
    let mapfile = dir.write("base/m.map", "MAP\n  INCLUDE 'inc/secret.inc'\nEND\n");
    assert_refused(&forager(&[&mapfile]), "the path leaves the base context");
}

#[cfg(unix)]
#[test]
fn a_link_that_stays_inside_the_base_is_followed() {
    // Verifies: Q-8
    let dir = Scratch::new("innerlink");
    dir.write("base/layers/a.inc", "NAME 'inner'\n");
    std::os::unix::fs::symlink(dir.0.join("base/layers"), dir.0.join("base/inc")).unwrap();
    let mapfile = dir.write("base/maps/m.map", "MAP\n  INCLUDE '../inc/a.inc'\nEND\n");
    let output = with_base(&mapfile, &dir.0.join("base"));
    assert!(output.status.success(), "{output:?}");
}

#[cfg(unix)]
#[test]
fn a_base_reached_through_a_link_holds_its_mapfile() {
    // Verifies: Q-8
    let dir = Scratch::new("baselink");
    dir.write("real/a.inc", "NAME 'a'\n");
    std::os::unix::fs::symlink(dir.0.join("real"), dir.0.join("link")).unwrap();
    let mapfile = dir.write("real/m.map", "MAP\n  INCLUDE 'a.inc'\nEND\n");
    let through_link = dir.0.join("link/m.map");
    assert!(
        with_base(&through_link, &dir.0.join("link"))
            .status
            .success()
    );
    assert!(with_base(&mapfile, &dir.0.join("link")).status.success());
}

#[cfg(unix)]
#[test]
fn a_mapfile_that_links_out_of_the_base_is_refused() {
    // Verifies: Q-8
    let dir = Scratch::new("maplink");
    dir.write("outside/real.map", "MAP END\n");
    std::fs::create_dir_all(dir.0.join("base")).unwrap();
    std::os::unix::fs::symlink(dir.0.join("outside/real.map"), dir.0.join("base/m.map")).unwrap();
    let output = with_base(&dir.0.join("base/m.map"), &dir.0.join("base"));
    assert_refused(&output, "outside the base context");
}

#[cfg(unix)]
#[test]
fn a_file_that_is_not_a_regular_file_is_refused_not_waited_on() {
    // Verifies: Q-9
    let dir = Scratch::new("fifo");
    let fifo = dir.0.join("pipe.inc");
    let made = Command::new("mkfifo").arg(&fifo).status().unwrap();
    assert!(made.success());
    let mapfile = dir.write("m.map", "MAP\n  INCLUDE 'pipe.inc'\nEND\n");
    assert_refused(&forager(&[&mapfile]), "not a regular file");
}

#[cfg(unix)]
#[test]
fn a_file_past_the_limit_is_cut_where_the_limit_is_not_read_whole() {
    // Verifies: Q-9
    let dir = Scratch::new("huge");
    // Sparse: it takes no space, but a loader that reads it whole asks for a terabyte.
    let huge = std::fs::File::create(dir.0.join("huge.inc")).unwrap();
    huge.set_len(1 << 40).unwrap();
    let mapfile = dir.write("m.map", "MAP\n  INCLUDE 'huge.inc'\nEND\n");
    assert_refused(&forager(&[&mapfile]), "error [mapfile-too-large]");
}
