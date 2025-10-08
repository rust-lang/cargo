//! # Cargo test support.
//!
//! See <https://rust-lang.github.io/cargo/contrib/> for a guide on writing tests.
//!
//! There are two places you can find API documentation
//!
//! - <https://docs.rs/cargo-test-support>:
//!   targeted at external tool developers testing cargo-related code
//!   - Released with every rustc release
//! - <https://doc.rust-lang.org/nightly/nightly-rustc/cargo_test_support>:
//!   targeted at cargo contributors
//!   - Updated on each update of the `cargo` submodule in `rust-lang/rust`
//!
//! > This crate is maintained by the Cargo team, primarily for use by Cargo
//! > and not intended for external use. This
//! > crate may make major changes to its APIs or be deprecated without warning.
//!
//! # Example
//!
//! ```rust,no_run
//! use cargo_test_support::prelude::*;
//! use cargo_test_support::str;
//! use cargo_test_support::project;
//!
//! #[cargo_test]
//! fn some_test() {
//!     let p = project()
//!         .file("src/main.rs", r#"fn main() { println!("hi!"); }"#)
//!         .build();
//!
//!     p.cargo("run --bin foo")
//!         .with_stderr_data(str![[r#"
//! [COMPILING] foo [..]
//! [FINISHED] [..]
//! [RUNNING] `target/debug/foo`
//! "#]])
//!         .with_stdout_data(str![["hi!"]])
//!         .run();
//! }
//! ```

#![allow(clippy::disallowed_methods)]
#![allow(clippy::print_stderr)]
#![allow(clippy::print_stdout)]

use std::env;
use std::ffi::OsStr;
use std::fmt::Write;
use std::fs;
use std::os;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::LazyLock;
use std::sync::OnceLock;
use std::thread::JoinHandle;
use std::time::{self, Duration};

use anyhow::{Result, bail};
use cargo_util::{ProcessError, is_ci};
use snapbox::IntoData as _;
use url::Url;

use self::paths::CargoPathExt;

/// Unwrap a `Result` with a useful panic message
///
/// # Example
///
/// ```rust
/// use cargo_test_support::t;
/// t!(std::fs::read_to_string("Cargo.toml"));
/// ```
#[macro_export]
macro_rules! t {
    ($e:expr) => {
        match $e {
            Ok(e) => e,
            Err(e) => $crate::panic_error(&format!("failed running {}", stringify!($e)), e),
        }
    };
}

pub use cargo_util::ProcessBuilder;
pub use snapbox::file;
pub use snapbox::str;
pub use snapbox::utils::current_dir;

/// `panic!`, reporting the specified error , see also [`t!`]
#[track_caller]
pub fn panic_error(what: &str, err: impl Into<anyhow::Error>) -> ! {
    let err = err.into();
    pe(what, err);
    #[track_caller]
    fn pe(what: &str, err: anyhow::Error) -> ! {
        let mut result = format!("{}\nerror: {}", what, err);
        for cause in err.chain().skip(1) {
            let _ = writeln!(result, "\nCaused by:");
            let _ = write!(result, "{}", cause);
        }
        panic!("\n{}", result);
    }
}

pub use cargo_test_macro::cargo_test;

pub mod compare;
pub mod containers;
pub mod cross_compile;
pub mod git;
pub mod install;
pub mod paths;
pub mod publish;
pub mod registry;

pub mod prelude {
    pub use crate::ArgLineCommandExt;
    pub use crate::ChannelChangerCommandExt;
    pub use crate::TestEnvCommandExt;
    pub use crate::cargo_test;
    pub use crate::paths::CargoPathExt;
    pub use snapbox::IntoData;
}

/*
 *
 * ===== Builders =====
 *
 */

#[derive(PartialEq, Clone)]
struct FileBuilder {
    path: PathBuf,
    body: String,
    executable: bool,
}

impl FileBuilder {
    pub fn new(path: PathBuf, body: &str, executable: bool) -> FileBuilder {
        FileBuilder {
            path,
            body: body.to_string(),
            executable: executable,
        }
    }

    fn mk(&mut self) {
        if self.executable {
            let mut path = self.path.clone().into_os_string();
            write!(path, "{}", env::consts::EXE_SUFFIX).unwrap();
            self.path = path.into();
        }

        self.dirname().mkdir_p();
        fs::write(&self.path, &self.body)
            .unwrap_or_else(|e| panic!("could not create file {}: {}", self.path.display(), e));

        #[cfg(unix)]
        if self.executable {
            use std::os::unix::fs::PermissionsExt;

            let mut perms = fs::metadata(&self.path).unwrap().permissions();
            let mode = perms.mode();
            perms.set_mode(mode | 0o111);
            fs::set_permissions(&self.path, perms).unwrap();
        }
    }

    fn dirname(&self) -> &Path {
        self.path.parent().unwrap()
    }
}

#[derive(PartialEq, Clone)]
struct SymlinkBuilder {
    dst: PathBuf,
    src: PathBuf,
    src_is_dir: bool,
}

impl SymlinkBuilder {
    pub fn new(dst: PathBuf, src: PathBuf) -> SymlinkBuilder {
        SymlinkBuilder {
            dst,
            src,
            src_is_dir: false,
        }
    }

    pub fn new_dir(dst: PathBuf, src: PathBuf) -> SymlinkBuilder {
        SymlinkBuilder {
            dst,
            src,
            src_is_dir: true,
        }
    }

    #[cfg(unix)]
    fn mk(&self) {
        self.dirname().mkdir_p();
        t!(os::unix::fs::symlink(&self.dst, &self.src));
    }

    #[cfg(windows)]
    fn mk(&mut self) {
        self.dirname().mkdir_p();
        if self.src_is_dir {
            t!(os::windows::fs::symlink_dir(&self.dst, &self.src));
        } else {
            if let Some(ext) = self.dst.extension() {
                if ext == env::consts::EXE_EXTENSION {
                    self.src.set_extension(ext);
                }
            }
            t!(os::windows::fs::symlink_file(&self.dst, &self.src));
        }
    }

    fn dirname(&self) -> &Path {
        self.src.parent().unwrap()
    }
}

/// A cargo project to run tests against.
///
/// See [`ProjectBuilder`] or [`Project::from_template`] to get started.
pub struct Project {
    root: PathBuf,
}

