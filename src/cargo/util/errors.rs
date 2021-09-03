#![allow(unknown_lints)]

use crate::core::{TargetKind, Workspace};
use crate::ops::CompileOptions;
use anyhow::Error;
use cargo_util::ProcessError;
use itertools::Itertools;
use lazy_static::lazy_static;
use std::fmt;
use std::path::PathBuf;

pub type CargoResult<T> = anyhow::Result<T>;

#[derive(Debug)]
pub struct HttpNot200 {
    pub code: u32,
    pub url: String,
}

impl fmt::Display for HttpNot200 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "failed to get 200 response from `{}`, got {}",
            self.url, self.code
        )
    }
}

impl std::error::Error for HttpNot200 {}

// =============================================================================
// Verbose error

/// An error wrapper for errors that should only be displayed with `--verbose`.
///
/// This should only be used in rare cases. When emitting this error, you
/// should have a normal error higher up the error-cause chain (like "could
/// not compile `foo`"), so at least *something* gets printed without
/// `--verbose`.
pub struct VerboseError {
    inner: Error,
}

impl VerboseError {
    pub fn new(inner: Error) -> VerboseError {
        VerboseError { inner }
    }
}

impl std::error::Error for VerboseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.inner.source()
    }
}

impl fmt::Debug for VerboseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl fmt::Display for VerboseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

// =============================================================================
// Internal error

/// An unexpected, internal error.
///
/// This should only be used for unexpected errors. It prints a message asking
/// the user to file a bug report.
pub struct InternalError {
    inner: Error,
}

impl InternalError {
    pub fn new(inner: Error) -> InternalError {
        InternalError { inner }
    }
}

impl std::error::Error for InternalError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.inner.source()
    }
}

impl fmt::Debug for InternalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl fmt::Display for InternalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

// =============================================================================
// Manifest error

/// Error wrapper related to a particular manifest and providing it's path.
///
/// This error adds no displayable info of it's own.
pub struct ManifestError {
    cause: Error,
    manifest: PathBuf,
}

impl ManifestError {
    pub fn new<E: Into<Error>>(cause: E, manifest: PathBuf) -> Self {
        Self {
            cause: cause.into(),
            manifest,
        }
    }

    pub fn manifest_path(&self) -> &PathBuf {
        &self.manifest
    }

    /// Returns an iterator over the `ManifestError` chain of causes.
    ///
    /// So if this error was not caused by another `ManifestError` this will be empty.
    pub fn manifest_causes(&self) -> ManifestCauses<'_> {
        ManifestCauses { current: self }
    }
}

impl std::error::Error for ManifestError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.cause.source()
    }
}

impl fmt::Debug for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.cause.fmt(f)
    }
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.cause.fmt(f)
    }
}

/// An iterator over the `ManifestError` chain of causes.
pub struct ManifestCauses<'a> {
    current: &'a ManifestError,
}

impl<'a> Iterator for ManifestCauses<'a> {
    type Item = &'a ManifestError;

    fn next(&mut self) -> Option<Self::Item> {
        self.current = self.current.cause.downcast_ref()?;
        Some(self.current)
    }
}

impl<'a> ::std::iter::FusedIterator for ManifestCauses<'a> {}

// =============================================================================
// Cargo test errors.

/// Error when testcases fail
#[derive(Debug)]
pub struct CargoTestError {
    pub test: Test,
    pub desc: String,
    pub code: Option<i32>,
    pub causes: Vec<ProcessError>,
}

impl fmt::Display for CargoTestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.desc.fmt(f)
    }
}

impl std::error::Error for CargoTestError {}

#[derive(Debug)]
pub enum Test {
    Multiple,
    Doc,
    UnitTest {
        kind: TargetKind,
        name: String,
        pkg_name: String,
    },
}

impl CargoTestError {
    pub fn new(test: Test, errors: Vec<ProcessError>) -> Self {
        if errors.is_empty() {
            panic!("Cannot create CargoTestError from empty Vec")
        }
        let desc = errors
            .iter()
            .map(|error| error.desc.clone())
            .collect::<Vec<String>>()
            .join("\n");
        CargoTestError {
            test,
            desc,
            code: errors[0].code,
            causes: errors,
        }
    }

