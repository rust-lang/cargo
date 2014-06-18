// use std::io::fs::{mkdir_recursive,rmdir_recursive};
use std;
use std::io;
use std::io::fs;
use std::io::process::{ProcessOutput};
use std::os;
use std::path::{Path,BytesContainer};
use std::str;
use std::vec::Vec;
use std::fmt::Show;
use ham = hamcrest;
use cargo::core::shell;
use cargo::util::{process,ProcessBuilder};
use cargo::util::ProcessError;

pub mod paths;

/*
 *
 * ===== Builders =====
 *
 */

#[deriving(PartialEq,Clone)]
struct FileBuilder {
    path: Path,
    body: String
}

impl FileBuilder {
    pub fn new(path: Path, body: &str) -> FileBuilder {
        FileBuilder { path: path, body: body.to_str() }
    }

    fn mk(&self) -> Result<(), String> {
        try!(mkdir_recursive(&self.dirname()));

        let mut file = try!(
            fs::File::create(&self.path)
                .with_err_msg(format!("Could not create file; path={}", self.path.display())));

        file.write_str(self.body.as_slice())
            .with_err_msg(format!("Could not write to file; path={}", self.path.display()))
    }

    fn dirname(&self) -> Path {
        Path::new(self.path.dirname())
    }
}

#[deriving(PartialEq,Clone)]
pub struct ProjectBuilder {
    name: String,
    root: Path,
    files: Vec<FileBuilder>
}

impl ProjectBuilder {
    pub fn new(name: &str, root: Path) -> ProjectBuilder {
        ProjectBuilder {
            name: name.to_str(),
            root: root,
            files: vec!()
        }
    }

    pub fn root(&self) -> Path {
      self.root.clone()
    }

    pub fn process(&self, program: &str) -> ProcessBuilder {
        process(program)
            .cwd(self.root())
            .env("HOME", Some(paths::home().display().to_str().as_slice()))
    }

    pub fn cargo_process(&self, program: &str) -> ProcessBuilder {
        self.build();
        self.process(program)
            .extra_path(cargo_dir())
    }

    pub fn file<B: BytesContainer, S: Str>(mut self, path: B, body: S) -> ProjectBuilder {
        self.files.push(FileBuilder::new(self.root.join(path), body.as_slice()));
        self
    }

    // TODO: return something different than a ProjectBuilder
    pub fn build<'a>(&'a self) -> &'a ProjectBuilder {
        match self.build_with_result() {
            Err(e) => fail!(e),
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

        Ok(())
    }

    fn rm_root(&self) -> Result<(), String> {
        if self.root.exists() {
            rmdir_recursive(&self.root)
        }
        else {
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
    fs::mkdir_recursive(path, io::UserDir)
        .with_err_msg(format!("could not create directory; path={}", path.display()))
}

pub fn rmdir_recursive(path: &Path) -> Result<(), String> {
    fs::rmdir_recursive(path)
        .with_err_msg(format!("could not rm directory; path={}", path.display()))
}

pub fn main_file<T: Str>(println: T, deps: &[&str]) -> String {
    let mut buf = String::new();

    for dep in deps.iter() {
        buf.push_str(format!("extern crate {};\n", dep).as_slice());
    }

    buf.push_str("fn main() { println!(");
    buf.push_str(println.as_slice());
    buf.push_str("); }\n");

    buf.to_str()
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
    os::getenv("CARGO_BIN_PATH")
        .map(|s| Path::new(s))
        .unwrap_or_else(|| fail!("CARGO_BIN_PATH wasn't set. Cannot continue running test"))
}

/// Returns an absolute path in the filesystem that `path` points to. The
/// returned path does not contain any symlinks in its hierarchy.
/*
 *
 * ===== Matchers =====
 *
 */

#[deriving(Clone)]
struct Execs {
    expect_stdout: Option<String>,
    expect_stdin: Option<String>,
    expect_stderr: Option<String>,
    expect_exit_code: Option<int>
}

impl Execs {

  pub fn with_stdout<S: ToStr>(mut ~self, expected: S) -> Box<Execs> {
    self.expect_stdout = Some(expected.to_str());
    self
  }

  pub fn with_stderr<S: ToStr>(mut ~self, expected: S) -> Box<Execs> {
      self.expect_stderr = Some(expected.to_str());
      self
  }

  pub fn with_status(mut ~self, expected: int) -> Box<Execs> {
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
                  str::from_utf8(actual.output.as_slice()),
                  str::from_utf8(actual.error.as_slice())))
      }
    }
  }

  fn match_stdout(&self, actual: &ProcessOutput) -> ham::MatchResult {
      self.match_std(self.expect_stdout.as_ref(), actual.output.as_slice(), "stdout", actual.error.as_slice())
  }

  fn match_stderr(&self, actual: &ProcessOutput) -> ham::MatchResult {
      self.match_std(self.expect_stderr.as_ref(), actual.error.as_slice(), "stderr", actual.output.as_slice())
  }

  fn match_std(&self, expected: Option<&String>, actual: &[u8], description: &str, extra: &[u8]) -> ham::MatchResult {
    match expected.as_ref().map(|s| s.as_slice()) {
      None => ham::success(),
      Some(out) => {
        match str::from_utf8(actual) {
          None => Err(format!("{} was not utf8 encoded", description)),
          Some(actual) => {
            ham::expect(actual == out, format!("{} was:\n`{}`\n\nexpected:\n`{}`\n\nother output:\n`{}`", description, actual, out, str::from_utf8_lossy(extra)))
          }
        }
      }
    }
  }
}

impl ham::SelfDescribing for Execs {
  fn describe(&self) -> String {
    "execs".to_str()
  }
}

impl ham::Matcher<ProcessBuilder> for Execs {
  fn matches(&self, process: ProcessBuilder) -> ham::MatchResult {
    let res = process.exec_with_output();

    match res {
      Ok(out) => self.match_output(&out),
      Err(ProcessError { output: Some(ref out), .. }) => self.match_output(out),
      Err(e) => Err(format!("could not exec process {}: {}", process, e))
    }
  }
}

pub fn execs() -> Box<Execs> {
    box Execs {
        expect_stdout: None,
        expect_stderr: None,
        expect_stdin: None,
        expect_exit_code: None
    }
}

#[deriving(Clone)]
struct ShellWrites {
    expected: String
}

impl ham::SelfDescribing for ShellWrites {
    fn describe(&self) -> String {
        format!("`{}` written to the shell", self.expected)
    }
}

impl<'a> ham::Matcher<&'a mut shell::Shell<std::io::MemWriter>> for ShellWrites {
    fn matches(&self, actual: &mut shell::Shell<std::io::MemWriter>) -> ham::MatchResult {
        use term::Terminal;

        let actual = std::str::from_utf8_lossy(actual.get_ref().get_ref()).to_str();
        ham::expect(actual == self.expected, actual)
    }
}

pub fn shell_writes<T: Show>(string: T) -> Box<ShellWrites> {
    box ShellWrites { expected: string.to_str() }
}

pub trait ResultTest<T,E> {
    fn assert(self) -> T;
}

impl<T,E: Show> ResultTest<T,E> for Result<T,E> {
    fn assert(self) -> T {
        match self {
            Ok(val) => val,
            Err(err) => fail!("Result was error: {}", err)
        }
    }
}

impl<T> ResultTest<T,()> for Option<T> {
    fn assert(self) -> T {
        match self {
            Some(val) => val,
            None => fail!("Option was None")
        }
    }
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
