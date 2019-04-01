/*
# Introduction to `support`.

Cargo has a wide variety of integration tests that execute the `cargo` binary
and verify its behavior. The `support` module contains many helpers to make
this process easy.

The general form of a test involves creating a "project", running cargo, and
checking the result. Projects are created with the `ProjectBuilder` where you
specify some files to create. The general form looks like this:

```
let p = project()
    .file("src/main.rs", r#"fn main() { println!("hi!"); }"#)
    .build();
```

If you do not specify a `Cargo.toml` manifest using `file()`, one is
automatically created with a project name of `foo` using `basic_manifest()`.

To run cargo, call the `cargo` method and make assertions on the execution:

```
p.cargo("run --bin foo")
    .with_stderr(
        "\
[COMPILING] foo [..]
[FINISHED] [..]
[RUNNING] `target/debug/foo`
",
    )
    .with_stdout("hi!")
    .run();
```

The project creates a mini sandbox under the "cargo integration test"
directory with each test getting a separate directory such as
`/path/to/cargo/target/cit/t123/`. Each project appears as a separate
directory. There is also an empty `home` directory created that will be used
as a home directory instead of your normal home directory.

See `support::lines_match` for an explanation of the string pattern matching.

Browse the `pub` functions in the `support` module for a variety of other
helpful utilities.

## Testing Nightly Features

If you are testing a Cargo feature that only works on "nightly" cargo, then
you need to call `masquerade_as_nightly_cargo` on the process builder like
this:

```
p.cargo("build").masquerade_as_nightly_cargo()
```

If you are testing a feature that only works on *nightly rustc* (such as
benchmarks), then you should exit the test if it is not running with nightly
rust, like this:

```
if !is_nightly() {
    // Add a comment here explaining why this is necessary.
    return;
}
```

## Platform-specific Notes

When checking output, use `/` for paths even on Windows: the actual output
of `\` on Windows will be replaced with `/`.

Be careful when executing binaries on Windows. You should not rename, delete,
or overwrite a binary immediately after running it. Under some conditions
Windows will fail with errors like "directory not empty" or "failed to remove"
or "access is denied".

## Specifying Dependencies

You should not write any tests that use the network such as contacting
crates.io. Typically, simple path dependencies are the easiest way to add a
dependency. Example:

```
let p = project()
    .file("Cargo.toml", r#"
        [package]
        name = "foo"
        version = "1.0.0"

        [dependencies]
        bar = {path = "bar"}
    "#)
    .file("src/lib.rs", "extern crate bar;")
    .file("bar/Cargo.toml", &basic_manifest("bar", "1.0.0"))
    .file("bar/src/lib.rs", "")
    .build();
```

If you need to test with registry dependencies, see
`support::registry::Package` for creating packages you can depend on.

If you need to test git dependencies, see `support::git` to create a git
dependency.

*/

use std::env;
use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::io::prelude::*;
use std::os;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::str;
use std::time::{self, Duration};
use std::usize;

use cargo;
use cargo::util::{CargoResult, ProcessBuilder, ProcessError, Rustc};
use filetime;
use serde_json::{self, Value};
use url::Url;

use self::paths::CargoPathExt;

macro_rules! t {
    ($e:expr) => {
        match $e {
            Ok(e) => e,
            Err(e) => panic!("{} failed with {}", stringify!($e), e),
        }
    };
}

pub mod cross_compile;
pub mod git;
pub mod paths;
pub mod publish;
pub mod registry;
#[macro_use]
pub mod resolver;

/*
 *
 * ===== Builders =====
 *
 */

#[derive(PartialEq, Clone)]
struct FileBuilder {
    path: PathBuf,
    body: String,
}

impl FileBuilder {
    pub fn new(path: PathBuf, body: &str) -> FileBuilder {
        FileBuilder {
            path,
            body: body.to_string(),
        }
    }

    fn mk(&self) {
        self.dirname().mkdir_p();

        let mut file = fs::File::create(&self.path)
            .unwrap_or_else(|e| panic!("could not create file {}: {}", self.path.display(), e));

        t!(file.write_all(self.body.as_bytes()));
    }

    fn dirname(&self) -> &Path {
        self.path.parent().unwrap()
    }
}

#[derive(PartialEq, Clone)]
struct SymlinkBuilder {
    dst: PathBuf,
    src: PathBuf,
}

impl SymlinkBuilder {
    pub fn new(dst: PathBuf, src: PathBuf) -> SymlinkBuilder {
        SymlinkBuilder { dst, src }
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

pub struct Project {
    root: PathBuf,
}

#[must_use]
pub struct ProjectBuilder {
    root: Project,
    files: Vec<FileBuilder>,
    symlinks: Vec<SymlinkBuilder>,
    no_manifest: bool,
}

impl ProjectBuilder {
    /// Root of the project, ex: `/path/to/cargo/target/cit/t0/foo`
    pub fn root(&self) -> PathBuf {
        self.root.root()
    }

    /// Project's debug dir, ex: `/path/to/cargo/target/cit/t0/foo/target/debug`
    pub fn target_debug_dir(&self) -> PathBuf {
        self.root.target_debug_dir()
    }

    pub fn new(root: PathBuf) -> ProjectBuilder {
        ProjectBuilder {
            root: Project { root },
            files: vec![],
            symlinks: vec![],
            no_manifest: false,
        }
    }

    pub fn at<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.root = Project {
            root: paths::root().join(path),
        };
        self
    }

    /// Adds a file to the project.
    pub fn file<B: AsRef<Path>>(mut self, path: B, body: &str) -> Self {
        self._file(path.as_ref(), body);
        self
    }

    fn _file(&mut self, path: &Path, body: &str) {
        self.files
            .push(FileBuilder::new(self.root.root().join(path), body));
    }

    /// Adds a symlink to the project.
    pub fn symlink<T: AsRef<Path>>(mut self, dst: T, src: T) -> Self {
        self.symlinks.push(SymlinkBuilder::new(
            self.root.root().join(dst),
            self.root.root().join(src),
        ));
        self
    }

    pub fn no_manifest(mut self) -> Self {
        self.no_manifest = true;
        self
    }

    /// Creates the project.
    pub fn build(mut self) -> Project {
        // First, clean the directory if it already exists
        self.rm_root();

        // Create the empty directory
        self.root.root().mkdir_p();

        let manifest_path = self.root.root().join("Cargo.toml");
        if !self.no_manifest && self.files.iter().all(|fb| fb.path != manifest_path) {
            self._file(Path::new("Cargo.toml"), &basic_manifest("foo", "0.0.1"))
        }

        let past = time::SystemTime::now() - Duration::new(1, 0);
        let ftime = filetime::FileTime::from_system_time(past);

        for file in self.files.iter() {
            file.mk();
            if is_coarse_mtime() {
                // Place the entire project 1 second in the past to ensure
                // that if cargo is called multiple times, the 2nd call will
                // see targets as "fresh". Without this, if cargo finishes in
                // under 1 second, the second call will see the mtime of
                // source == mtime of output and consider it dirty.
                filetime::set_file_times(&file.path, ftime, ftime).unwrap();
            }
        }

        for symlink in self.symlinks.iter() {
            symlink.mk();
        }

        let ProjectBuilder { root, .. } = self;
        root
    }

    fn rm_root(&self) {
        self.root.root().rm_rf()
    }
}

impl Project {
    /// Root of the project, ex: `/path/to/cargo/target/cit/t0/foo`
    pub fn root(&self) -> PathBuf {
        self.root.clone()
    }

