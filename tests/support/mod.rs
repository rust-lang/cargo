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

use url::Url;
use hamcrest as ham;
use cargo::util::ProcessBuilder;
use cargo::util::ProcessError;
use cargo::util::process;

use support::paths::CargoPathExt;

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

    fn mk(&self) -> Result<(), String> {
        try!(mkdir_recursive(&self.dirname()));

        let mut file = try!(
            fs::File::create(&self.path)
                .with_err_msg(format!("Could not create file; path={}",
                                      self.path.display())));

        file.write_all(self.body.as_bytes())
            .with_err_msg(format!("Could not write to file; path={}",
                                  self.path.display()))
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
    fn mk(&self) -> Result<(), String> {
        try!(mkdir_recursive(&self.dirname()));

        os::unix::fs::symlink(&self.dst, &self.src)
            .with_err_msg(format!("Could not create symlink; dst={} src={}",
                                   self.dst.display(), self.src.display()))
    }

    #[cfg(windows)]
    fn mk(&self) -> Result<(), String> {
        try!(mkdir_recursive(&self.dirname()));

        os::windows::fs::symlink_file(&self.dst, &self.src)
            .with_err_msg(format!("Could not create symlink; dst={} src={}",
                                   self.dst.display(), self.src.display()))
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
            files: vec!(),
            symlinks: vec!()
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
        let mut p = process(program);
        p.cwd(&self.root())
         .env("HOME", &paths::home())
         .env_remove("CARGO_HOME")  // make sure we don't pick up an outer one
         .env_remove("CARGO_TARGET_DIR") // we assume 'target'
         .env_remove("MSYSTEM");    // assume cmd.exe everywhere on windows
        return p;
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
        if self.root.c_exists() {
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
    fs::create_dir_all(path)
        .with_err_msg(format!("could not create directory; path={}",
                              path.display()))
}

pub fn rmdir_recursive(path: &Path) -> Result<(), String> {
    path.rm_rf()
        .with_err_msg(format!("could not rm directory; path={}",
                              path.display()))
}

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
    expect_stdout_contains: Vec<String>
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
        Ok(())
    }

    fn match_stderr(&self, actual: &Output) -> ham::MatchResult {
        self.match_std(self.expect_stderr.as_ref(), &actual.stderr,
                       "stderr", &actual.stdout, false)
    }

    #[allow(deprecated)] // connect => join in 1.3
    fn match_std(&self, expected: Option<&String>, actual: &[u8],
                 description: &str, extra: &[u8],
                 partial: bool) -> ham::MatchResult {
        let out = match expected {
            Some(out) => out,
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
        ham::expect(diffs.len() == 0,
                    format!("differences:\n\
                            {}\n\n\
                            other output:\n\
                            `{}`", diffs.connect("\n"),
                            String::from_utf8_lossy(extra)))

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
    for part in expected.split("[..]") {
        match actual.find(part) {
            Some(i) => actual = &actual[i + part.len()..],
            None => {
                return false
            }
        }
    }
    actual.len() == 0 || expected.ends_with("[..]")
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

pub fn execs() -> Execs {
    Execs {
        expect_stdout: None,
        expect_stderr: None,
        expect_stdin: None,
        expect_exit_code: None,
        expect_stdout_contains: vec![]
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

pub static RUNNING:     &'static str = "     Running";
pub static COMPILING:   &'static str = "   Compiling";
pub static DOCUMENTING: &'static str = " Documenting";
pub static FRESH:       &'static str = "       Fresh";
pub static UPDATING:    &'static str = "    Updating";
pub static ADDING:      &'static str = "      Adding";
pub static REMOVING:    &'static str = "    Removing";
pub static DOCTEST:     &'static str = "   Doc-tests";
pub static PACKAGING:   &'static str = "   Packaging";
pub static DOWNLOADING: &'static str = " Downloading";
pub static UPLOADING:   &'static str = "   Uploading";
pub static VERIFYING:   &'static str = "   Verifying";
pub static ARCHIVING:   &'static str = "   Archiving";
pub static INSTALLING:  &'static str = "  Installing";