    pub fn hint(&self, ws: &Workspace<'_>, opts: &CompileOptions) -> String {
        match self.test {
            Test::UnitTest {
                ref kind,
                ref name,
                ref pkg_name,
            } => {
                let pkg_info = if opts.spec.needs_spec_flag(ws) {
                    format!("-p {} ", pkg_name)
                } else {
                    String::new()
                };

                match *kind {
                    TargetKind::Bench => {
                        format!("test failed, to rerun pass '{}--bench {}'", pkg_info, name)
                    }
                    TargetKind::Bin => {
                        format!("test failed, to rerun pass '{}--bin {}'", pkg_info, name)
                    }
                    TargetKind::Lib(_) => format!("test failed, to rerun pass '{}--lib'", pkg_info),
                    TargetKind::Test => {
                        format!("test failed, to rerun pass '{}--test {}'", pkg_info, name)
                    }
                    TargetKind::ExampleBin | TargetKind::ExampleLib(_) => {
                        format!("test failed, to rerun pass '{}--example {}", pkg_info, name)
                    }
                    _ => "test failed.".into(),
                }
            }
            Test::Doc => "test failed, to rerun pass '--doc'".into(),
            _ => "test failed.".into(),
        }
    }
}

// =============================================================================
// CLI errors

pub type CliResult = Result<(), CliError>;

#[derive(Debug)]
/// The CLI error is the error type used at Cargo's CLI-layer.
///
/// All errors from the lib side of Cargo will get wrapped with this error.
/// Other errors (such as command-line argument validation) will create this
/// directly.
pub struct CliError {
    /// The error to display. This can be `None` in rare cases to exit with a
    /// code without displaying a message. For example `cargo run -q` where
    /// the resulting process exits with a nonzero code (on Windows), or an
    /// external subcommand that exits nonzero (we assume it printed its own
    /// message).
    pub error: Option<anyhow::Error>,
    /// The process exit code.
    pub exit_code: i32,
}

impl CliError {
    pub fn new(error: anyhow::Error, code: i32) -> CliError {
        CliError {
            error: Some(error),
            exit_code: code,
        }
    }

    pub fn code(code: i32) -> CliError {
        CliError {
            error: None,
            exit_code: code,
        }
    }
}

impl From<anyhow::Error> for CliError {
    fn from(err: anyhow::Error) -> CliError {
        CliError::new(err, 101)
    }
}

impl From<clap::Error> for CliError {
    fn from(err: clap::Error) -> CliError {
        let code = if err.use_stderr() { 1 } else { 0 };
        CliError::new(err.into(), code)
    }
}

/// Error that comes from running `git` CLI. This wrapper exists to detect
/// this kind of errors in [`crate::util::network::with_retry`] and properly
/// retry them.
pub struct GitCliError {
    inner: anyhow::Error,
}

impl GitCliError {
    /// Creates an insteance of [`GitCliError`].
    ///
    /// # Invariants
    ///
    /// The inner error must by constructed from [`ProcessError`] that
    /// was returned as a result of running `git` CLI operation.
    /// If it isn't true, the code won't panic, but [`Self::is_retryable()`]
    /// will always return `false`.
    pub fn new(inner: anyhow::Error) -> Self {
        Self { inner }
    }