    /// Project's target dir, ex: `/path/to/cargo/target/cit/t0/foo/target`
    pub fn build_dir(&self) -> PathBuf {
        self.root().join("target")
    }

    /// Project's debug dir, ex: `/path/to/cargo/target/cit/t0/foo/target/debug`
    pub fn target_debug_dir(&self) -> PathBuf {
        self.build_dir().join("debug")
    }

    /// File url for root, ex: `file:///path/to/cargo/target/cit/t0/foo`
    pub fn url(&self) -> Url {
        path2url(self.root())
    }

    /// Path to an example built as a library.
    /// `kind` should be one of: "lib", "rlib", "staticlib", "dylib", "proc-macro"
    /// ex: `/path/to/cargo/target/cit/t0/foo/target/debug/examples/libex.rlib`
    pub fn example_lib(&self, name: &str, kind: &str) -> PathBuf {
        let prefix = Project::get_lib_prefix(kind);

        let extension = Project::get_lib_extension(kind);

        let lib_file_name = format!("{}{}.{}", prefix, name, extension);

        self.target_debug_dir()
            .join("examples")
            .join(&lib_file_name)
    }

    /// Path to a debug binary.
    /// ex: `/path/to/cargo/target/cit/t0/foo/target/debug/foo`
    pub fn bin(&self, b: &str) -> PathBuf {
        self.build_dir()
            .join("debug")
            .join(&format!("{}{}", b, env::consts::EXE_SUFFIX))
    }

    /// Path to a release binary.
    /// ex: `/path/to/cargo/target/cit/t0/foo/target/release/foo`
    pub fn release_bin(&self, b: &str) -> PathBuf {
        self.build_dir()
            .join("release")
            .join(&format!("{}{}", b, env::consts::EXE_SUFFIX))
    }

    /// Path to a debug binary for a specific target triple.
    /// ex: `/path/to/cargo/target/cit/t0/foo/target/i686-apple-darwin/debug/foo`
    pub fn target_bin(&self, target: &str, b: &str) -> PathBuf {
        self.build_dir().join(target).join("debug").join(&format!(
            "{}{}",
            b,
            env::consts::EXE_SUFFIX
        ))
    }

    /// Returns an iterator of paths matching the glob pattern, which is
    /// relative to the project root.
    pub fn glob<P: AsRef<Path>>(&self, pattern: P) -> glob::Paths {
        let pattern = self.root().join(pattern);
        glob::glob(pattern.to_str().expect("failed to convert pattern to str"))
            .expect("failed to glob")
    }

    /// Changes the contents of an existing file.
    pub fn change_file(&self, path: &str, body: &str) {
        FileBuilder::new(self.root().join(path), body).mk()
    }

    /// Creates a `ProcessBuilder` to run a program in the project
    /// and wrap it in an Execs to assert on the execution.
    /// Example:
    ///         p.process(&p.bin("foo"))
    ///             .with_stdout("bar\n")
    ///             .run();
    pub fn process<T: AsRef<OsStr>>(&self, program: T) -> Execs {
        let mut p = crate::support::process(program);
        p.cwd(self.root());
        execs().with_process_builder(p)
    }

    /// Creates a `ProcessBuilder` to run cargo.
    /// Arguments can be separated by spaces.
    /// Example:
    ///     p.cargo("build --bin foo").run();
    pub fn cargo(&self, cmd: &str) -> Execs {
        let mut execs = self.process(&cargo_exe());
        if let Some(ref mut p) = execs.process_builder {
            split_and_add_args(p, cmd);
        }
        execs
    }

    /// Safely run a process after `cargo build`.
    ///
    /// Windows has a problem where a process cannot be reliably
    /// be replaced, removed, or renamed immediately after executing it.
    /// The action may fail (with errors like Access is denied), or
    /// it may succeed, but future attempts to use the same filename
    /// will fail with "Already Exists".
    ///
    /// If you have a test that needs to do `cargo run` multiple
    /// times, you should instead use `cargo build` and use this
    /// method to run the executable. Each time you call this,
    /// use a new name for `dst`.
    /// See rust-lang/cargo#5481.
    pub fn rename_run(&self, src: &str, dst: &str) -> Execs {
        let src = self.bin(src);
        let dst = self.bin(dst);
        fs::rename(&src, &dst)
            .unwrap_or_else(|e| panic!("Failed to rename `{:?}` to `{:?}`: {}", src, dst, e));
        self.process(dst)
    }

    /// Returns the contents of `Cargo.lock`.
    pub fn read_lockfile(&self) -> String {
        self.read_file("Cargo.lock")
    }

    /// Returns the contents of a path in the project root
    pub fn read_file(&self, path: &str) -> String {
        let mut buffer = String::new();
        fs::File::open(self.root().join(path))
            .unwrap()
            .read_to_string(&mut buffer)
            .unwrap();
        buffer
    }

    /// Modifies `Cargo.toml` to remove all commented lines.
    pub fn uncomment_root_manifest(&self) {
        let mut contents = String::new();
        fs::File::open(self.root().join("Cargo.toml"))
            .unwrap()
            .read_to_string(&mut contents)
            .unwrap();
        fs::File::create(self.root().join("Cargo.toml"))
            .unwrap()
            .write_all(contents.replace("#", "").as_bytes())
            .unwrap();
    }

    fn get_lib_prefix(kind: &str) -> &str {
        match kind {
            "lib" | "rlib" => "lib",
            "staticlib" | "dylib" | "proc-macro" => {
                if cfg!(windows) {
                    ""
                } else {
                    "lib"
                }
            }
            _ => unreachable!(),
        }
    }

