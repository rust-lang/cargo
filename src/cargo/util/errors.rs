#![allow(unknown_lints)]

use anyhow::Error;
use curl::easy::Easy;
use std::fmt::{self, Write};
use std::path::PathBuf;

use super::truncate_with_ellipsis;

pub type CargoResult<T> = anyhow::Result<T>;

/// These are headers that are included in error messages to help with
/// diagnosing issues.
pub const DEBUG_HEADERS: &[&str] = &[
    // This is the unique ID that identifies the request in CloudFront which
    // can be used for looking at the AWS logs.
    "x-amz-cf-id",
    // This is the CloudFront POP (Point of Presence) that identifies the
    // region where the request was routed. This can help identify if an issue
    // is region-specific.
    "x-amz-cf-pop",
    // The unique token used for troubleshooting S3 requests via AWS logs or support.
    "x-amz-request-id",
    // Another token used in conjunction with x-amz-request-id.
    "x-amz-id-2",
    // Whether or not there was a cache hit or miss (both CloudFront and Fastly).
    "x-cache",
    // The cache server that processed the request (Fastly).
    "x-served-by",
];

#[derive(Debug)]
pub struct HttpNotSuccessful {
    pub code: u32,
    pub url: String,
    pub ip: Option<String>,
    pub body: Vec<u8>,
    pub headers: Vec<String>,
}

impl HttpNotSuccessful {
    pub fn new_from_handle(
        handle: &mut Easy,
        initial_url: &str,
        body: Vec<u8>,
        headers: Vec<String>,
    ) -> HttpNotSuccessful {
        let ip = handle.primary_ip().ok().flatten().map(|s| s.to_string());
        let url = handle
            .effective_url()
            .ok()
            .flatten()
            .unwrap_or(initial_url)
            .to_string();
        HttpNotSuccessful {
            code: handle.response_code().unwrap_or(0),
            url,
            ip,
            body,
            headers,
        }
    }

    /// Renders the error in a compact form.
    pub fn display_short(&self) -> String {
        self.render(false)
    }

    fn render(&self, show_headers: bool) -> String {
        let mut result = String::new();
        let body = std::str::from_utf8(&self.body)
            .map(|s| truncate_with_ellipsis(s, 512))
            .unwrap_or_else(|_| format!("[{} non-utf8 bytes]", self.body.len()));

        write!(
            result,
            "failed to get successful HTTP response from `{}`",
            self.url
        )
        .unwrap();
        if let Some(ip) = &self.ip {
            write!(result, " ({ip})").unwrap();
        }
        write!(result, ", got {}\n", self.code).unwrap();
        if show_headers {
            let headers: Vec<_> = self
                .headers
                .iter()
                .filter(|header| {
                    let Some((name, _)) = header.split_once(":") else {
                        return false;
                    };
                    DEBUG_HEADERS.contains(&name.to_ascii_lowercase().trim())
                })
                .collect();
            if !headers.is_empty() {
                writeln!(result, "debug headers:").unwrap();
                for header in headers {
                    writeln!(result, "{header}").unwrap();
                }
            }
        }
        write!(result, "body:\n{body}").unwrap();
        result
    }
}

impl fmt::Display for HttpNotSuccessful {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.render(true))
    }
}

impl std::error::Error for HttpNotSuccessful {}

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
// Already printed error

/// An error that does not need to be printed because it does not add any new
/// information to what has already been printed.
pub struct AlreadyPrintedError {
    inner: Error,
}

impl AlreadyPrintedError {
    pub fn new(inner: Error) -> Self {
        AlreadyPrintedError { inner }
    }
}

impl std::error::Error for AlreadyPrintedError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.inner.source()
    }
}

impl fmt::Debug for AlreadyPrintedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl fmt::Display for AlreadyPrintedError {
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

impl From<std::io::Error> for CliError {
    fn from(err: std::io::Error) -> CliError {
        CliError::new(err.into(), 1)
    }
}

// =============================================================================
// Construction helpers

pub fn internal<S: fmt::Display>(error: S) -> anyhow::Error {
    InternalError::new(anyhow::format_err!("{}", error)).into()
}
