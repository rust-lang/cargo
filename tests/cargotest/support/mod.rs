use std::env;
use std::error::Error;
use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::io::prelude::*;
use std::os;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::str;
use std::usize;

use rustc_serialize::json::Json;
use url::Url;
use hamcrest as ham;
use cargo::util::ProcessBuilder;
use cargo::util::ProcessError;
use cargo::util::process;

use support::paths::CargoPathExt;

#[macro_export]
macro_rules! t {
    ($e:expr) => (match $e {
        Ok(e) => e,
        Err(e) => panic!("{} failed with {}", stringify!($e), e),
    })
}

pub mod paths;
pub mod git;
pub mod registry;

/*
 *
 * ===== Builders =====
 *
 */

#[derive(PartialEq,Clone)]
struct FileBuilder {
    path: PathBuf,
    body: String
}

impl FileBuilder {
    pub fn new(path: PathBuf, body: &str) -> FileBuilder {
        FileBuilder { path: path, body: body.to_string() }
    }

    fn mk(&self) {
        self.dirname().mkdir_p();

        let mut file = fs::File::create(&self.path).unwrap_or_else(|e| {
            panic!("could not create file {}: {}", self.path.display(), e)
        });

        t!(file.write_all(self.body.as_bytes()));
    }

    fn dirname(&self) -> &Path {
        self.path.parent().unwrap()
    }
}

#[derive(PartialEq,Clone)]
struct SymlinkBuilder {
    dst: PathBuf,
    src: PathBuf,
}

impl SymlinkBuilder {
    pub fn new(dst: PathBuf, src: PathBuf) -> SymlinkBuilder {
        SymlinkBuilder { dst: dst, src: src }
    }

    #[cfg(unix)]
    fn mk(&self) {
        self.dirname().mkdir_p();
        t!(os::unix::fs::symlink(&self.dst, &self.src));
    }

    #[cfg(windows)]
    fn mk(&self) {
        self.dirname().mkdir_p();
        t!(os::windows::fs::symlink_file(&self.dst, &self.src));
    }

    fn dirname(&self) -> &Path {
        self.src.parent().unwrap()
    }
}

#[derive(PartialEq,Clone)]
pub struct ProjectBuilder {
    name: String,
    root: PathBuf,
    files: Vec<FileBuilder>,
    symlinks: Vec<SymlinkBuilder>
}

impl ProjectBuilder {
    pub fn new(name: &str, root: PathBuf) -> ProjectBuilder {
        ProjectBuilder {
            name: name.to_string(),
            root: root,
            files: vec![],
            symlinks: vec![]
        }
    }

    pub fn root(&self) -> PathBuf {
        self.root.clone()
    }

    pub fn url(&self) -> Url { path2url(self.root()) }

    pub fn bin(&self, b: &str) -> PathBuf {
        self.build_dir().join("debug").join(&format!("{}{}", b,
                                                     env::consts::EXE_SUFFIX))
    }

    pub fn release_bin(&self, b: &str) -> PathBuf {
        self.build_dir().join("release").join(&format!("{}{}", b,
                                                       env::consts::EXE_SUFFIX))
    }

    pub fn target_bin(&self, target: &str, b: &str) -> PathBuf {
        self.build_dir().join(target).join("debug")
                        .join(&format!("{}{}", b, env::consts::EXE_SUFFIX))
    }

    pub fn build_dir(&self) -> PathBuf {
        self.root.join("target")
    }

    pub fn process<T: AsRef<OsStr>>(&self, program: T) -> ProcessBuilder {
        let mut p = ::process(program);
        p.cwd(self.root());
        return p
    }

    pub fn cargo(&self, cmd: &str) -> ProcessBuilder {
        let mut p = self.process(&cargo_dir().join("cargo"));
        p.arg(cmd);
        return p;
    }

    pub fn cargo_process(&self, cmd: &str) -> ProcessBuilder {
        self.build();
        self.cargo(cmd)
    }