    fn get_lib_extension(kind: &str) -> &str {
        match kind {
            "lib" | "rlib" => "rlib",
            "staticlib" => {
                if cfg!(windows) {
                    "lib"
                } else {
                    "a"
                }
            }
            "dylib" | "proc-macro" => {
                if cfg!(windows) {
                    "dll"
                } else if cfg!(target_os = "macos") {
                    "dylib"
                } else {
                    "so"
                }
            }
            _ => unreachable!(),
        }
    }
}

// Generates a project layout
pub fn project() -> ProjectBuilder {
    ProjectBuilder::new(paths::root().join("foo"))
}

// Generates a project layout inside our fake home dir
pub fn project_in_home(name: &str) -> ProjectBuilder {
    ProjectBuilder::new(paths::home().join(name))
}

// === Helpers ===

pub fn main_file(println: &str, deps: &[&str]) -> String {
    let mut buf = String::new();

    for dep in deps.iter() {
        buf.push_str(&format!("extern crate {};\n", dep));
    }

    buf.push_str("fn main() { println!(");
    buf.push_str(println);
    buf.push_str("); }\n");

    buf
}

trait ErrMsg<T> {
    fn with_err_msg(self, val: String) -> Result<T, String>;
}

impl<T, E: fmt::Display> ErrMsg<T> for Result<T, E> {
    fn with_err_msg(self, val: String) -> Result<T, String> {
        match self {
            Ok(val) => Ok(val),
            Err(err) => Err(format!("{}; original={}", val, err)),
        }
    }
}

// Path to cargo executables
pub fn cargo_dir() -> PathBuf {
    env::var_os("CARGO_BIN_PATH")
        .map(PathBuf::from)
        .or_else(|| {
            env::current_exe().ok().map(|mut path| {
                path.pop();
                if path.ends_with("deps") {
                    path.pop();
                }
                path
            })
        })
        .unwrap_or_else(|| panic!("CARGO_BIN_PATH wasn't set. Cannot continue running test"))
}

pub fn cargo_exe() -> PathBuf {
    cargo_dir().join(format!("cargo{}", env::consts::EXE_SUFFIX))
}

/*
 *
 * ===== Matchers =====
 *
 */

pub type MatchResult = Result<(), String>;

#[must_use]
#[derive(Clone)]
pub struct Execs {
    ran: bool,
    process_builder: Option<ProcessBuilder>,
    expect_stdout: Option<String>,
    expect_stdin: Option<String>,
    expect_stderr: Option<String>,
    expect_exit_code: Option<i32>,
    expect_stdout_contains: Vec<String>,
    expect_stderr_contains: Vec<String>,
    expect_either_contains: Vec<String>,
    expect_stdout_contains_n: Vec<(String, usize)>,
    expect_stdout_not_contains: Vec<String>,
    expect_stderr_not_contains: Vec<String>,
    expect_stderr_unordered: Vec<String>,
    expect_neither_contains: Vec<String>,
    expect_stderr_with_without: Vec<(Vec<String>, Vec<String>)>,
    expect_json: Option<Vec<Value>>,
    expect_json_contains_unordered: Vec<Value>,
    stream_output: bool,
}

impl Execs {
    pub fn with_process_builder(mut self, p: ProcessBuilder) -> Execs {
        self.process_builder = Some(p);
        self
    }

    /// Verifies that stdout is equal to the given lines.
    /// See `lines_match` for supported patterns.
    pub fn with_stdout<S: ToString>(&mut self, expected: S) -> &mut Self {
        self.expect_stdout = Some(expected.to_string());
        self
    }

    /// Verifies that stderr is equal to the given lines.
    /// See `lines_match` for supported patterns.
    pub fn with_stderr<S: ToString>(&mut self, expected: S) -> &mut Self {
        self.expect_stderr = Some(expected.to_string());
        self
    }

    /// Verifies the exit code from the process.
    ///
    /// This is not necessary if the expected exit code is `0`.
    pub fn with_status(&mut self, expected: i32) -> &mut Self {
        self.expect_exit_code = Some(expected);
        self
    }

    /// Removes exit code check for the process.
    ///
    /// By default, the expected exit code is `0`.
    pub fn without_status(&mut self) -> &mut Self {
        self.expect_exit_code = None;
        self
    }

    /// Verifies that stdout contains the given contiguous lines somewhere in
    /// its output.
    /// See `lines_match` for supported patterns.
    pub fn with_stdout_contains<S: ToString>(&mut self, expected: S) -> &mut Self {
        self.expect_stdout_contains.push(expected.to_string());
        self
    }

    /// Verifies that stderr contains the given contiguous lines somewhere in
    /// its output.
    /// See `lines_match` for supported patterns.
    pub fn with_stderr_contains<S: ToString>(&mut self, expected: S) -> &mut Self {
        self.expect_stderr_contains.push(expected.to_string());
        self
    }

    /// Verifies that either stdout or stderr contains the given contiguous
    /// lines somewhere in its output.
    /// See `lines_match` for supported patterns.
    pub fn with_either_contains<S: ToString>(&mut self, expected: S) -> &mut Self {
        self.expect_either_contains.push(expected.to_string());
        self
    }

    /// Verifies that stdout contains the given contiguous lines somewhere in
    /// its output, and should be repeated `number` times.
    /// See `lines_match` for supported patterns.
    pub fn with_stdout_contains_n<S: ToString>(&mut self, expected: S, number: usize) -> &mut Self {
        self.expect_stdout_contains_n
            .push((expected.to_string(), number));
        self
    }

    /// Verifies that stdout does not contain the given contiguous lines.
    /// See `lines_match` for supported patterns.
    /// See note on `with_stderr_does_not_contain`.
    pub fn with_stdout_does_not_contain<S: ToString>(&mut self, expected: S) -> &mut Self {
        self.expect_stdout_not_contains.push(expected.to_string());
        self
    }

    /// Verifies that stderr does not contain the given contiguous lines.
    /// See `lines_match` for supported patterns.
    ///
    /// Care should be taken when using this method because there is a
    /// limitless number of possible things that *won't* appear. A typo means
    /// your test will pass without verifying the correct behavior. If
    /// possible, write the test first so that it fails, and then implement
    /// your fix/feature to make it pass.
    pub fn with_stderr_does_not_contain<S: ToString>(&mut self, expected: S) -> &mut Self {
        self.expect_stderr_not_contains.push(expected.to_string());
        self
    }

