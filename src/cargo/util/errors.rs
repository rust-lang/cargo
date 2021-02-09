#![allow(unknown_lints)]

use crate::core::{TargetKind, Workspace};
use crate::ops::CompileOptions;
use anyhow::Error;
use std::fmt;
use std::path::PathBuf;
use std::process::{ExitStatus, Output};
use std::str;

pub type CargoResult<T> = anyhow::Result<T>;

// TODO: should delete this trait and just use `with_context` instead
pub trait CargoResultExt<T, E> {
    fn chain_err<F, D>(self, f: F) -> CargoResult<T>
    where
        F: FnOnce() -> D,
        D: fmt::Display + Send + Sync + 'static;
}

impl<T, E> CargoResultExt<T, E> for Result<T, E>
where
    E: Into<Error>,
{
    fn chain_err<F, D>(self, f: F) -> CargoResult<T>
    where
        F: FnOnce() -> D,
        D: fmt::Display + Send + Sync + 'static,
    {
        self.map_err(|e| e.into().context(f()))
    }
}

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
// Process errors
#[derive(Debug)]
pub struct ProcessError {
    /// A detailed description to show to the user why the process failed.
    pub desc: String,

    /// The exit status of the process.
    ///
    /// This can be `None` if the process failed to launch (like process not
    /// found) or if the exit status wasn't a code but was instead something
    /// like termination via a signal.
    pub code: Option<i32>,

    /// The stdout from the process.
    ///
    /// This can be `None` if the process failed to launch, or the output was
    /// not captured.
    pub stdout: Option<Vec<u8>>,

    /// The stderr from the process.
    ///
    /// This can be `None` if the process failed to launch, or the output was
    /// not captured.
    pub stderr: Option<Vec<u8>>,
}

impl fmt::Display for ProcessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.desc.fmt(f)
    }
}

impl std::error::Error for ProcessError {}

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

// =============================================================================
// Construction helpers

/// Creates a new process error.
///
/// `status` can be `None` if the process did not launch.
/// `output` can be `None` if the process did not launch, or output was not captured.
pub fn process_error(
    msg: &str,
    status: Option<ExitStatus>,
    output: Option<&Output>,
) -> ProcessError {
    let exit = match status {
        Some(s) => exit_status_to_string(s),
        None => "never executed".to_string(),
    };

    process_error_raw(
        msg,
        status.and_then(|s| s.code()),
        &exit,
        output.map(|s| s.stdout.as_slice()),
        output.map(|s| s.stderr.as_slice()),
    )
}

pub fn process_error_raw(
    msg: &str,
    code: Option<i32>,
    status: &str,
    stdout: Option<&[u8]>,
    stderr: Option<&[u8]>,
) -> ProcessError {
    let mut desc = format!("{} ({})", msg, status);

    if let Some(out) = stdout {
        match str::from_utf8(out) {
            Ok(s) if !s.trim().is_empty() => {
                desc.push_str("\n--- stdout\n");
                desc.push_str(s);
            }
            Ok(..) | Err(..) => {}
        }
    }
    if let Some(out) = stderr {
        match str::from_utf8(out) {
            Ok(s) if !s.trim().is_empty() => {
                desc.push_str("\n--- stderr\n");
                desc.push_str(s);
            }
            Ok(..) | Err(..) => {}
        }
    }

    ProcessError {
        desc,
        code,
        stdout: stdout.map(|s| s.to_vec()),
        stderr: stderr.map(|s| s.to_vec()),
    }
}

