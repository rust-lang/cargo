use std::c_str::ToCStr;
use std::error::Error;
use std::fmt::{self, Show};
use std::io::fs::{self, PathExtensions};
use std::io::process::{ProcessOutput};
use std::io;
use std::os;
use std::path::{Path,BytesContainer};
use std::str::{self, Str};

use url::Url;
use hamcrest as ham;
use cargo::util::{process,ProcessBuilder};
use cargo::util::ProcessError;

use support::paths::PathExt;

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
    path: Path,
    body: String
}

impl FileBuilder {
    pub fn new(path: Path, body: &str) -> FileBuilder {
        FileBuilder { path: path, body: body.to_string() }
    }

    fn mk(&self) -> Result<(), String> {
        try!(mkdir_recursive(&self.dirname()));

        let mut file = try!(
            fs::File::create(&self.path)
                .with_err_msg(format!("Could not create file; path={}",
                                      self.path.display())));

        file.write_str(self.body.as_slice())
            .with_err_msg(format!("Could not write to file; path={}",
                                  self.path.display()))
    }

    fn dirname(&self) -> Path {
        Path::new(self.path.dirname())
    }
}

#[derive(PartialEq,Clone)]
struct SymlinkBuilder {
    dst: Path,
    src: Path
}

impl SymlinkBuilder {
    pub fn new(dst: Path, src: Path) -> SymlinkBuilder {
        SymlinkBuilder { dst: dst, src: src }
    }

    fn mk(&self) -> Result<(), String> {
        try!(mkdir_recursive(&self.dirname()));

        fs::symlink(&self.dst, &self.src)
            .with_err_msg(format!("Could not create symlink; dst={} src={}",
                                   self.dst.display(), self.src.display()))
    }

    fn dirname(&self) -> Path {
        Path::new(self.src.dirname())
    }
}

#[derive(PartialEq,Clone)]
pub struct ProjectBuilder {
    name: String,
    root: Path,
    files: Vec<FileBuilder>,
    symlinks: Vec<SymlinkBuilder>
}

impl ProjectBuilder {
    pub fn new(name: &str, root: Path) -> ProjectBuilder {
        ProjectBuilder {
            name: name.to_string(),
            root: root,
            files: vec!(),
            symlinks: vec!()
        }
    }

    pub fn root(&self) -> Path {
        self.root.clone()
    }

    pub fn url(&self) -> Url { path2url(self.root()) }

    pub fn bin(&self, b: &str) -> Path {
        self.build_dir().join(format!("{}{}", b, os::consts::EXE_SUFFIX))
    }

    pub fn release_bin(&self, b: &str) -> Path {
        self.build_dir().join("release").join(format!("{}{}", b, os::consts::EXE_SUFFIX))
    }

    pub fn target_bin(&self, target: &str, b: &str) -> Path {
        self.build_dir().join(target).join(format!("{}{}", b,
                                                   os::consts::EXE_SUFFIX))
    }

    pub fn build_dir(&self) -> Path {
        self.root.join("target")
    }

    pub fn process<T: ToCStr>(&self, program: T) -> ProcessBuilder {
        process(program)
            .unwrap()
            .cwd(self.root())
            .env("HOME", Some(paths::home().display().to_string().as_slice()))
    }

    pub fn cargo_process(&self, cmd: &str) -> ProcessBuilder {
        self.build();
        self.process(cargo_dir().join("cargo")).arg(cmd)
    }

    pub fn file<B: BytesContainer, S: Str>(mut self, path: B,
                                           body: S) -> ProjectBuilder {
        self.files.push(FileBuilder::new(self.root.join(path), body.as_slice()));
        self
    }

    pub fn symlink<T: BytesContainer>(mut self, dst: T,
                                      src: T) -> ProjectBuilder {
        self.symlinks.push(SymlinkBuilder::new(self.root.join(dst),
                                               self.root.join(src)));
        self
    }