    /// Verifies that all of the stderr output is equal to the given lines,
    /// ignoring the order of the lines.
    /// See `lines_match` for supported patterns.
    /// This is useful when checking the output of `cargo build -v` since
    /// the order of the output is not always deterministic.
    /// Recommend use `with_stderr_contains` instead unless you really want to
    /// check *every* line of output.
    ///
    /// Be careful when using patterns such as `[..]`, because you may end up
    /// with multiple lines that might match, and this is not smart enough to
    /// do anything like longest-match. For example, avoid something like:
    ///
    ///     [RUNNING] `rustc [..]
    ///     [RUNNING] `rustc --crate-name foo [..]
    ///
    /// This will randomly fail if the other crate name is `bar`, and the
    /// order changes.
    pub fn with_stderr_unordered<S: ToString>(&mut self, expected: S) -> &mut Self {
        self.expect_stderr_unordered.push(expected.to_string());
        self
    }

    /// Verify that a particular line appears in stderr with and without the
    /// given substrings. Exactly one line must match.
    ///
    /// The substrings are matched as `contains`. Example:
    ///
    /// ```no_run
    /// execs.with_stderr_line_without(
    ///     &[
    ///         "[RUNNING] `rustc --crate-name build_script_build",
    ///         "-C opt-level=3",
    ///     ],
    ///     &["-C debuginfo", "-C incremental"],
    /// )
    /// ```
    ///
    /// This will check that a build line includes `-C opt-level=3` but does
    /// not contain `-C debuginfo` or `-C incremental`.
    ///
    /// Be careful writing the `without` fragments, see note in
    /// `with_stderr_does_not_contain`.
    pub fn with_stderr_line_without<S: ToString>(
        &mut self,
        with: &[S],
        without: &[S],
    ) -> &mut Self {
        let with = with.iter().map(|s| s.to_string()).collect();
        let without = without.iter().map(|s| s.to_string()).collect();
        self.expect_stderr_with_without.push((with, without));
        self
    }

    /// Verifies the JSON output matches the given JSON.
    /// Typically used when testing cargo commands that emit JSON.
    /// Each separate JSON object should be separated by a blank line.
    /// Example:
    ///     assert_that(
    ///         p.cargo("metadata"),
    ///         execs().with_json(r#"
    ///             {"example": "abc"}
    ///
    ///             {"example": "def"}
    ///         "#)
    ///      );
    /// Objects should match in the order given.
    /// The order of arrays is ignored.
    /// Strings support patterns described in `lines_match`.
    /// Use `{...}` to match any object.
    pub fn with_json(&mut self, expected: &str) -> &mut Self {
        self.expect_json = Some(
            expected
                .split("\n\n")
                .map(|line| line.parse().expect("line to be a valid JSON value"))
                .collect(),
        );
        self
    }

    /// Verifies JSON output contains the given objects (in any order) somewhere
    /// in its output.
    ///
    /// CAUTION: Be very careful when using this. Make sure every object is
    /// unique (not a subset of one another). Also avoid using objects that
    /// could possibly match multiple output lines unless you're very sure of
    /// what you are doing.
    ///
    /// See `with_json` for more detail.
    pub fn with_json_contains_unordered(&mut self, expected: &str) -> &mut Self {
        self.expect_json_contains_unordered.extend(
            expected
                .split("\n\n")
                .map(|line| line.parse().expect("line to be a valid JSON value")),
        );
        self
    }

    /// Forward subordinate process stdout/stderr to the terminal.
    /// Useful for printf debugging of the tests.
    /// CAUTION: CI will fail if you leave this in your test!
    #[allow(unused)]
    pub fn stream(&mut self) -> &mut Self {
        self.stream_output = true;
        self
    }

    pub fn arg<T: AsRef<OsStr>>(&mut self, arg: T) -> &mut Self {
        if let Some(ref mut p) = self.process_builder {
            p.arg(arg);
        }
        self
    }

    pub fn cwd<T: AsRef<OsStr>>(&mut self, path: T) -> &mut Self {
        if let Some(ref mut p) = self.process_builder {
            if let Some(cwd) = p.get_cwd() {
                p.cwd(cwd.join(path.as_ref()));
            } else {
                p.cwd(path);
            }
        }
        self
    }

    pub fn env<T: AsRef<OsStr>>(&mut self, key: &str, val: T) -> &mut Self {
        if let Some(ref mut p) = self.process_builder {
            p.env(key, val);
        }
        self
    }

    pub fn env_remove(&mut self, key: &str) -> &mut Self {
        if let Some(ref mut p) = self.process_builder {
            p.env_remove(key);
        }
        self
    }

    pub fn exec_with_output(&mut self) -> CargoResult<Output> {
        self.ran = true;
        // TODO avoid unwrap
        let p = (&self.process_builder).clone().unwrap();
        p.exec_with_output()
    }

    pub fn build_command(&mut self) -> Command {
        self.ran = true;
        // TODO avoid unwrap
        let p = (&self.process_builder).clone().unwrap();
        p.build_command()
    }

    pub fn masquerade_as_nightly_cargo(&mut self) -> &mut Self {
        if let Some(ref mut p) = self.process_builder {
            p.masquerade_as_nightly_cargo();
        }
        self
    }

    pub fn run(&mut self) {
        self.ran = true;
        let p = (&self.process_builder).clone().unwrap();
        if let Err(e) = self.match_process(&p) {
            panic!("\nExpected: {:?}\n    but: {}", self, e)
        }
    }

    pub fn run_output(&mut self, output: &Output) {
        self.ran = true;
        if let Err(e) = self.match_output(output) {
            panic!("\nExpected: {:?}\n    but: {}", self, e)
        }
    }

    fn verify_checks_output(&self, output: &Output) {
        if self.expect_exit_code.unwrap_or(0) != 0
            && self.expect_stdout.is_none()
            && self.expect_stdin.is_none()
            && self.expect_stderr.is_none()
            && self.expect_stdout_contains.is_empty()
            && self.expect_stderr_contains.is_empty()
            && self.expect_either_contains.is_empty()
            && self.expect_stdout_contains_n.is_empty()
            && self.expect_stdout_not_contains.is_empty()
            && self.expect_stderr_not_contains.is_empty()
            && self.expect_stderr_unordered.is_empty()
            && self.expect_neither_contains.is_empty()
            && self.expect_stderr_with_without.is_empty()
            && self.expect_json.is_none()
            && self.expect_json_contains_unordered.is_empty()
        {
            panic!(
                "`with_status()` is used, but no output is checked.\n\
                 The test must check the output to ensure the correct error is triggered.\n\
                 --- stdout\n{}\n--- stderr\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
            );
        }
    }