pub fn exit_status_to_string(status: ExitStatus) -> String {
    return status_to_string(status);

    #[cfg(unix)]
    fn status_to_string(status: ExitStatus) -> String {
        use std::os::unix::process::*;

        if let Some(signal) = status.signal() {
            let name = match signal as libc::c_int {
                libc::SIGABRT => ", SIGABRT: process abort signal",
                libc::SIGALRM => ", SIGALRM: alarm clock",
                libc::SIGFPE => ", SIGFPE: erroneous arithmetic operation",
                libc::SIGHUP => ", SIGHUP: hangup",
                libc::SIGILL => ", SIGILL: illegal instruction",
                libc::SIGINT => ", SIGINT: terminal interrupt signal",
                libc::SIGKILL => ", SIGKILL: kill",
                libc::SIGPIPE => ", SIGPIPE: write on a pipe with no one to read",
                libc::SIGQUIT => ", SIGQUIT: terminal quit signal",
                libc::SIGSEGV => ", SIGSEGV: invalid memory reference",
                libc::SIGTERM => ", SIGTERM: termination signal",
                libc::SIGBUS => ", SIGBUS: access to undefined memory",
                #[cfg(not(target_os = "haiku"))]
                libc::SIGSYS => ", SIGSYS: bad system call",
                libc::SIGTRAP => ", SIGTRAP: trace/breakpoint trap",
                _ => "",
            };
            format!("signal: {}{}", signal, name)
        } else {
            status.to_string()
        }
    }

    #[cfg(windows)]
    fn status_to_string(status: ExitStatus) -> String {
        use winapi::shared::minwindef::DWORD;
        use winapi::um::winnt::*;

        let mut base = status.to_string();
        let extra = match status.code().unwrap() as DWORD {
            STATUS_ACCESS_VIOLATION => "STATUS_ACCESS_VIOLATION",
            STATUS_IN_PAGE_ERROR => "STATUS_IN_PAGE_ERROR",
            STATUS_INVALID_HANDLE => "STATUS_INVALID_HANDLE",
            STATUS_INVALID_PARAMETER => "STATUS_INVALID_PARAMETER",
            STATUS_NO_MEMORY => "STATUS_NO_MEMORY",
            STATUS_ILLEGAL_INSTRUCTION => "STATUS_ILLEGAL_INSTRUCTION",
            STATUS_NONCONTINUABLE_EXCEPTION => "STATUS_NONCONTINUABLE_EXCEPTION",
            STATUS_INVALID_DISPOSITION => "STATUS_INVALID_DISPOSITION",
            STATUS_ARRAY_BOUNDS_EXCEEDED => "STATUS_ARRAY_BOUNDS_EXCEEDED",
            STATUS_FLOAT_DENORMAL_OPERAND => "STATUS_FLOAT_DENORMAL_OPERAND",
            STATUS_FLOAT_DIVIDE_BY_ZERO => "STATUS_FLOAT_DIVIDE_BY_ZERO",
            STATUS_FLOAT_INEXACT_RESULT => "STATUS_FLOAT_INEXACT_RESULT",
            STATUS_FLOAT_INVALID_OPERATION => "STATUS_FLOAT_INVALID_OPERATION",
            STATUS_FLOAT_OVERFLOW => "STATUS_FLOAT_OVERFLOW",
            STATUS_FLOAT_STACK_CHECK => "STATUS_FLOAT_STACK_CHECK",
            STATUS_FLOAT_UNDERFLOW => "STATUS_FLOAT_UNDERFLOW",
            STATUS_INTEGER_DIVIDE_BY_ZERO => "STATUS_INTEGER_DIVIDE_BY_ZERO",
            STATUS_INTEGER_OVERFLOW => "STATUS_INTEGER_OVERFLOW",
            STATUS_PRIVILEGED_INSTRUCTION => "STATUS_PRIVILEGED_INSTRUCTION",
            STATUS_STACK_OVERFLOW => "STATUS_STACK_OVERFLOW",
            STATUS_DLL_NOT_FOUND => "STATUS_DLL_NOT_FOUND",
            STATUS_ORDINAL_NOT_FOUND => "STATUS_ORDINAL_NOT_FOUND",
            STATUS_ENTRYPOINT_NOT_FOUND => "STATUS_ENTRYPOINT_NOT_FOUND",
            STATUS_CONTROL_C_EXIT => "STATUS_CONTROL_C_EXIT",
            STATUS_DLL_INIT_FAILED => "STATUS_DLL_INIT_FAILED",
            STATUS_FLOAT_MULTIPLE_FAULTS => "STATUS_FLOAT_MULTIPLE_FAULTS",
            STATUS_FLOAT_MULTIPLE_TRAPS => "STATUS_FLOAT_MULTIPLE_TRAPS",
            STATUS_REG_NAT_CONSUMPTION => "STATUS_REG_NAT_CONSUMPTION",
            STATUS_HEAP_CORRUPTION => "STATUS_HEAP_CORRUPTION",
            STATUS_STACK_BUFFER_OVERRUN => "STATUS_STACK_BUFFER_OVERRUN",
            STATUS_ASSERTION_FAILURE => "STATUS_ASSERTION_FAILURE",
            _ => return base,
        };
        base.push_str(", ");
        base.push_str(extra);
        base
    }
}

pub fn is_simple_exit_code(code: i32) -> bool {
    // Typical unix exit codes are 0 to 127.
    // Windows doesn't have anything "typical", and is a
    // 32-bit number (which appears signed here, but is really
    // unsigned). However, most of the interesting NTSTATUS
    // codes are very large. This is just a rough
    // approximation of which codes are "normal" and which
    // ones are abnormal termination.
    code >= 0 && code <= 127
}

pub fn internal<S: fmt::Display>(error: S) -> anyhow::Error {
    InternalError::new(anyhow::format_err!("{}", error)).into()
}
