// use std::io::fs::{mkdir_recursive,rmdir_recursive};
use std;
use std::io;
use std::io::fs;
use std::io::process::{ProcessOutput,ProcessExit};
use std::os;
use std::path::{Path,BytesContainer};
use std::str;
use std::vec::Vec;
use std::fmt::Show;
use ham = hamcrest;
use cargo::core::shell;
use cargo::util::{process,ProcessBuilder,CargoError};
use cargo::util::result::ProcessError;

static CARGO_INTEGRATION_TEST_DIR : &'static str = "cargo-integration-tests";

/*
 *
 * ===== Builders =====
 *
 */

#[deriving(Eq,Clone)]
struct FileBuilder {
    path: Path,
    body: String
}

impl FileBuilder {
    pub fn new(path: Path, body: &str) -> FileBuilder {
        FileBuilder { path: path, body: body.to_owned() }
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

#[deriving(Eq,Clone)]
struct ProjectBuilder {
    name: String,
    root: Path,
    files: Vec<FileBuilder>
}

impl ProjectBuilder {
    pub fn new(name: &str, root: Path) -> ProjectBuilder {
        ProjectBuilder {
            name: name.to_owned(),
            root: root,
            files: vec!()
        }
    }

    pub fn root(&self) -> Path {
      self.root.clone()
    }

    pub fn cargo_process(&self, program: &str) -> ProcessBuilder {
        self.build();

        process(program)
            .cwd(self.root())
            .extra_path(cargo_dir())
    }

    pub fn file<B: BytesContainer>(mut self, path: B, body: &str) -> ProjectBuilder {
        self.files.push(FileBuilder::new(self.root.join(path), body));
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
    ProjectBuilder::new(name, os::tmpdir().join(CARGO_INTEGRATION_TEST_DIR))
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

/*
 *
 * ===== Matchers =====
 *
 */

#[deriving(Clone,Eq)]
struct Execs {
    expect_stdout: Option<String>,
    expect_stdin: Option<String>,
    expect_stderr: Option<String>,
    expect_exit_code: Option<int>
}

impl Execs {

  pub fn with_stdout(mut ~self, expected: &str) -> Box<Execs> {
    self.expect_stdout = Some(expected.to_owned());
    self
  }

  pub fn with_stderr(mut ~self, expected: &str) -> Box<Execs> {
      self.expect_stderr = Some(expected.to_owned());
      self
  }

  pub fn with_status(mut ~self, expected: int) -> Box<Execs> {
       self.expect_exit_code = Some(expected);
       self
  }

  fn match_output(&self, actual: &ProcessOutput) -> ham::MatchResult {
    self.match_status(actual.status)
      .and(self.match_stdout(&actual.output))
      .and(self.match_stderr(&actual.error))
  }

  fn match_status(&self, actual: ProcessExit) -> ham::MatchResult {
    match self.expect_exit_code {
      None => ham::success(),
      Some(code) => {
        ham::expect(
          actual.matches_exit_status(code),
          format!("exited with {}", actual))
      }
    }
  }

  fn match_stdout(&self, actual: &Vec<u8>) -> ham::MatchResult {
      self.match_std(&self.expect_stdout, actual, "stdout")
  }

  fn match_stderr(&self, actual: &Vec<u8>) -> ham::MatchResult {
      self.match_std(&self.expect_stderr, actual, "stderr")
  }

  fn match_std(&self, expected: &Option<String>, actual: &Vec<u8>, description: &str) -> ham::MatchResult {
    match expected.as_ref().map(|s| s.as_slice()) {
      None => ham::success(),
      Some(out) => {
        match str::from_utf8(actual.as_slice()) {
          None => Err(format!("{} was not utf8 encoded", description)),
          Some(actual) => {
            ham::expect(actual == out, format!("{} was `{}`", description, actual))
          }
        }
      }
    }
  }
}

impl ham::SelfDescribing for Execs {
  fn describe(&self) -> String {
    "execs".to_owned()
  }
}

impl ham::Matcher<ProcessBuilder> for Execs {
  fn matches(&self, process: ProcessBuilder) -> ham::MatchResult {
    let res = process.exec_with_output();

    match res {
      Ok(out) => self.match_output(&out),
      Err(CargoError { kind: ProcessError(_, ref out), .. }) => self.match_output(out.get_ref()),
      Err(_) => Err(format!("could not exec process {}", process))
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

#[deriving(Clone,Eq)]
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