    pub fn file<B: AsRef<Path>>(mut self, path: B,
                                body: &str) -> ProjectBuilder {
        self.files.push(FileBuilder::new(self.root.join(path), body));
        self
    }

    pub fn symlink<T: AsRef<Path>>(mut self, dst: T,
                                   src: T) -> ProjectBuilder {
        self.symlinks.push(SymlinkBuilder::new(self.root.join(dst),
                                               self.root.join(src)));
        self
    }

    // TODO: return something different than a ProjectBuilder
    pub fn build(&self) -> &ProjectBuilder {
        // First, clean the directory if it already exists
        self.rm_root();

        // Create the empty directory
        self.root.mkdir_p();

        for file in self.files.iter() {
            file.mk();
        }

        for symlink in self.symlinks.iter() {
            symlink.mk();
        }
        self
    }

    fn rm_root(&self) {
        self.root.rm_rf()
    }
}

// Generates a project layout
pub fn project(name: &str) -> ProjectBuilder {
    ProjectBuilder::new(name, paths::root().join(name))
}

// === Helpers ===

pub fn main_file(println: &str, deps: &[&str]) -> String {
    let mut buf = String::new();

    for dep in deps.iter() {
        buf.push_str(&format!("extern crate {};\n", dep));
    }

    buf.push_str("fn main() { println!(");
    buf.push_str(&println);
    buf.push_str("); }\n");

    buf.to_string()
}

trait ErrMsg<T> {
    fn with_err_msg(self, val: String) -> Result<T, String>;
}

impl<T, E: fmt::Display> ErrMsg<T> for Result<T, E> {
    fn with_err_msg(self, val: String) -> Result<T, String> {
        match self {
            Ok(val) => Ok(val),
            Err(err) => Err(format!("{}; original={}", val, err))
        }
    }
}

// Path to cargo executables
pub fn cargo_dir() -> PathBuf {
    env::var_os("CARGO_BIN_PATH").map(PathBuf::from).or_else(|| {
        env::current_exe().ok().as_ref().and_then(|s| s.parent())
            .map(|s| s.to_path_buf())
    }).unwrap_or_else(|| {
        panic!("CARGO_BIN_PATH wasn't set. Cannot continue running test")
    })
}

/// Returns an absolute path in the filesystem that `path` points to. The
/// returned path does not contain any symlinks in its hierarchy.
/*
 *
 * ===== Matchers =====
 *
 */

#[derive(Clone)]
pub struct Execs {
    expect_stdout: Option<String>,
    expect_stdin: Option<String>,
    expect_stderr: Option<String>,
    expect_exit_code: Option<i32>,
    expect_stdout_contains: Vec<String>,
    expect_stderr_contains: Vec<String>,
    expect_json: Option<Json>,
}

impl Execs {
    pub fn with_stdout<S: ToString>(mut self, expected: S) -> Execs {
        self.expect_stdout = Some(expected.to_string());
        self
    }

    pub fn with_stderr<S: ToString>(mut self, expected: S) -> Execs {
        self.expect_stderr = Some(expected.to_string());
        self
    }

    pub fn with_status(mut self, expected: i32) -> Execs {
        self.expect_exit_code = Some(expected);
        self
    }

    pub fn with_stdout_contains<S: ToString>(mut self, expected: S) -> Execs {
        self.expect_stdout_contains.push(expected.to_string());
        self
    }

    pub fn with_stderr_contains<S: ToString>(mut self, expected: S) -> Execs {
        self.expect_stderr_contains.push(expected.to_string());
        self
    }

    pub fn with_json(mut self, expected: &str) -> Execs {
        self.expect_json = Some(Json::from_str(expected).unwrap());
        self
    }

    fn match_output(&self, actual: &Output) -> ham::MatchResult {
        self.match_status(actual)
            .and(self.match_stdout(actual))
            .and(self.match_stderr(actual))
    }