    // TODO: return something different than a ProjectBuilder
    pub fn build(&self) -> &ProjectBuilder {
        match self.build_with_result() {
            Err(e) => panic!(e),
            _ => return self
        }
    }

    pub fn build_with_result(&self) -> Result<(), String> {
        // First, clean the directory if it already exists
        try!(self.rm_root());

        // Create the empty directory
        try!(mkdir_recursive(&self.root));

        for file in self.files.iter() {
            try!(file.mk());
        }

        for symlink in self.symlinks.iter() {
            try!(symlink.mk());
        }

        Ok(())
    }

    fn rm_root(&self) -> Result<(), String> {
        if self.root.exists() {
            rmdir_recursive(&self.root)
        } else {
            Ok(())
        }
    }
}

// Generates a project layout
pub fn project(name: &str) -> ProjectBuilder {
    ProjectBuilder::new(name, paths::root().join(name))
}

// === Helpers ===

pub fn mkdir_recursive(path: &Path) -> Result<(), String> {
    fs::mkdir_recursive(path, io::USER_DIR)
        .with_err_msg(format!("could not create directory; path={}",
                              path.display()))
}

pub fn rmdir_recursive(path: &Path) -> Result<(), String> {
    path.rm_rf()
        .with_err_msg(format!("could not rm directory; path={}",
                              path.display()))
}

pub fn main_file<T: Str>(println: T, deps: &[&str]) -> String {
    let mut buf = String::new();

    for dep in deps.iter() {
        buf.push_str(format!("extern crate {};\n", dep).as_slice());
    }

    buf.push_str("fn main() { println!(");
    buf.push_str(println.as_slice());
    buf.push_str("); }\n");

    buf.to_string()
}

trait ErrMsg<T> {
    fn with_err_msg(self, val: String) -> Result<T, String>;
}

impl<T, E: Show> ErrMsg<T> for Result<T, E> {
    fn with_err_msg(self, val: String) -> Result<T, String> {
        match self {
            Ok(val) => Ok(val),
            Err(err) => Err(format!("{}; original={}", val, err))
        }
    }
}