    fn match_process(&self, process: &ProcessBuilder) -> MatchResult {
        println!("running {}", process);
        let res = if self.stream_output {
            if env::var("CI").is_ok() {
                panic!("`.stream()` is for local debugging")
            }
            process.exec_with_streaming(
                &mut |out| Ok(println!("{}", out)),
                &mut |err| Ok(eprintln!("{}", err)),
                true,
            )
        } else {
            process.exec_with_output()
        };

        match res {
            Ok(out) => self.match_output(&out),
            Err(e) => {
                let err = e.downcast_ref::<ProcessError>();
                if let Some(&ProcessError {
                    output: Some(ref out),
                    ..
                }) = err
                {
                    return self.match_output(out);
                }
                let mut s = format!("could not exec process {}: {}", process, e);
                for cause in e.iter_causes() {
                    s.push_str(&format!("\ncaused by: {}", cause));
                }
                Err(s)
            }
        }
    }

    fn match_output(&self, actual: &Output) -> MatchResult {
        self.verify_checks_output(actual);
        self.match_status(actual)
            .and(self.match_stdout(actual))
            .and(self.match_stderr(actual))
    }

    fn match_status(&self, actual: &Output) -> MatchResult {
        match self.expect_exit_code {
            None => Ok(()),
            Some(code) if actual.status.code() == Some(code) => Ok(()),
            Some(_) => Err(format!(
                "exited with {}\n--- stdout\n{}\n--- stderr\n{}",
                actual.status,
                String::from_utf8_lossy(&actual.stdout),
                String::from_utf8_lossy(&actual.stderr)
            )),
        }
    }

    fn match_stdout(&self, actual: &Output) -> MatchResult {
        self.match_std(
            self.expect_stdout.as_ref(),
            &actual.stdout,
            "stdout",
            &actual.stderr,
            MatchKind::Exact,
        )?;
        for expect in self.expect_stdout_contains.iter() {
            self.match_std(
                Some(expect),
                &actual.stdout,
                "stdout",
                &actual.stderr,
                MatchKind::Partial,
            )?;
        }
        for expect in self.expect_stderr_contains.iter() {
            self.match_std(
                Some(expect),
                &actual.stderr,
                "stderr",
                &actual.stdout,
                MatchKind::Partial,
            )?;
        }
        for &(ref expect, number) in self.expect_stdout_contains_n.iter() {
            self.match_std(
                Some(expect),
                &actual.stdout,
                "stdout",
                &actual.stderr,
                MatchKind::PartialN(number),
            )?;
        }
        for expect in self.expect_stdout_not_contains.iter() {
            self.match_std(
                Some(expect),
                &actual.stdout,
                "stdout",
                &actual.stderr,
                MatchKind::NotPresent,
            )?;
        }
        for expect in self.expect_stderr_not_contains.iter() {
            self.match_std(
                Some(expect),
                &actual.stderr,
                "stderr",
                &actual.stdout,
                MatchKind::NotPresent,
            )?;
        }
        for expect in self.expect_stderr_unordered.iter() {
            self.match_std(
                Some(expect),
                &actual.stderr,
                "stderr",
                &actual.stdout,
                MatchKind::Unordered,
            )?;
        }
        for expect in self.expect_neither_contains.iter() {
            self.match_std(
                Some(expect),
                &actual.stdout,
                "stdout",
                &actual.stdout,
                MatchKind::NotPresent,
            )?;

            self.match_std(
                Some(expect),
                &actual.stderr,
                "stderr",
                &actual.stderr,
                MatchKind::NotPresent,
            )?;
        }

        for expect in self.expect_either_contains.iter() {
            let match_std = self.match_std(
                Some(expect),
                &actual.stdout,
                "stdout",
                &actual.stdout,
                MatchKind::Partial,
            );
            let match_err = self.match_std(
                Some(expect),
                &actual.stderr,
                "stderr",
                &actual.stderr,
                MatchKind::Partial,
            );

            if let (Err(_), Err(_)) = (match_std, match_err) {
                Err(format!(
                    "expected to find:\n\
                     {}\n\n\
                     did not find in either output.",
                    expect
                ))?;
            }
        }

        for (with, without) in self.expect_stderr_with_without.iter() {
            self.match_with_without(&actual.stderr, with, without)?;
        }

        if let Some(ref objects) = self.expect_json {
            let stdout = str::from_utf8(&actual.stdout)
                .map_err(|_| "stdout was not utf8 encoded".to_owned())?;
            let lines = stdout
                .lines()
                .filter(|line| line.starts_with('{'))
                .collect::<Vec<_>>();
            if lines.len() != objects.len() {
                return Err(format!(
                    "expected {} json lines, got {}, stdout:\n{}",
                    objects.len(),
                    lines.len(),
                    stdout
                ));
            }
            for (obj, line) in objects.iter().zip(lines) {
                self.match_json(obj, line)?;
            }
        }

        if !self.expect_json_contains_unordered.is_empty() {
            let stdout = str::from_utf8(&actual.stdout)
                .map_err(|_| "stdout was not utf8 encoded".to_owned())?;
            let mut lines = stdout
                .lines()
                .filter(|line| line.starts_with('{'))
                .collect::<Vec<_>>();
            for obj in &self.expect_json_contains_unordered {
                match lines
                    .iter()
                    .position(|line| self.match_json(obj, line).is_ok())
                {
                    Some(index) => lines.remove(index),
                    None => {
                        return Err(format!(
                            "Did not find expected JSON:\n\
                             {}\n\
                             Remaining available output:\n\
                             {}\n",
                            serde_json::to_string_pretty(obj).unwrap(),
                            lines.join("\n")
                        ));
                    }
                };
            }
        }
        Ok(())
    }

    fn match_stderr(&self, actual: &Output) -> MatchResult {
        self.match_std(
            self.expect_stderr.as_ref(),
            &actual.stderr,
            "stderr",
            &actual.stdout,
            MatchKind::Exact,
        )
    }

    fn normalize_actual(&self, description: &str, actual: &[u8]) -> Result<String, String> {
        let actual = match str::from_utf8(actual) {
            Err(..) => return Err(format!("{} was not utf8 encoded", description)),
            Ok(actual) => actual,
        };
        // Let's not deal with \r\n vs \n on windows...
        let actual = actual.replace("\r", "");
        let actual = actual.replace("\t", "<tab>");
        Ok(actual)
    }