    fn match_status(&self, actual: &Output) -> ham::MatchResult {
        match self.expect_exit_code {
            None => ham::success(),
            Some(code) => {
                ham::expect(
                    actual.status.code() == Some(code),
                    format!("exited with {}\n--- stdout\n{}\n--- stderr\n{}",
                            actual.status,
                            String::from_utf8_lossy(&actual.stdout),
                            String::from_utf8_lossy(&actual.stderr)))
            }
        }
    }

    fn match_stdout(&self, actual: &Output) -> ham::MatchResult {
        try!(self.match_std(self.expect_stdout.as_ref(), &actual.stdout,
                            "stdout", &actual.stderr, false));
        for expect in self.expect_stdout_contains.iter() {
            try!(self.match_std(Some(expect), &actual.stdout, "stdout",
                                &actual.stderr, true));
        }
        for expect in self.expect_stderr_contains.iter() {
            try!(self.match_std(Some(expect), &actual.stderr, "stderr",
                                &actual.stdout, true));
        }

        if let Some(ref expect_json) = self.expect_json {
            try!(self.match_json(expect_json, &actual.stdout));
        }
        Ok(())
    }

    fn match_stderr(&self, actual: &Output) -> ham::MatchResult {
        self.match_std(self.expect_stderr.as_ref(), &actual.stderr,
                       "stderr", &actual.stdout, false)
    }

    fn match_std(&self, expected: Option<&String>, actual: &[u8],
                 description: &str, extra: &[u8],
                 partial: bool) -> ham::MatchResult {
        let out = match expected {
            Some(out) => substitute_macros(out),
            None => return ham::success(),
        };
        let actual = match str::from_utf8(actual) {
            Err(..) => return Err(format!("{} was not utf8 encoded",
                                       description)),
            Ok(actual) => actual,
        };
        // Let's not deal with \r\n vs \n on windows...
        let actual = actual.replace("\r", "");
        let actual = actual.replace("\t", "<tab>");

        let mut a = actual.lines();
        let e = out.lines();

        let diffs = if partial {
            let mut min = self.diff_lines(a.clone(), e.clone(), partial);
            while let Some(..) = a.next() {
                let a = self.diff_lines(a.clone(), e.clone(), partial);
                if a.len() < min.len() {
                    min = a;
                }
            }
            min
        } else {
            self.diff_lines(a, e, partial)
        };
        ham::expect(diffs.is_empty(),
                    format!("differences:\n\
                            {}\n\n\
                            other output:\n\
                            `{}`", diffs.join("\n"),
                            String::from_utf8_lossy(extra)))

    }

    fn match_json(&self, expected: &Json, stdout: &[u8]) -> ham::MatchResult {
        let stdout = match str::from_utf8(stdout) {
            Err(..) => return Err("stdout was not utf8 encoded".to_owned()),
            Ok(stdout) => stdout,
        };

        let actual = match Json::from_str(stdout) {
             Err(..) => return Err(format!("Invalid json {}", stdout)),
             Ok(actual) => actual,
        };

        match find_mismatch(expected, &actual) {
            Some((expected_part, actual_part)) => Err(format!(
                "JSON mismatch\nExpected:\n{}\nWas:\n{}\nExpected part:\n{}\nActual part:\n{}\n",
                expected.pretty(), actual.pretty(),
                expected_part.pretty(), actual_part.pretty()
            )),
            None => Ok(()),
        }
    }

    fn diff_lines<'a>(&self, actual: str::Lines<'a>, expected: str::Lines<'a>,
                      partial: bool) -> Vec<String> {
        let actual = actual.take(if partial {
            expected.clone().count()
        } else {
            usize::MAX
        });
        zip_all(actual, expected).enumerate().filter_map(|(i, (a,e))| {
            match (a, e) {
                (Some(a), Some(e)) => {
                    if lines_match(&e, &a) {
                        None
                    } else {
                        Some(format!("{:3} - |{}|\n    + |{}|\n", i, e, a))
                    }
                },
                (Some(a), None) => {
                    Some(format!("{:3} -\n    + |{}|\n", i, a))
                },
                (None, Some(e)) => {
                    Some(format!("{:3} - |{}|\n    +\n", i, e))
                },
                (None, None) => panic!("Cannot get here")
            }
        }).collect()
    }
}