    /// Returns `true` if the git CLI error is transient and may be retried.
    ///
    /// The code here is inspired by `chromium` git retries code:
    /// <https://chromium.googlesource.com/infra/infra/+/b9f6e35d2aa1ce9fcb385e414866e0b1635a6c77/go/src/infra/tools/git/retry_regexp.go#17>
    pub fn is_retryable(&self) -> bool {
        const GIT_TRANSIENT_ERRORS: &[&str] = &[
            // Error patterns are taken from chromium.
            // Please, keep them 1-to-1 exactly the same as in their source code
            //
            // If you are updating them, please keep the commit ref in the doc comment
            // of this method up-to-date.
            r#"!.*\[remote rejected\].*\(error in hook\)"#,
            r#"!.*\[remote rejected\].*\(failed to lock\)"#,
            r#"!.*\[remote rejected\].*\(error in Gerrit backend\)"#,
            r#"remote error: Internal Server Error"#,
            r#"fatal: Couldn't find remote ref "#,
            r#"git fetch_pack: expected ACK/NAK, got"#,
            r#"protocol error: bad pack header"#,
            r#"The remote end hung up unexpectedly"#,
            r#"The remote end hung up upon initial contact"#,
            r#"TLS packet with unexpected length was received"#,
            r#"RPC failed; result=\d+, HTTP code = \d+"#,
            r#"Connection timed out"#,
            r#"repository cannot accept new pushes; contact support"#,
            r#"Service Temporarily Unavailable"#,
            r#"The service is currently unavailable"#,
            r#"Connection refused"#,
            r#"The requested URL returned error: 5\d+"#,
            r#"Operation too slow"#,
            r#"Connection reset by peer"#,
            r#"Unable to look up"#,
            r#"Couldn't resolve host"#,
            r#"Unknown SSL protocol error"#,
            r#"Revision .* of patch set \d+ does not match refs/changes"#,
            r#"Git repository not found"#,
            r#"Couldn't connect to server"#,
            r#"transfer closed with outstanding read data remaining"#,
            r#"Access denied to"#,
            r#"The requested URL returned error: 429"#,
            r#"RESOURCE_EXHAUSTED"#,
            r#"Resource has been exhausted"#,
            r#"check quota"#,
            r#"fetch-pack: protocol error: bad band #\d+"#,
            r#"The requested URL returned error: 400"#,
            r#"fetch-pack: fetch failed"#,
            r#"fetch-pack: unable to spawn http-fetch"#,
            r#"fetch-pack: expected keep then TAB at start of http-fetch output"#,
            r#"fetch-pack: expected hash then LF at end of http-fetch output"#,
            r#"fetch-pack: unable to finish http-fetch"#,
            r#"fetch-pack: pack downloaded from .* does not match expected hash .*"#,
            // Regexes that are not included in chromium source code
            // This one was reported in https://github.com/rust-lang/cargo/issues/9820
            r#"Failed to connect .* Timed out"#,
        ];

        /// Merge all of the regex into a single regex OR-ed together
        fn merge_regex(regexps: &[&str]) -> regex::Regex {
            let source = regexps
                .iter()
                .format_with("|", |regex, f| f(&format_args!("(?:{})", regex)));

            let source = &format!("(?i){}", source);

            regex::Regex::new(source).expect("BUG: invalid git transient error regex")
        }

        lazy_static! {
            static ref GIT_TRANSIENT_ERRORS_RE: regex::Regex = merge_regex(GIT_TRANSIENT_ERRORS);
        }

        let err = match self.inner.downcast_ref::<ProcessError>() {
            Some(err) => err,
            None => {
                log::warn!(
                    "BUG: `GitCliError` was constructed from non-`ProcessError`, \
                    can't determine if it's retryable, assuming it's not... Error: {:?}",
                    self,
                );
                return false;
            }
        };

        err.stderr
            .as_deref()
            .map(|stderr| GIT_TRANSIENT_ERRORS_RE.is_match(&String::from_utf8_lossy(stderr)))
            .unwrap_or_else(|| {
                log::warn!(
                    "BUG: git CLI stderr wasn't captured, \
                    can't determine if it's retryable, assuming it's not... Error: {:?}",
                    err
                );
                false
            })
    }
}

impl std::error::Error for GitCliError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.inner.source()
    }
}

impl fmt::Debug for GitCliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl fmt::Display for GitCliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

// =============================================================================
// Construction helpers

pub fn internal<S: fmt::Display>(error: S) -> anyhow::Error {
    InternalError::new(anyhow::format_err!("{}", error)).into()
}