/// Create a project to run tests against
///
/// - Creates a [`basic_manifest`] if one isn't supplied
///
/// To get started, see:
/// - [`project`]
/// - [`project_in`]
/// - [`project_in_home`]
/// - [`Project::from_template`]
#[must_use]
pub struct ProjectBuilder {
    root: Project,
    files: Vec<FileBuilder>,
    symlinks: Vec<SymlinkBuilder>,
    no_manifest: bool,
}

impl ProjectBuilder {
    /// Root of the project
    ///
    /// ex: `$CARGO_TARGET_TMPDIR/cit/t0/foo`
    pub fn root(&self) -> PathBuf {
        self.root.root()
    }

    /// Project's debug dir
    ///
    /// ex: `$CARGO_TARGET_TMPDIR/cit/t0/foo/target/debug`
    pub fn target_debug_dir(&self) -> PathBuf {
        self.root.target_debug_dir()
    }

    /// Create project in `root`
    pub fn new(root: PathBuf) -> ProjectBuilder {
        ProjectBuilder {
            root: Project { root },
            files: vec![],
            symlinks: vec![],
            no_manifest: false,
        }
    }

    /// Create project, relative to [`paths::root`]
    pub fn at<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.root = Project {
            root: paths::root().join(path),
        };
        self
    }

    /// Adds a file to the project.
    pub fn file<B: AsRef<Path>>(mut self, path: B, body: &str) -> Self {
        self._file(path.as_ref(), body, false);
        self
    }

    /// Adds an executable file to the project.
    pub fn executable<B: AsRef<Path>>(mut self, path: B, body: &str) -> Self {
        self._file(path.as_ref(), body, true);
        self
    }

    fn _file(&mut self, path: &Path, body: &str, executable: bool) {
        self.files.push(FileBuilder::new(
            self.root.root().join(path),
            body,
            executable,
        ));
    }

    /// Adds a symlink to a file to the project.
    pub fn symlink(mut self, dst: impl AsRef<Path>, src: impl AsRef<Path>) -> Self {
        self.symlinks.push(SymlinkBuilder::new(
            self.root.root().join(dst),
            self.root.root().join(src),
        ));
        self
    }

    /// Create a symlink to a directory
    pub fn symlink_dir(mut self, dst: impl AsRef<Path>, src: impl AsRef<Path>) -> Self {
        self.symlinks.push(SymlinkBuilder::new_dir(
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
            self._file(
                Path::new("Cargo.toml"),
                &basic_manifest("foo", "0.0.1"),
                false,
            )
        }

        let past = time::SystemTime::now() - Duration::new(1, 0);
        let ftime = filetime::FileTime::from_system_time(past);

        for file in self.files.iter_mut() {
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

        for symlink in self.symlinks.iter_mut() {
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
    /// Copy the test project from a fixed state
    pub fn from_template(template_path: impl AsRef<Path>) -> Self {
        let root = paths::root();
        let project_root = root.join("case");
        snapbox::dir::copy_template(template_path.as_ref(), &project_root).unwrap();
        Self { root: project_root }
    }

    /// Root of the project
    ///
    /// ex: `$CARGO_TARGET_TMPDIR/cit/t0/foo`
    pub fn root(&self) -> PathBuf {
        self.root.clone()
    }

    /// Project's target dir
    ///
    /// ex: `$CARGO_TARGET_TMPDIR/cit/t0/foo/target`
    pub fn build_dir(&self) -> PathBuf {
        self.root().join("target")
    }

    /// Project's debug dir
    ///
    /// ex: `$CARGO_TARGET_TMPDIR/cit/t0/foo/target/debug`
    pub fn target_debug_dir(&self) -> PathBuf {
        self.build_dir().join("debug")
    }

    /// File url for root
    ///
    /// ex: `file://$CARGO_TARGET_TMPDIR/cit/t0/foo`
    pub fn url(&self) -> Url {
        use paths::CargoPathExt;
        self.root().to_url()
    }

    /// Path to an example built as a library.
    ///
    /// `kind` should be one of: "lib", "rlib", "staticlib", "dylib", "proc-macro"
    ///
    /// ex: `$CARGO_TARGET_TMPDIR/cit/t0/foo/target/debug/examples/libex.rlib`
    pub fn example_lib(&self, name: &str, kind: &str) -> PathBuf {
        self.target_debug_dir()
            .join("examples")
            .join(paths::get_lib_filename(name, kind))
    }

    /// Path to a dynamic library.
    /// ex: `/path/to/cargo/target/cit/t0/foo/target/debug/examples/libex.dylib`
    pub fn dylib(&self, name: &str) -> PathBuf {
        self.target_debug_dir().join(format!(
            "{}{name}{}",
            env::consts::DLL_PREFIX,
            env::consts::DLL_SUFFIX
        ))
    }

    /// Path to a debug binary.
    ///
    /// ex: `$CARGO_TARGET_TMPDIR/cit/t0/foo/target/debug/foo`
    pub fn bin(&self, b: &str) -> PathBuf {
        self.build_dir()
            .join("debug")
            .join(&format!("{}{}", b, env::consts::EXE_SUFFIX))
    }

    /// Path to a release binary.
    ///
    /// ex: `$CARGO_TARGET_TMPDIR/cit/t0/foo/target/release/foo`
    pub fn release_bin(&self, b: &str) -> PathBuf {
        self.build_dir()
            .join("release")
            .join(&format!("{}{}", b, env::consts::EXE_SUFFIX))
    }

    /// Path to a debug binary for a specific target triple.
    ///
    /// ex: `$CARGO_TARGET_TMPDIR/cit/t0/foo/target/i686-apple-darwin/debug/foo`
    pub fn target_bin(&self, target: &str, b: &str) -> PathBuf {
        self.build_dir().join(target).join("debug").join(&format!(
            "{}{}",
            b,
            env::consts::EXE_SUFFIX
        ))
    }

    /// Returns an iterator of paths within [`Project::root`] matching the glob pattern
    pub fn glob<P: AsRef<Path>>(&self, pattern: P) -> glob::Paths {
        let pattern = self.root().join(pattern);
        glob::glob(pattern.to_str().expect("failed to convert pattern to str"))
            .expect("failed to glob")
    }

    /// Overwrite a file with new content
    ///
    // # Example:
    ///
    /// ```no_run
    /// # let p = cargo_test_support::project().build();
    /// p.change_file("src/lib.rs", "fn new_fn() {}");
    /// ```
    pub fn change_file(&self, path: impl AsRef<Path>, body: &str) {
        FileBuilder::new(self.root().join(path), body, false).mk()
    }

    /// Creates a `ProcessBuilder` to run a program in the project
    /// and wrap it in an Execs to assert on the execution.
    ///
    /// # Example:
    ///
    /// ```no_run
    /// # use cargo_test_support::str;
    /// # let p = cargo_test_support::project().build();
    /// p.process(&p.bin("foo"))
    ///     .with_stdout_data(str!["bar\n"])
    ///     .run();
    /// ```
    pub fn process<T: AsRef<OsStr>>(&self, program: T) -> Execs {
        let mut p = process(program);
        p.cwd(self.root());
        execs().with_process_builder(p)
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
    pub fn read_file(&self, path: impl AsRef<Path>) -> String {
        let full = self.root().join(path);
        fs::read_to_string(&full)
            .unwrap_or_else(|e| panic!("could not read file {}: {}", full.display(), e))
    }

    /// Modifies `Cargo.toml` to remove all commented lines.
    pub fn uncomment_root_manifest(&self) {
        let contents = self.read_file("Cargo.toml").replace("#", "");
        fs::write(self.root().join("Cargo.toml"), contents).unwrap();
    }

    pub fn symlink(&self, src: impl AsRef<Path>, dst: impl AsRef<Path>) {
        let src = self.root().join(src.as_ref());
        let dst = self.root().join(dst.as_ref());
        #[cfg(unix)]
        {
            if let Err(e) = os::unix::fs::symlink(&src, &dst) {
                panic!("failed to symlink {:?} to {:?}: {:?}", src, dst, e);
            }
        }
        #[cfg(windows)]
        {
            if src.is_dir() {
                if let Err(e) = os::windows::fs::symlink_dir(&src, &dst) {
                    panic!("failed to symlink {:?} to {:?}: {:?}", src, dst, e);
                }
            } else {
                if let Err(e) = os::windows::fs::symlink_file(&src, &dst) {
                    panic!("failed to symlink {:?} to {:?}: {:?}", src, dst, e);
                }
            }
        }
    }
}

/// Generates a project layout, see [`ProjectBuilder`]
pub fn project() -> ProjectBuilder {
    ProjectBuilder::new(paths::root().join("foo"))
}

/// Generates a project layout in given directory, see [`ProjectBuilder`]
pub fn project_in(dir: impl AsRef<Path>) -> ProjectBuilder {
    ProjectBuilder::new(paths::root().join(dir).join("foo"))
}

/// Generates a project layout inside our fake home dir, see [`ProjectBuilder`]
pub fn project_in_home(name: impl AsRef<Path>) -> ProjectBuilder {
    ProjectBuilder::new(paths::home().join(name))
}

// === Helpers ===

/// Generate a `main.rs` printing the specified text
///
/// ```rust
/// # use cargo_test_support::main_file;
/// # mod dep {
/// #     fn bar() -> &'static str {
/// #         "world"
/// #     }
/// # }
/// main_file(
///     r#""hello {}", dep::bar()"#,
///     &[]
/// );
/// ```
pub fn main_file(println: &str, externed_deps: &[&str]) -> String {
    let mut buf = String::new();

    for dep in externed_deps.iter() {
        buf.push_str(&format!("extern crate {};\n", dep));
    }

    buf.push_str("fn main() { println!(");
    buf.push_str(println);
    buf.push_str("); }\n");

    buf
}

/// This is the raw output from the process.
///
/// This is similar to `std::process::Output`, however the `status` is
/// translated to the raw `code`. This is necessary because `ProcessError`
/// does not have access to the raw `ExitStatus` because `ProcessError` needs
/// to be serializable (for the Rustc cache), and `ExitStatus` does not
/// provide a constructor.
pub struct RawOutput {
    pub code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// Run and verify a [`ProcessBuilder`]
///
/// Construct with
/// - [`execs`]
/// - [`Project`] methods
/// - `cargo_process` in testsuite
#[must_use]
#[derive(Clone)]
pub struct Execs {
    ran: bool,
    process_builder: Option<ProcessBuilder>,
    expect_stdin: Option<String>,
    expect_exit_code: Option<i32>,
    expect_stdout_data: Option<snapbox::Data>,
    expect_stderr_data: Option<snapbox::Data>,
    expect_stdout_contains: Vec<String>,
    expect_stderr_contains: Vec<String>,
    expect_stdout_not_contains: Vec<String>,
    expect_stderr_not_contains: Vec<String>,
    expect_stderr_with_without: Vec<(Vec<String>, Vec<String>)>,
    stream_output: bool,
    assert: snapbox::Assert,
}

impl Execs {
    pub fn with_process_builder(mut self, p: ProcessBuilder) -> Execs {
        self.process_builder = Some(p);
        self
    }
}

/// # Configure assertions
impl Execs {
    /// Verifies that stdout is equal to the given lines.
    ///
    /// See [`compare::assert_e2e`] for assertion details.
    ///
    /// <div class="warning">
    ///
    /// Prefer passing in [`str!`] for `expected` to get snapshot updating.
    ///
    /// If `format!` is needed for content that changes from run to run that you don't care about,
    /// consider whether you could have [`compare::assert_e2e`] redact the content.
    /// If nothing else, a wildcard (`[..]`, `...`) may be useful.
    ///
    /// However, `""` may be preferred for intentionally empty output so people don't accidentally
    /// bless a change.
    ///
    /// </div>
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use cargo_test_support::prelude::*;
    /// use cargo_test_support::str;
    /// use cargo_test_support::execs;
    ///
    /// execs().with_stdout_data(str![r#"
    /// Hello world!
    /// "#]);
    /// ```
    ///
    /// Non-deterministic compiler output
    /// ```no_run
    /// use cargo_test_support::prelude::*;
    /// use cargo_test_support::str;
    /// use cargo_test_support::execs;
    ///
    /// execs().with_stdout_data(str![r#"
    /// [COMPILING] foo
    /// [COMPILING] bar
    /// "#].unordered());
    /// ```
    ///
    /// jsonlines
    /// ```no_run
    /// use cargo_test_support::prelude::*;
    /// use cargo_test_support::str;
    /// use cargo_test_support::execs;
    ///
    /// execs().with_stdout_data(str![r#"
    /// [
    ///   {},
    ///   {}
    /// ]
    /// "#].is_json().against_jsonlines());
    /// ```
    pub fn with_stdout_data(&mut self, expected: impl snapbox::IntoData) -> &mut Self {
        self.expect_stdout_data = Some(expected.into_data());
        self
    }

    /// Verifies that stderr is equal to the given lines.
    ///
    /// See [`compare::assert_e2e`] for assertion details.
    ///
    /// <div class="warning">
    ///
    /// Prefer passing in [`str!`] for `expected` to get snapshot updating.
    ///
    /// If `format!` is needed for content that changes from run to run that you don't care about,
    /// consider whether you could have [`compare::assert_e2e`] redact the content.
    /// If nothing else, a wildcard (`[..]`, `...`) may be useful.
    ///
    /// However, `""` may be preferred for intentionally empty output so people don't accidentally
    /// bless a change.
    ///
    /// </div>
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use cargo_test_support::prelude::*;
    /// use cargo_test_support::str;
    /// use cargo_test_support::execs;
    ///
    /// execs().with_stderr_data(str![r#"
    /// Hello world!
    /// "#]);
    /// ```
    ///
    /// Non-deterministic compiler output
    /// ```no_run
    /// use cargo_test_support::prelude::*;
    /// use cargo_test_support::str;
    /// use cargo_test_support::execs;
    ///
    /// execs().with_stderr_data(str![r#"
    /// [COMPILING] foo
    /// [COMPILING] bar
    /// "#].unordered());
    /// ```
    ///
    /// jsonlines
    /// ```no_run
    /// use cargo_test_support::prelude::*;
    /// use cargo_test_support::str;
    /// use cargo_test_support::execs;
    ///
    /// execs().with_stderr_data(str![r#"
    /// [
    ///   {},
    ///   {}
    /// ]
    /// "#].is_json().against_jsonlines());
    /// ```
    pub fn with_stderr_data(&mut self, expected: impl snapbox::IntoData) -> &mut Self {
        self.expect_stderr_data = Some(expected.into_data());
        self
    }

    /// Writes the given lines to stdin.
    pub fn with_stdin<S: ToString>(&mut self, expected: S) -> &mut Self {
        self.expect_stdin = Some(expected.to_string());
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
    ///
    /// See [`compare`] for supported patterns.
    ///
    /// <div class="warning">
    ///
    /// Prefer [`Execs::with_stdout_data`] where possible.
    /// - `expected` cannot be snapshotted
    /// - `expected` can end up being ambiguous, causing the assertion to succeed when it should fail
    ///
    /// </div>
    pub fn with_stdout_contains<S: ToString>(&mut self, expected: S) -> &mut Self {
        self.expect_stdout_contains.push(expected.to_string());
        self
    }

    /// Verifies that stderr contains the given contiguous lines somewhere in
    /// its output.
    ///
    /// See [`compare`] for supported patterns.
    ///
    /// <div class="warning">
    ///
    /// Prefer [`Execs::with_stderr_data`] where possible.
    /// - `expected` cannot be snapshotted
    /// - `expected` can end up being ambiguous, causing the assertion to succeed when it should fail
    ///
    /// </div>
    pub fn with_stderr_contains<S: ToString>(&mut self, expected: S) -> &mut Self {
        self.expect_stderr_contains.push(expected.to_string());
        self
    }

    /// Verifies that stdout does not contain the given contiguous lines.
    ///
    /// See [`compare`] for supported patterns.
    ///
    /// See note on [`Self::with_stderr_does_not_contain`].
    ///
    /// <div class="warning">
    ///
    /// Prefer [`Execs::with_stdout_data`] where possible.
    /// - `expected` cannot be snapshotted
    /// - The absence of `expected` can either mean success or that the string being looked for
    ///   changed.
    ///
    /// To mitigate this, consider matching this up with
    /// [`Execs::with_stdout_contains`].
    ///
    /// </div>
    pub fn with_stdout_does_not_contain<S: ToString>(&mut self, expected: S) -> &mut Self {
        self.expect_stdout_not_contains.push(expected.to_string());
        self
    }

    /// Verifies that stderr does not contain the given contiguous lines.
    ///
    /// See [`compare`] for supported patterns.
    ///
    /// <div class="warning">
    ///
    /// Prefer [`Execs::with_stdout_data`] where possible.
    /// - `expected` cannot be snapshotted
    /// - The absence of `expected` can either mean success or that the string being looked for
    ///   changed.
    ///
    /// To mitigate this, consider either matching this up with
    /// [`Execs::with_stdout_contains`] or replace it
    /// with [`Execs::with_stderr_line_without`].
    ///
    /// </div>
    pub fn with_stderr_does_not_contain<S: ToString>(&mut self, expected: S) -> &mut Self {
        self.expect_stderr_not_contains.push(expected.to_string());
        self
    }

    /// Verify that a particular line appears in stderr with and without the
    /// given substrings. Exactly one line must match.
    ///
    /// The substrings are matched as `contains`.
    ///
    /// <div class="warning">
    ///
    /// Prefer [`Execs::with_stdout_data`] where possible.
    /// - `with` cannot be snapshotted
    /// - The absence of `without` can either mean success or that the string being looked for
    ///   changed.
    ///
    /// </div>
    ///
    /// # Example
    ///
    /// ```no_run
    /// use cargo_test_support::execs;
    ///
    /// execs().with_stderr_line_without(
    ///     &[
    ///         "[RUNNING] `rustc --crate-name build_script_build",
    ///         "-C opt-level=3",
    ///     ],
    ///     &["-C debuginfo", "-C incremental"],
    /// );
    /// ```
    ///
    /// This will check that a build line includes `-C opt-level=3` but does
    /// not contain `-C debuginfo` or `-C incremental`.
    ///
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
}

/// # Configure the process
impl Execs {
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

    pub fn args<T: AsRef<OsStr>>(&mut self, args: &[T]) -> &mut Self {
        if let Some(ref mut p) = self.process_builder {
            p.args(args);
        }
        self
    }

    pub fn cwd<T: AsRef<OsStr>>(&mut self, path: T) -> &mut Self {
        if let Some(ref mut p) = self.process_builder {
            if let Some(cwd) = p.get_cwd() {
                let new_path = cwd.join(path.as_ref());
                p.cwd(new_path);
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

    /// Enables nightly features for testing
    ///
    /// The list of reasons should be why nightly cargo is needed. If it is
    /// because of an unstable feature put the name of the feature as the reason,
    /// e.g. `&["print-im-a-teapot"]`
    pub fn masquerade_as_nightly_cargo(&mut self, reasons: &[&str]) -> &mut Self {
        if let Some(ref mut p) = self.process_builder {
            p.masquerade_as_nightly_cargo(reasons);
        }
        self
    }

    /// Overrides the crates.io URL for testing.
    ///
    /// Can be used for testing crates-io functionality where alt registries
    /// cannot be used.
    pub fn replace_crates_io(&mut self, url: &Url) -> &mut Self {
        if let Some(ref mut p) = self.process_builder {
            p.env("__CARGO_TEST_CRATES_IO_URL_DO_NOT_USE_THIS", url.as_str());
        }
        self
    }

    pub fn overlay_registry(&mut self, url: &Url, path: &str) -> &mut Self {
        if let Some(ref mut p) = self.process_builder {
            let env_value = format!("{}={}", url, path);
            p.env(
                "__CARGO_TEST_DEPENDENCY_CONFUSION_VULNERABILITY_DO_NOT_USE_THIS",
                env_value,
            );
        }
        self
    }

    pub fn enable_split_debuginfo_packed(&mut self) -> &mut Self {
        self.env("CARGO_PROFILE_DEV_SPLIT_DEBUGINFO", "packed")
            .env("CARGO_PROFILE_TEST_SPLIT_DEBUGINFO", "packed")
            .env("CARGO_PROFILE_RELEASE_SPLIT_DEBUGINFO", "packed")
            .env("CARGO_PROFILE_BENCH_SPLIT_DEBUGINFO", "packed");
        self
    }

    pub fn enable_mac_dsym(&mut self) -> &mut Self {
        if cfg!(target_os = "macos") {
            return self.enable_split_debuginfo_packed();
        }
        self
    }
}

/// # Run and verify the process
impl Execs {
    pub fn exec_with_output(&mut self) -> Result<Output> {
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

    #[track_caller]
    pub fn run(&mut self) -> RawOutput {
        self.ran = true;
        let mut p = (&self.process_builder).clone().unwrap();
        if let Some(stdin) = self.expect_stdin.take() {
            p.stdin(stdin);
        }

        match self.match_process(&p) {
            Err(e) => panic_error(&format!("test failed running {}", p), e),
            Ok(output) => output,
        }
    }

    /// Runs the process, checks the expected output, and returns the first
    /// JSON object on stdout.
    #[track_caller]
    pub fn run_json(&mut self) -> serde_json::Value {
        let output = self.run();
        serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
            panic!(
                "\nfailed to parse JSON: {}\n\
                     output was:\n{}\n",
                e,
                String::from_utf8_lossy(&output.stdout)
            );
        })
    }

    #[track_caller]
    pub fn run_output(&mut self, output: &Output) {
        self.ran = true;
        if let Err(e) = self.match_output(output.status.code(), &output.stdout, &output.stderr) {
            panic_error("process did not return the expected result", e)
        }
    }

    #[track_caller]
    fn verify_checks_output(&self, stdout: &[u8], stderr: &[u8]) {
        if self.expect_exit_code.unwrap_or(0) != 0
            && self.expect_stdin.is_none()
            && self.expect_stdout_data.is_none()
            && self.expect_stderr_data.is_none()
            && self.expect_stdout_contains.is_empty()
            && self.expect_stderr_contains.is_empty()
            && self.expect_stdout_not_contains.is_empty()
            && self.expect_stderr_not_contains.is_empty()
            && self.expect_stderr_with_without.is_empty()
        {
            panic!(
                "`with_status()` is used, but no output is checked.\n\
                 The test must check the output to ensure the correct error is triggered.\n\
                 --- stdout\n{}\n--- stderr\n{}",
                String::from_utf8_lossy(stdout),
                String::from_utf8_lossy(stderr),
            );
        }
    }

    #[track_caller]
    fn match_process(&self, process: &ProcessBuilder) -> Result<RawOutput> {
        println!("running {}", process);
        let res = if self.stream_output {
            if is_ci() {
                panic!("`.stream()` is for local debugging")
            }
            process.exec_with_streaming(
                &mut |out| {
                    println!("{}", out);
                    Ok(())
                },
                &mut |err| {
                    eprintln!("{}", err);
                    Ok(())
                },
                true,
            )
        } else {
            process.exec_with_output()
        };

        match res {
            Ok(out) => {
                self.match_output(out.status.code(), &out.stdout, &out.stderr)?;
                return Ok(RawOutput {
                    stdout: out.stdout,
                    stderr: out.stderr,
                    code: out.status.code(),
                });
            }
            Err(e) => {
                if let Some(ProcessError {
                    stdout: Some(stdout),
                    stderr: Some(stderr),
                    code,
                    ..
                }) = e.downcast_ref::<ProcessError>()
                {
                    self.match_output(*code, stdout, stderr)?;
                    return Ok(RawOutput {
                        stdout: stdout.to_vec(),
                        stderr: stderr.to_vec(),
                        code: *code,
                    });
                }
                bail!("could not exec process {}: {:?}", process, e)
            }
        }
    }

    #[track_caller]
    fn match_output(&self, code: Option<i32>, stdout: &[u8], stderr: &[u8]) -> Result<()> {
        self.verify_checks_output(stdout, stderr);
        let stdout = std::str::from_utf8(stdout).expect("stdout is not utf8");
        let stderr = std::str::from_utf8(stderr).expect("stderr is not utf8");

        match self.expect_exit_code {
            None => {}
            Some(expected) if code == Some(expected) => {}
            Some(expected) => bail!(
                "process exited with code {} (expected {})\n--- stdout\n{}\n--- stderr\n{}",
                code.unwrap_or(-1),
                expected,
                stdout,
                stderr
            ),
        }

        if let Some(expect_stdout_data) = &self.expect_stdout_data {
            if let Err(err) = self.assert.try_eq(
                Some(&"stdout"),
                stdout.into_data(),
                expect_stdout_data.clone(),
            ) {
                panic!("{err}")
            }
        }
        if let Some(expect_stderr_data) = &self.expect_stderr_data {
            if let Err(err) = self.assert.try_eq(
                Some(&"stderr"),
                stderr.into_data(),
                expect_stderr_data.clone(),
            ) {
                panic!("{err}")
            }
        }
        for expect in self.expect_stdout_contains.iter() {
            compare::match_contains(expect, stdout, self.assert.redactions())?;
        }
        for expect in self.expect_stderr_contains.iter() {
            compare::match_contains(expect, stderr, self.assert.redactions())?;
        }
        for expect in self.expect_stdout_not_contains.iter() {
            compare::match_does_not_contain(expect, stdout, self.assert.redactions())?;
        }
        for expect in self.expect_stderr_not_contains.iter() {
            compare::match_does_not_contain(expect, stderr, self.assert.redactions())?;
        }
        for (with, without) in self.expect_stderr_with_without.iter() {
            compare::match_with_without(stderr, with, without, self.assert.redactions())?;
        }
        Ok(())
    }
}

impl Drop for Execs {
    fn drop(&mut self) {
        if !self.ran && !std::thread::panicking() {
            panic!("forgot to run this command");
        }
    }
}

/// Run and verify a process, see [`Execs`]
pub fn execs() -> Execs {
    Execs {
        ran: false,
        process_builder: None,
        expect_stdin: None,
        expect_exit_code: Some(0),
        expect_stdout_data: None,
        expect_stderr_data: None,
        expect_stdout_contains: Vec::new(),
        expect_stderr_contains: Vec::new(),
        expect_stdout_not_contains: Vec::new(),
        expect_stderr_not_contains: Vec::new(),
        expect_stderr_with_without: Vec::new(),
        stream_output: false,
        assert: compare::assert_e2e(),
    }
}

/// Generate a basic `Cargo.toml`
pub fn basic_manifest(name: &str, version: &str) -> String {
    format!(
        r#"
        [package]
        name = "{}"
        version = "{}"
        authors = []
        edition = "2015"
    "#,
        name, version
    )
}

/// Generate a `Cargo.toml` with the specified `bin.name`
pub fn basic_bin_manifest(name: &str) -> String {
    format!(
        r#"
        [package]

        name = "{}"
        version = "0.5.0"
        authors = ["wycats@example.com"]
        edition = "2015"

        [[bin]]

        name = "{}"
    "#,
        name, name
    )
}

/// Generate a `Cargo.toml` with the specified `lib.name`
pub fn basic_lib_manifest(name: &str) -> String {
    format!(
        r#"
        [package]

        name = "{}"
        version = "0.5.0"
        authors = ["wycats@example.com"]
        edition = "2015"

        [lib]

        name = "{}"
    "#,
        name, name
    )
}

/// Gets a valid target spec JSON from rustc.
///
/// To avoid any hardcoded value, this fetches `x86_64-unknown-none` target
/// spec JSON directly from `rustc`, as Cargo shouldn't know the JSON schema.
pub fn target_spec_json() -> &'static str {
    static TARGET_SPEC_JSON: LazyLock<String> = LazyLock::new(|| {
        let json = std::process::Command::new("rustc")
            .env("RUSTC_BOOTSTRAP", "1")
            .arg("--print")
            .arg("target-spec-json")
            .arg("-Zunstable-options")
            .arg("--target")
            .arg("x86_64-unknown-none")
            .output()
            .expect("rustc --print target-spec-json")
            .stdout;
        String::from_utf8(json).expect("utf8 target spec json")
    });

    TARGET_SPEC_JSON.as_str()
}

struct RustcInfo {
    verbose_version: String,
    host: String,
}

impl RustcInfo {
    fn new() -> RustcInfo {
        let output = ProcessBuilder::new("rustc")
            .arg("-vV")
            .exec_with_output()
            .expect("rustc should exec");
        let verbose_version = String::from_utf8(output.stdout).expect("utf8 output");
        let host = verbose_version
            .lines()
            .filter_map(|line| line.strip_prefix("host: "))
            .next()
            .expect("verbose version has host: field")
            .to_string();
        RustcInfo {
            verbose_version,
            host,
        }
    }
}

fn rustc_info() -> &'static RustcInfo {
    static RUSTC_INFO: OnceLock<RustcInfo> = OnceLock::new();
    RUSTC_INFO.get_or_init(RustcInfo::new)
}

/// The rustc host such as `x86_64-unknown-linux-gnu`.
pub fn rustc_host() -> &'static str {
    &rustc_info().host
}

/// The host triple suitable for use in a cargo environment variable (uppercased).
pub fn rustc_host_env() -> String {
    rustc_host().to_uppercase().replace('-', "_")
}

pub fn is_nightly() -> bool {
    let vv = &rustc_info().verbose_version;
    // CARGO_TEST_DISABLE_NIGHTLY is set in rust-lang/rust's CI so that all
    // nightly-only tests are disabled there. Otherwise, it could make it
    // difficult to land changes which would need to be made simultaneously in
    // rust-lang/cargo and rust-lan/rust, which isn't possible.
    env::var("CARGO_TEST_DISABLE_NIGHTLY").is_err()
        && (vv.contains("-nightly") || vv.contains("-dev"))
}

/// Run `$bin` in the test's environment, see [`ProcessBuilder`]
///
/// For more on the test environment, see
/// - [`paths::root`]
/// - [`TestEnvCommandExt`]
pub fn process<T: AsRef<OsStr>>(bin: T) -> ProcessBuilder {
    _process(bin.as_ref())
}

fn _process(t: &OsStr) -> ProcessBuilder {
    let mut p = ProcessBuilder::new(t);
    p.cwd(&paths::root()).test_env();
    p
}

/// Enable nightly features for testing
pub trait ChannelChangerCommandExt {
    /// The list of reasons should be why nightly cargo is needed. If it is
    /// because of an unstable feature put the name of the feature as the reason,
    /// e.g. `&["print-im-a-teapot"]`.
    fn masquerade_as_nightly_cargo(self, _reasons: &[&str]) -> Self;
}

impl ChannelChangerCommandExt for &mut ProcessBuilder {
    fn masquerade_as_nightly_cargo(self, _reasons: &[&str]) -> Self {
        self.env("__CARGO_TEST_CHANNEL_OVERRIDE_DO_NOT_USE_THIS", "nightly")
    }
}

impl ChannelChangerCommandExt for snapbox::cmd::Command {
    fn masquerade_as_nightly_cargo(self, _reasons: &[&str]) -> Self {
        self.env("__CARGO_TEST_CHANNEL_OVERRIDE_DO_NOT_USE_THIS", "nightly")
    }
}

/// Establish a process's test environment
pub trait TestEnvCommandExt: Sized {
    fn test_env(mut self) -> Self {
        // In general just clear out all cargo-specific configuration already in the
        // environment. Our tests all assume a "default configuration" unless
        // specified otherwise.
        for (k, _v) in env::vars() {
            if k.starts_with("CARGO_") {
                self = self.env_remove(&k);
            }
        }
        if env::var_os("RUSTUP_TOOLCHAIN").is_some() {
            // Override the PATH to avoid executing the rustup wrapper thousands
            // of times. This makes the testsuite run substantially faster.
            static RUSTC_DIR: OnceLock<PathBuf> = OnceLock::new();
            let rustc_dir = RUSTC_DIR.get_or_init(|| {
                match ProcessBuilder::new("rustup")
                    .args(&["which", "rustc"])
                    .exec_with_output()
                {
                    Ok(output) => {
                        let s = std::str::from_utf8(&output.stdout).expect("utf8").trim();
                        let mut p = PathBuf::from(s);
                        p.pop();
                        p
                    }
                    Err(e) => {
                        panic!("RUSTUP_TOOLCHAIN was set, but could not run rustup: {}", e);
                    }
                }
            });
            let path = env::var_os("PATH").unwrap_or_default();
            let paths = env::split_paths(&path);
            let new_path =
                env::join_paths(std::iter::once(rustc_dir.clone()).chain(paths)).unwrap();
            self = self.env("PATH", new_path);
        }

        self = self
            .current_dir(&paths::root())
            .env("HOME", paths::home())
            .env("CARGO_HOME", paths::cargo_home())
            .env("__CARGO_TEST_ROOT", paths::global_root())
            // Force Cargo to think it's on the stable channel for all tests, this
            // should hopefully not surprise us as we add cargo features over time and
            // cargo rides the trains.
            .env("__CARGO_TEST_CHANNEL_OVERRIDE_DO_NOT_USE_THIS", "stable")
            // Keeps cargo within its sandbox.
            .env("__CARGO_TEST_DISABLE_GLOBAL_KNOWN_HOST", "1")
            // Set retry sleep to 1 millisecond.
            .env("__CARGO_TEST_FIXED_RETRY_SLEEP_MS", "1")
            // Incremental generates a huge amount of data per test, which we
            // don't particularly need. Tests that specifically need to check
            // the incremental behavior should turn this back on.
            .env("CARGO_INCREMENTAL", "0")
            // Don't read the system git config which is out of our control.
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env_remove("__CARGO_DEFAULT_LIB_METADATA")
            .env_remove("ALL_PROXY")
            .env_remove("EMAIL")
            .env_remove("GIT_AUTHOR_EMAIL")
            .env_remove("GIT_AUTHOR_NAME")
            .env_remove("GIT_COMMITTER_EMAIL")
            .env_remove("GIT_COMMITTER_NAME")
            .env_remove("http_proxy")
            .env_remove("HTTPS_PROXY")
            .env_remove("https_proxy")
            .env_remove("MAKEFLAGS")
            .env_remove("MFLAGS")
            .env_remove("MSYSTEM") // assume cmd.exe everywhere on windows
            .env_remove("RUSTC")
            .env_remove("RUST_BACKTRACE")
            .env_remove("RUSTC_WORKSPACE_WRAPPER")
            .env_remove("RUSTC_WRAPPER")
            .env_remove("RUSTDOC")
            .env_remove("RUSTDOCFLAGS")
            .env_remove("RUSTFLAGS")
            .env_remove("SSH_AUTH_SOCK") // ensure an outer agent is never contacted
            .env_remove("USER") // not set on some rust-lang docker images
            .env_remove("XDG_CONFIG_HOME") // see #2345
            .env_remove("OUT_DIR"); // see #13204
        if cfg!(windows) {
            self = self.env("USERPROFILE", paths::home());
        }
        self
    }

    fn current_dir<S: AsRef<std::path::Path>>(self, path: S) -> Self;
    fn env<S: AsRef<std::ffi::OsStr>>(self, key: &str, value: S) -> Self;
    fn env_remove(self, key: &str) -> Self;
}

impl TestEnvCommandExt for &mut ProcessBuilder {
    fn current_dir<S: AsRef<std::path::Path>>(self, path: S) -> Self {
        let path = path.as_ref();
        self.cwd(path)
    }
    fn env<S: AsRef<std::ffi::OsStr>>(self, key: &str, value: S) -> Self {
        self.env(key, value)
    }
    fn env_remove(self, key: &str) -> Self {
        self.env_remove(key)
    }
}

impl TestEnvCommandExt for snapbox::cmd::Command {
    fn current_dir<S: AsRef<std::path::Path>>(self, path: S) -> Self {
        self.current_dir(path)
    }
    fn env<S: AsRef<std::ffi::OsStr>>(self, key: &str, value: S) -> Self {
        self.env(key, value)
    }
    fn env_remove(self, key: &str) -> Self {
        self.env_remove(key)
    }
}

/// Add a list of arguments as a line
pub trait ArgLineCommandExt: Sized {
    fn arg_line(mut self, s: &str) -> Self {
        for mut arg in s.split_whitespace() {
            if (arg.starts_with('"') && arg.ends_with('"'))
                || (arg.starts_with('\'') && arg.ends_with('\''))
            {
                arg = &arg[1..(arg.len() - 1).max(1)];
            } else if arg.contains(&['"', '\''][..]) {
                panic!("shell-style argument parsing is not supported")
            }
            self = self.arg(arg);
        }
        self
    }

    fn arg<S: AsRef<std::ffi::OsStr>>(self, s: S) -> Self;
}

impl ArgLineCommandExt for &mut ProcessBuilder {
    fn arg<S: AsRef<std::ffi::OsStr>>(self, s: S) -> Self {
        self.arg(s)
    }
}

impl ArgLineCommandExt for &mut Execs {
    fn arg<S: AsRef<std::ffi::OsStr>>(self, s: S) -> Self {
        self.arg(s)
    }
}

impl ArgLineCommandExt for snapbox::cmd::Command {
    fn arg<S: AsRef<std::ffi::OsStr>>(self, s: S) -> Self {
        self.arg(s)
    }
}

/// Run `git $arg_line`, see [`ProcessBuilder`]
pub fn git_process(arg_line: &str) -> ProcessBuilder {
    let mut p = process("git");
    p.arg_line(arg_line);
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
    cfg!(target_os = "macos") && is_ci()
}

/// A way for to increase the cut off for all the time based test.
///
/// Some CI setups are much slower then the equipment used by Cargo itself.
/// Architectures that do not have a modern processor, hardware emulation, etc.
pub fn slow_cpu_multiplier(main: u64) -> Duration {
    static SLOW_CPU_MULTIPLIER: OnceLock<u64> = OnceLock::new();
    let slow_cpu_multiplier = SLOW_CPU_MULTIPLIER.get_or_init(|| {
        env::var("CARGO_TEST_SLOW_CPU_MULTIPLIER")
            .ok()
            .and_then(|m| m.parse().ok())
            .unwrap_or(1)
    });
    Duration::from_secs(slow_cpu_multiplier * main)
}

#[cfg(windows)]
pub fn symlink_supported() -> bool {
    if is_ci() {
        // We want to be absolutely sure this runs on CI.
        return true;
    }
    let src = paths::root().join("symlink_src");
    fs::write(&src, "").unwrap();
    let dst = paths::root().join("symlink_dst");
    let result = match os::windows::fs::symlink_file(&src, &dst) {
        Ok(_) => {
            fs::remove_file(&dst).unwrap();
            true
        }
        Err(e) => {
            eprintln!(
                "symlinks not supported: {:?}\n\
                 Windows 10 users should enable developer mode.",
                e
            );
            false
        }
    };
    fs::remove_file(&src).unwrap();
    return result;
}

#[cfg(not(windows))]
pub fn symlink_supported() -> bool {
    true
}

/// The error message for ENOENT.
pub fn no_such_file_err_msg() -> String {
    std::io::Error::from_raw_os_error(2).to_string()
}

/// Helper to retry a function `n` times.
///
/// The function should return `Some` when it is ready.
#[track_caller]
pub fn retry<F, R>(n: u32, mut f: F) -> R
where
    F: FnMut() -> Option<R>,
{
    let mut count = 0;
    let start = std::time::Instant::now();
    loop {
        if let Some(r) = f() {
            return r;
        }
        count += 1;
        if count > n {
            panic!(
                "test did not finish within {n} attempts ({:?} total)",
                start.elapsed()
            );
        }
        sleep_ms(100);
    }
}

#[test]
#[should_panic(expected = "test did not finish")]
fn retry_fails() {
    retry(2, || None::<()>);
}

/// Helper that waits for a thread to finish, up to `n` tenths of a second.
#[track_caller]
pub fn thread_wait_timeout<T>(n: u32, thread: JoinHandle<T>) -> T {
    retry(n, || thread.is_finished().then_some(()));
    thread.join().unwrap()
}

/// Helper that runs some function, and waits up to `n` tenths of a second for
/// it to finish.
#[track_caller]
pub fn threaded_timeout<F, R>(n: u32, f: F) -> R
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    let thread = std::thread::spawn(|| f());
    thread_wait_timeout(n, thread)
}

// Helper for testing dep-info files in the fingerprint dir.
#[track_caller]
pub fn assert_deps(project: &Project, fingerprint: &str, test_cb: impl Fn(&Path, &[(u8, &str)])) {
    let mut files = project
        .glob(fingerprint)
        .map(|f| f.expect("unwrap glob result"))
        // Filter out `.json` entries.
        .filter(|f| f.extension().is_none());
    let info_path = files
        .next()
        .unwrap_or_else(|| panic!("expected 1 dep-info file at {}, found 0", fingerprint));
    assert!(files.next().is_none(), "expected only 1 dep-info file");
    let dep_info = fs::read(&info_path).unwrap();
    let dep_info = &mut &dep_info[..];

    // Consume the magic marker and version. Here they don't really matter.
    read_usize(dep_info);
    read_u8(dep_info);
    read_u8(dep_info);

    let deps = (0..read_usize(dep_info))
        .map(|_| {
            let ty = read_u8(dep_info);
            let path = std::str::from_utf8(read_bytes(dep_info)).unwrap();
            let checksum_present = read_bool(dep_info);
            if checksum_present {
                // Read out the checksum info without using it
                let _file_len = read_u64(dep_info);
                let _checksum = read_bytes(dep_info);
            }
            (ty, path)
        })
        .collect::<Vec<_>>();
    test_cb(&info_path, &deps);

    fn read_usize(bytes: &mut &[u8]) -> usize {
        let ret = &bytes[..4];
        *bytes = &bytes[4..];

        u32::from_le_bytes(ret.try_into().unwrap()) as usize
    }

    fn read_u8(bytes: &mut &[u8]) -> u8 {
        let ret = bytes[0];
        *bytes = &bytes[1..];
        ret
    }

    fn read_bool(bytes: &mut &[u8]) -> bool {
        read_u8(bytes) != 0
    }

    fn read_u64(bytes: &mut &[u8]) -> u64 {
        let ret = &bytes[..8];
        *bytes = &bytes[8..];

        u64::from_le_bytes(ret.try_into().unwrap())
    }

    fn read_bytes<'a>(bytes: &mut &'a [u8]) -> &'a [u8] {
        let n = read_usize(bytes);
        let ret = &bytes[..n];
        *bytes = &bytes[n..];
        ret
    }
}

#[track_caller]
pub fn assert_deps_contains(project: &Project, fingerprint: &str, expected: &[(u8, &str)]) {
    assert_deps(project, fingerprint, |info_path, entries| {
        for (e_kind, e_path) in expected {
            let pattern = glob::Pattern::new(e_path).unwrap();
            let count = entries
                .iter()
                .filter(|(kind, path)| kind == e_kind && pattern.matches(path))
                .count();
            if count != 1 {
                panic!(
                    "Expected 1 match of {} {} in {:?}, got {}:\n{:#?}",
                    e_kind, e_path, info_path, count, entries
                );
            }
        }
    })
}