fn lines_match(expected: &str, mut actual: &str) -> bool {
    for (i, part) in expected.split("[..]").enumerate() {
        match actual.find(part) {
            Some(j) => {
                if i == 0 && j != 0 {
                    return false
                }
                actual = &actual[j + part.len()..];
            }
            None => {
                return false
            }
        }
    }
    actual.is_empty() || expected.ends_with("[..]")
}

#[test]
fn lines_match_works() {
    assert!(lines_match("a b", "a b"));
    assert!(lines_match("a[..]b", "a b"));
    assert!(lines_match("a[..]", "a b"));
    assert!(lines_match("[..]", "a b"));
    assert!(lines_match("[..]b", "a b"));

    assert!(!lines_match("[..]b", "c"));
    assert!(!lines_match("b", "c"));
    assert!(!lines_match("b", "cb"));
}

// Compares JSON object for approximate equality.
// You can use `[..]` wildcard in strings (useful for OS dependent things such as paths).
// Arrays are sorted before comparison.
fn find_mismatch<'a>(expected: &'a Json, actual: &'a Json) -> Option<(&'a Json, &'a Json)> {
    use rustc_serialize::json::Json::*;
    match (expected, actual) {
        (&I64(l), &I64(r)) if l == r => None,
        (&F64(l), &F64(r)) if l == r => None,
        (&U64(l), &U64(r)) if l == r => None,
        (&Boolean(l), &Boolean(r)) if l == r => None,
        (&String(ref l), &String(ref r)) if lines_match(l, r) => None,
        (&Array(ref l), &Array(ref r)) => {
            if l.len() != r.len() {
                return Some((expected, actual));
            }

            fn sorted(xs: &Vec<Json>) -> Vec<&Json> {
                let mut result = xs.iter().collect::<Vec<_>>();
                // `unwrap` should be safe because JSON spec does not allow NaNs
                result.sort_by(|x, y| x.partial_cmp(y).unwrap());
                result
            }

            sorted(l).iter().zip(sorted(r))
             .filter_map(|(l, r)| find_mismatch(l, r))
             .nth(0)
        }
        (&Object(ref l), &Object(ref r)) => {
            let same_keys = l.len() == r.len() && l.keys().all(|k| r.contains_key(k));
            if !same_keys {
                return Some((expected, actual));
            }

            l.values().zip(r.values())
             .filter_map(|(l, r)| find_mismatch(l, r))
             .nth(0)
        }
        (&Null, &Null) => None,
        _ => Some((expected, actual)),
    }

}

struct ZipAll<I1: Iterator, I2: Iterator> {
    first: I1,
    second: I2,
}

impl<T, I1: Iterator<Item=T>, I2: Iterator<Item=T>> Iterator for ZipAll<I1, I2> {
    type Item = (Option<T>, Option<T>);
    fn next(&mut self) -> Option<(Option<T>, Option<T>)> {
        let first = self.first.next();
        let second = self.second.next();

        match (first, second) {
            (None, None) => None,
            (a, b) => Some((a, b))
        }
    }
}

fn zip_all<T, I1: Iterator<Item=T>, I2: Iterator<Item=T>>(a: I1, b: I2) -> ZipAll<I1, I2> {
    ZipAll {
        first: a,
        second: b,
    }
}

impl fmt::Display for Execs {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "execs")
    }
}

impl ham::Matcher<ProcessBuilder> for Execs {
    fn matches(&self, mut process: ProcessBuilder) -> ham::MatchResult {
        self.matches(&mut process)
    }
}