// Path to cargo executables
pub fn cargo_dir() -> Path {
    os::getenv("CARGO_BIN_PATH").map(Path::new)
        .or_else(|| os::self_exe_path())
        .unwrap_or_else(|| {
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
struct Execs {
    expect_stdout: Option<String>,
    expect_stdin: Option<String>,
    expect_stderr: Option<String>,
    expect_exit_code: Option<int>
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

    pub fn with_status(mut self, expected: int) -> Execs {
        self.expect_exit_code = Some(expected);
        self
    }

    fn match_output(&self, actual: &ProcessOutput) -> ham::MatchResult {
        self.match_status(actual)
            .and(self.match_stdout(actual))
            .and(self.match_stderr(actual))
    }

    fn match_status(&self, actual: &ProcessOutput) -> ham::MatchResult {
        match self.expect_exit_code {
            None => ham::success(),
            Some(code) => {
                ham::expect(
                    actual.status.matches_exit_status(code),
                    format!("exited with {}\n--- stdout\n{}\n--- stderr\n{}",
                            actual.status,
                            String::from_utf8_lossy(actual.output.as_slice()),
                            String::from_utf8_lossy(actual.error.as_slice())))
            }
        }
    }

    fn match_stdout(&self, actual: &ProcessOutput) -> ham::MatchResult {
        self.match_std(self.expect_stdout.as_ref(), actual.output.as_slice(),
                       "stdout", actual.error.as_slice())
    }

    fn match_stderr(&self, actual: &ProcessOutput) -> ham::MatchResult {
        self.match_std(self.expect_stderr.as_ref(), actual.error.as_slice(),
                       "stderr", actual.output.as_slice())
    }

    fn match_std(&self, expected: Option<&String>, actual: &[u8],
                 description: &str, extra: &[u8]) -> ham::MatchResult {
        match expected.map(|s| Str::as_slice(s)) {
            None => ham::success(),
            Some(out) => {
                let actual = match str::from_utf8(actual) {
                    Err(..) => return Err(format!("{} was not utf8 encoded",
                                               description)),
                    Ok(actual) => actual,
                };
                // Let's not deal with \r\n vs \n on windows...
                let actual = actual.replace("\r", "");
                let actual = actual.replace("\t", "<tab>");

                let a = actual.as_slice().lines();
                let e = out.lines();

                let diffs = zip_all(a, e).enumerate();
                let diffs = diffs.filter_map(|(i, (a,e))| {
                    match (a, e) {
                        (Some(a), Some(e)) => {
                            if lines_match(e.as_slice(), a.as_slice()) {
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
                });

                let diffs = diffs.collect::<Vec<String>>().connect("\n");

                ham::expect(diffs.len() == 0,
                            format!("differences:\n\
                                    {}\n\n\
                                    other output:\n\
                                    `{}`", diffs,
                                    String::from_utf8_lossy(extra)))
            }
        }
    }
}

fn lines_match(expected: &str, mut actual: &str) -> bool {
    for part in expected.split_str("[..]") {
        match actual.find_str(part) {
            Some(i) => actual = actual.slice_from(i + part.len()),
            None => {
                return false
            }
        }
    }
    actual.len() == 0 || expected.ends_with("[..]")
}

struct ZipAll<T, I1, I2> {
    first: I1,
    second: I2,
}

impl<T, I1: Iterator<Item=T>, I2: Iterator<Item=T>> Iterator for ZipAll<T, I1, I2> {
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

fn zip_all<T, I1: Iterator<Item=T>, I2: Iterator<Item=T>>(a: I1, b: I2) -> ZipAll<T, I1, I2> {
    ZipAll {
        first: a,
        second: b
    }
}

impl fmt::Show for Execs {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "execs")
    }
}

impl ham::Matcher<ProcessBuilder> for Execs {
    fn matches(&self, process: ProcessBuilder) -> ham::MatchResult {
        let res = process.exec_with_output();

        match res {
            Ok(out) => self.match_output(&out),
            Err(ProcessError { output: Some(ref out), .. }) => {
                self.match_output(out)
            }
            Err(e) => {
                let mut s = format!("could not exec process {}: {}", process, e);
                match e.cause() {
                    Some(cause) => s.push_str(format!("\ncaused by: {}",
                                                      cause.description()).as_slice()),
                    None => {}
                }
                Err(s)
            }
        }
    }
}

pub fn execs() -> Execs {
    Execs {
        expect_stdout: None,
        expect_stderr: None,
        expect_stdin: None,
        expect_exit_code: None
    }
}

#[derive(Clone)]
struct ShellWrites {
    expected: String
}

impl fmt::Show for ShellWrites {
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

pub fn shell_writes<T: Show>(string: T) -> ShellWrites {
    ShellWrites { expected: string.to_string() }
}

pub trait Tap {
    fn tap(mut self, callback: |&mut Self|) -> Self;
}

impl<T> Tap for T {
    fn tap(mut self, callback: |&mut T|) -> T {
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

pub fn path2url(p: Path) -> Url {
    Url::from_file_path(&p).unwrap()
}

pub static RUNNING:     &'static str = "     Running";
pub static COMPILING:   &'static str = "   Compiling";
pub static FRESH:       &'static str = "       Fresh";
pub static UPDATING:    &'static str = "    Updating";
pub static DOCTEST:     &'static str = "   Doc-tests";
pub static PACKAGING:   &'static str = "   Packaging";
pub static DOWNLOADING: &'static str = " Downloading";
pub static UPLOADING:   &'static str = "   Uploading";
pub static VERIFYING:   &'static str = "   Verifying";
pub static ARCHIVING:   &'static str = "   Archiving";