    fn replace_expected(&self, expected: &str) -> String {
        // Do the template replacements on the expected string.
        let replaced = match self.process_builder {
            None => expected.to_string(),
            Some(ref p) => match p.get_cwd() {
                None => expected.to_string(),
                Some(cwd) => expected.replace("[CWD]", &cwd.display().to_string()),
            },
        };

        // On Windows, we need to use a wildcard for the drive,
        // because we don't actually know what it will be.
        replaced.replace("[ROOT]", if cfg!(windows) { r#"[..]:\"# } else { "/" })
    }

    fn match_std(
        &self,
        expected: Option<&String>,
        actual: &[u8],
        description: &str,
        extra: &[u8],
        kind: MatchKind,
    ) -> MatchResult {
        let out = match expected {
            Some(out) => self.replace_expected(out),
            None => return Ok(()),
        };

        let actual = self.normalize_actual(description, actual)?;

        match kind {
            MatchKind::Exact => {
                let a = actual.lines();
                let e = out.lines();

                let diffs = self.diff_lines(a, e, false);
                if diffs.is_empty() {
                    Ok(())
                } else {
                    Err(format!(
                        "differences:\n\
                         {}\n\n\
                         other output:\n\
                         `{}`",
                        diffs.join("\n"),
                        String::from_utf8_lossy(extra)
                    ))
                }
            }
            MatchKind::Partial => {
                let mut a = actual.lines();
                let e = out.lines();

                let mut diffs = self.diff_lines(a.clone(), e.clone(), true);
                while let Some(..) = a.next() {
                    let a = self.diff_lines(a.clone(), e.clone(), true);
                    if a.len() < diffs.len() {
                        diffs = a;
                    }
                }
                if diffs.is_empty() {
                    Ok(())
                } else {
                    Err(format!(
                        "expected to find:\n\
                         {}\n\n\
                         did not find in output:\n\
                         {}",
                        out, actual
                    ))
                }
            }
            MatchKind::PartialN(number) => {
                let mut a = actual.lines();
                let e = out.lines();

                let mut matches = 0;

                while let Some(..) = {
                    if self.diff_lines(a.clone(), e.clone(), true).is_empty() {
                        matches += 1;
                    }
                    a.next()
                } {}

                if matches == number {
                    Ok(())
                } else {
                    Err(format!(
                        "expected to find {} occurrences:\n\
                         {}\n\n\
                         did not find in output:\n\
                         {}",
                        number, out, actual
                    ))
                }
            }
            MatchKind::NotPresent => {
                let mut a = actual.lines();
                let e = out.lines();

                let mut diffs = self.diff_lines(a.clone(), e.clone(), true);
                while let Some(..) = a.next() {
                    let a = self.diff_lines(a.clone(), e.clone(), true);
                    if a.len() < diffs.len() {
                        diffs = a;
                    }
                }
                if diffs.is_empty() {
                    Err(format!(
                        "expected not to find:\n\
                         {}\n\n\
                         but found in output:\n\
                         {}",
                        out, actual
                    ))
                } else {
                    Ok(())
                }
            }
            MatchKind::Unordered => {
                let mut a = actual.lines().collect::<Vec<_>>();
                let e = out.lines();

                for e_line in e {
                    match a.iter().position(|a_line| lines_match(e_line, a_line)) {
                        Some(index) => a.remove(index),
                        None => {
                            return Err(format!(
                                "Did not find expected line:\n\
                                 {}\n\
                                 Remaining available output:\n\
                                 {}\n",
                                e_line,
                                a.join("\n")
                            ));
                        }
                    };
                }
                if !a.is_empty() {
                    Err(format!(
                        "Output included extra lines:\n\
                         {}\n",
                        a.join("\n")
                    ))
                } else {
                    Ok(())
                }
            }
        }
    }

    fn match_with_without(
        &self,
        actual: &[u8],
        with: &[String],
        without: &[String],
    ) -> MatchResult {
        let actual = self.normalize_actual("stderr", actual)?;
        let contains = |s, line| {
            let mut s = self.replace_expected(s);
            s.insert_str(0, "[..]");
            s.push_str("[..]");
            lines_match(&s, line)
        };
        let matches: Vec<&str> = actual
            .lines()
            .filter(|line| with.iter().all(|with| contains(with, line)))
            .filter(|line| !without.iter().any(|without| contains(without, line)))
            .collect();
        match matches.len() {
            0 => Err(format!(
                "Could not find expected line in output.\n\
                 With contents: {:?}\n\
                 Without contents: {:?}\n\
                 Actual stderr:\n\
                 {}\n",
                with, without, actual
            )),
            1 => Ok(()),
            _ => Err(format!(
                "Found multiple matching lines, but only expected one.\n\
                 With contents: {:?}\n\
                 Without contents: {:?}\n\
                 Matching lines:\n\
                 {}\n",
                with,
                without,
                matches.join("\n")
            )),
        }
    }

    fn match_json(&self, expected: &Value, line: &str) -> MatchResult {
        let actual = match line.parse() {
            Err(e) => return Err(format!("invalid json, {}:\n`{}`", e, line)),
            Ok(actual) => actual,
        };

        find_json_mismatch(expected, &actual)
    }

    fn diff_lines<'a>(
        &self,
        actual: str::Lines<'a>,
        expected: str::Lines<'a>,
        partial: bool,
    ) -> Vec<String> {
        let actual = actual.take(if partial {
            expected.clone().count()
        } else {
            usize::MAX
        });
        zip_all(actual, expected)
            .enumerate()
            .filter_map(|(i, (a, e))| match (a, e) {
                (Some(a), Some(e)) => {
                    if lines_match(e, a) {
                        None
                    } else {
                        Some(format!("{:3} - |{}|\n    + |{}|\n", i, e, a))
                    }
                }
                (Some(a), None) => Some(format!("{:3} -\n    + |{}|\n", i, a)),
                (None, Some(e)) => Some(format!("{:3} - |{}|\n    +\n", i, e)),
                (None, None) => panic!("Cannot get here"),
            })
            .collect()
    }
}

impl Drop for Execs {
    fn drop(&mut self) {
        if !self.ran && !std::thread::panicking() {
            panic!("forgot to run this command");
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum MatchKind {
    Exact,
    Partial,
    PartialN(usize),
    NotPresent,
    Unordered,
}

/// Compares a line with an expected pattern.
/// - Use `[..]` as a wildcard to match 0 or more characters on the same line
///   (similar to `.*` in a regex).
/// - Use `[EXE]` to optionally add `.exe` on Windows (empty string on other
///   platforms).
/// - There is a wide range of macros (such as `[COMPILING]` or `[WARNING]`)
///   to match cargo's "status" output and allows you to ignore the alignment.
///   See `substitute_macros` for a complete list of macros.
/// - `[ROOT]` is `/` or `[..]:\` on Windows.
/// - `[CWD]` is the working directory of the process that was run.
pub fn lines_match(expected: &str, actual: &str) -> bool {
    // Let's not deal with / vs \ (windows...)
    // First replace backslash-escaped backslashes with forward slashes
    // which can occur in, for example, JSON output
    let expected = expected.replace("\\\\", "/").replace("\\", "/");
    let mut actual: &str = &actual.replace("\\\\", "/").replace("\\", "/");
    let expected = substitute_macros(&expected);
    for (i, part) in expected.split("[..]").enumerate() {
        match actual.find(part) {
            Some(j) => {
                if i == 0 && j != 0 {
                    return false;
                }
                actual = &actual[j + part.len()..];
            }
            None => return false,
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

/// Compares JSON object for approximate equality.
/// You can use `[..]` wildcard in strings (useful for OS-dependent things such
/// as paths). You can use a `"{...}"` string literal as a wildcard for
/// arbitrary nested JSON (useful for parts of object emitted by other programs
/// (e.g., rustc) rather than Cargo itself). Arrays are sorted before comparison.
pub fn find_json_mismatch(expected: &Value, actual: &Value) -> Result<(), String> {
    match find_json_mismatch_r(expected, actual) {
        Some((expected_part, actual_part)) => Err(format!(
            "JSON mismatch\nExpected:\n{}\nWas:\n{}\nExpected part:\n{}\nActual part:\n{}\n",
            serde_json::to_string_pretty(expected).unwrap(),
            serde_json::to_string_pretty(&actual).unwrap(),
            serde_json::to_string_pretty(expected_part).unwrap(),
            serde_json::to_string_pretty(actual_part).unwrap(),
        )),
        None => Ok(()),
    }
}

fn find_json_mismatch_r<'a>(
    expected: &'a Value,
    actual: &'a Value,
) -> Option<(&'a Value, &'a Value)> {
    use serde_json::Value::*;
    match (expected, actual) {
        (&Number(ref l), &Number(ref r)) if l == r => None,
        (&Bool(l), &Bool(r)) if l == r => None,
        (&String(ref l), &String(ref r)) if lines_match(l, r) => None,
        (&Array(ref l), &Array(ref r)) => {
            if l.len() != r.len() {
                return Some((expected, actual));
            }

            let mut l = l.iter().collect::<Vec<_>>();
            let mut r = r.iter().collect::<Vec<_>>();

            l.retain(
                |l| match r.iter().position(|r| find_json_mismatch_r(l, r).is_none()) {
                    Some(i) => {
                        r.remove(i);
                        false
                    }
                    None => true,
                },
            );

            if !l.is_empty() {
                assert!(!r.is_empty());
                Some((l[0], r[0]))
            } else {
                assert_eq!(r.len(), 0);
                None
            }
        }
        (&Object(ref l), &Object(ref r)) => {
            let same_keys = l.len() == r.len() && l.keys().all(|k| r.contains_key(k));
            if !same_keys {
                return Some((expected, actual));
            }

            l.values()
                .zip(r.values())
                .filter_map(|(l, r)| find_json_mismatch_r(l, r))
                .nth(0)
        }
        (&Null, &Null) => None,
        // Magic string literal `"{...}"` acts as wildcard for any sub-JSON.
        (&String(ref l), _) if l == "{...}" => None,
        _ => Some((expected, actual)),
    }
}

struct ZipAll<I1: Iterator, I2: Iterator> {
    first: I1,
    second: I2,
}

impl<T, I1: Iterator<Item = T>, I2: Iterator<Item = T>> Iterator for ZipAll<I1, I2> {
    type Item = (Option<T>, Option<T>);
    fn next(&mut self) -> Option<(Option<T>, Option<T>)> {
        let first = self.first.next();
        let second = self.second.next();

        match (first, second) {
            (None, None) => None,
            (a, b) => Some((a, b)),
        }
    }
}

fn zip_all<T, I1: Iterator<Item = T>, I2: Iterator<Item = T>>(a: I1, b: I2) -> ZipAll<I1, I2> {
    ZipAll {
        first: a,
        second: b,
    }
}

impl fmt::Debug for Execs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "execs")
    }
}

pub fn execs() -> Execs {
    Execs {
        ran: false,
        process_builder: None,
        expect_stdout: None,
        expect_stderr: None,
        expect_stdin: None,
        expect_exit_code: Some(0),
        expect_stdout_contains: Vec::new(),
        expect_stderr_contains: Vec::new(),
        expect_either_contains: Vec::new(),
        expect_stdout_contains_n: Vec::new(),
        expect_stdout_not_contains: Vec::new(),
        expect_stderr_not_contains: Vec::new(),
        expect_stderr_unordered: Vec::new(),
        expect_neither_contains: Vec::new(),
        expect_stderr_with_without: Vec::new(),
        expect_json: None,
        expect_json_contains_unordered: Vec::new(),
        stream_output: false,
    }
}

pub trait Tap {
    fn tap<F: FnOnce(&mut Self)>(self, callback: F) -> Self;
}

impl<T> Tap for T {
    fn tap<F: FnOnce(&mut Self)>(mut self, callback: F) -> T {
        callback(&mut self);
        self
    }
}

pub fn basic_manifest(name: &str, version: &str) -> String {
    format!(
        r#"
        [package]
        name = "{}"
        version = "{}"
        authors = []
    "#,
        name, version
    )
}

pub fn basic_bin_manifest(name: &str) -> String {
    format!(
        r#"
        [package]

        name = "{}"
        version = "0.5.0"
        authors = ["wycats@example.com"]

        [[bin]]

        name = "{}"
    "#,
        name, name
    )
}

pub fn basic_lib_manifest(name: &str) -> String {
    format!(
        r#"
        [package]

        name = "{}"
        version = "0.5.0"
        authors = ["wycats@example.com"]

        [lib]

        name = "{}"
    "#,
        name, name
    )
}

pub fn path2url<P: AsRef<Path>>(p: P) -> Url {
    Url::from_file_path(p).ok().unwrap()
}

fn substitute_macros(input: &str) -> String {
    let macros = [
        ("[RUNNING]", "     Running"),
        ("[COMPILING]", "   Compiling"),
        ("[CHECKING]", "    Checking"),
        ("[CREATED]", "     Created"),
        ("[FINISHED]", "    Finished"),
        ("[ERROR]", "error:"),
        ("[WARNING]", "warning:"),
        ("[DOCUMENTING]", " Documenting"),
        ("[FRESH]", "       Fresh"),
        ("[UPDATING]", "    Updating"),
        ("[ADDING]", "      Adding"),
        ("[REMOVING]", "    Removing"),
        ("[DOCTEST]", "   Doc-tests"),
        ("[PACKAGING]", "   Packaging"),
        ("[DOWNLOADING]", " Downloading"),
        ("[DOWNLOADED]", "  Downloaded"),
        ("[UPLOADING]", "   Uploading"),
        ("[VERIFYING]", "   Verifying"),
        ("[ARCHIVING]", "   Archiving"),
        ("[INSTALLING]", "  Installing"),
        ("[REPLACING]", "   Replacing"),
        ("[UNPACKING]", "   Unpacking"),
        ("[SUMMARY]", "     Summary"),
        ("[FIXING]", "      Fixing"),
        ("[EXE]", env::consts::EXE_SUFFIX),
    ];
    let mut result = input.to_owned();
    for &(pat, subst) in &macros {
        result = result.replace(pat, subst)
    }
    result
}

pub mod install;

thread_local!(
pub static RUSTC: Rustc = Rustc::new(
    PathBuf::from("rustc"),
    None,
    Path::new("should be path to rustup rustc, but we don't care in tests"),
    None,
).unwrap()
);

/// The rustc host such as `x86_64-unknown-linux-gnu`.
pub fn rustc_host() -> String {
    RUSTC.with(|r| r.host.clone())
}

pub fn is_nightly() -> bool {
    RUSTC.with(|r| r.verbose_version.contains("-nightly") || r.verbose_version.contains("-dev"))
}

pub fn process<T: AsRef<OsStr>>(t: T) -> cargo::util::ProcessBuilder {
    _process(t.as_ref())
}

fn _process(t: &OsStr) -> cargo::util::ProcessBuilder {
    let mut p = cargo::util::process(t);
    p.cwd(&paths::root())
        .env_remove("CARGO_HOME")
        .env("HOME", paths::home())
        .env("CARGO_HOME", paths::home().join(".cargo"))
        .env("__CARGO_TEST_ROOT", paths::root())
        // Force Cargo to think it's on the stable channel for all tests, this
        // should hopefully not surprise us as we add cargo features over time and
        // cargo rides the trains.
        .env("__CARGO_TEST_CHANNEL_OVERRIDE_DO_NOT_USE_THIS", "stable")
        // For now disable incremental by default as support hasn't ridden to the
        // stable channel yet. Once incremental support hits the stable compiler we
        // can switch this to one and then fix the tests.
        .env("CARGO_INCREMENTAL", "0")
        // This env var can switch the git backend from libgit2 to git2-curl, which
        // can tweak error messages and cause some tests to fail, so let's forcibly
        // remove it.
        .env_remove("CARGO_HTTP_CHECK_REVOKE")
        .env_remove("__CARGO_DEFAULT_LIB_METADATA")
        .env_remove("RUSTC")
        .env_remove("RUSTDOC")
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTFLAGS")
        .env_remove("XDG_CONFIG_HOME") // see #2345
        .env("GIT_CONFIG_NOSYSTEM", "1") // keep trying to sandbox ourselves
        .env_remove("EMAIL")
        .env_remove("MFLAGS")
        .env_remove("MAKEFLAGS")
        .env_remove("CARGO_MAKEFLAGS")
        .env_remove("GIT_AUTHOR_NAME")
        .env_remove("GIT_AUTHOR_EMAIL")
        .env_remove("GIT_COMMITTER_NAME")
        .env_remove("GIT_COMMITTER_EMAIL")
        .env_remove("CARGO_TARGET_DIR") // we assume 'target'
        .env_remove("MSYSTEM"); // assume cmd.exe everywhere on windows
    p
}

pub trait ChannelChanger: Sized {
    fn masquerade_as_nightly_cargo(&mut self) -> &mut Self;
}

impl ChannelChanger for cargo::util::ProcessBuilder {
    fn masquerade_as_nightly_cargo(&mut self) -> &mut Self {
        self.env("__CARGO_TEST_CHANNEL_OVERRIDE_DO_NOT_USE_THIS", "nightly")
    }
}

fn split_and_add_args(p: &mut ProcessBuilder, s: &str) {
    for arg in s.split_whitespace() {
        if arg.contains('"') || arg.contains('\'') {
            panic!("shell-style argument parsing is not supported")
        }
        p.arg(arg);
    }
}

pub fn cargo_process(s: &str) -> Execs {
    let mut p = process(&cargo_exe());
    split_and_add_args(&mut p, s);
    execs().with_process_builder(p)
}

pub fn git_process(s: &str) -> ProcessBuilder {
    let mut p = process("git");
    split_and_add_args(&mut p, s);
    p
}

pub fn sleep_ms(ms: u64) {
    ::std::thread::sleep(Duration::from_millis(ms));
}

/// Returns `true` if the local filesystem has low-resolution mtimes.
pub fn is_coarse_mtime() -> bool {
    // If the filetime crate is being used to emulate HFS then
    // return `true`, without looking at the actual hardware.
    cfg!(emulate_second_only_system) ||
    // This should actually be a test that `$CARGO_TARGET_DIR` is on an HFS
    // filesystem, (or any filesystem with low-resolution mtimes). However,
    // that's tricky to detect, so for now just deal with CI.
    cfg!(target_os = "macos") && env::var("CI").is_ok()
}

/// Some CI setups are much slower then the equipment used by Cargo itself.
/// Architectures that do not have a modern processor, hardware emulation, ect.
/// This provides a way for those setups to increase the cut off for all the time based test.
pub fn slow_cpu_multiplier(main: u64) -> Duration {
    lazy_static::lazy_static! {
        static ref SLOW_CPU_MULTIPLIER: u64 =
            env::var("CARGO_TEST_SLOW_CPU_MULTIPLIER").ok().and_then(|m| m.parse().ok()).unwrap_or(1);
    }
    Duration::from_secs(*SLOW_CPU_MULTIPLIER * main)
}