impl<'a> ham::Matcher<&'a mut ProcessBuilder> for Execs {
    fn matches(&self, process: &'a mut ProcessBuilder) -> ham::MatchResult {
        println!("running {}", process);
        let res = process.exec_with_output();

        match res {
            Ok(out) => self.match_output(&out),
            Err(ProcessError { output: Some(ref out), .. }) => {
                self.match_output(out)
            }
            Err(e) => {
                let mut s = format!("could not exec process {}: {}", process, e);
                match e.cause() {
                    Some(cause) => s.push_str(&format!("\ncaused by: {}",
                                                       cause.description())),
                    None => {}
                }
                Err(s)
            }
        }
    }
}

impl ham::Matcher<Output> for Execs {
    fn matches(&self, output: Output) -> ham::MatchResult {
        self.match_output(&output)
    }
}

pub fn execs() -> Execs {
    Execs {
        expect_stdout: None,
        expect_stderr: None,
        expect_stdin: None,
        expect_exit_code: None,
        expect_stdout_contains: Vec::new(),
        expect_stderr_contains: Vec::new(),
        expect_json: None,
    }
}

#[derive(Clone)]
pub struct ShellWrites {
    expected: String
}

impl fmt::Display for ShellWrites {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "`{}` written to the shell", self.expected)
    }
}

impl<'a> ham::Matcher<&'a [u8]> for ShellWrites {
    fn matches(&self, actual: &[u8])
        -> ham::MatchResult
    {
        let actual = String::from_utf8_lossy(actual);
        let actual = actual.to_string();
        ham::expect(actual == self.expected, actual)
    }
}

pub fn shell_writes<T: fmt::Display>(string: T) -> ShellWrites {
    ShellWrites { expected: string.to_string() }
}

pub trait Tap {
    fn tap<F: FnOnce(&mut Self)>(mut self, callback: F) -> Self;
}

impl<T> Tap for T {
    fn tap<F: FnOnce(&mut Self)>(mut self, callback: F) -> T {
        callback(&mut self);
        self
    }
}

pub fn basic_bin_manifest(name: &str) -> String {
    format!(r#"
        [package]

        name = "{}"
        version = "0.5.0"
        authors = ["wycats@example.com"]

        [[bin]]

        name = "{}"
    "#, name, name)
}

pub fn basic_lib_manifest(name: &str) -> String {
    format!(r#"
        [package]

        name = "{}"
        version = "0.5.0"
        authors = ["wycats@example.com"]

        [lib]

        name = "{}"
    "#, name, name)
}

pub fn path2url(p: PathBuf) -> Url {
    Url::from_file_path(&*p).ok().unwrap()
}

fn substitute_macros(input: &str) -> String {
    let macros = [
        ("[RUNNING]",     "     Running"),
        ("[COMPILING]",   "   Compiling"),
        ("[CREATED]",     "     Created"),
        ("[FINISHED]",    "    Finished"),
        ("[ERROR]",       "error:"),
        ("[WARNING]",     "warning:"),
        ("[DOCUMENTING]", " Documenting"),
        ("[FRESH]",       "       Fresh"),
        ("[UPDATING]",    "    Updating"),
        ("[ADDING]",      "      Adding"),
        ("[REMOVING]",    "    Removing"),
        ("[DOCTEST]",     "   Doc-tests"),
        ("[PACKAGING]",   "   Packaging"),
        ("[DOWNLOADING]", " Downloading"),
        ("[UPLOADING]",   "   Uploading"),
        ("[VERIFYING]",   "   Verifying"),
        ("[ARCHIVING]",   "   Archiving"),
        ("[INSTALLING]",  "  Installing"),
        ("[REPLACING]",   "   Replacing")
    ];
    let mut result = input.to_owned();
    for &(pat, subst) in macros.iter() {
        result = result.replace(pat, subst)
    }
    return result;
}
