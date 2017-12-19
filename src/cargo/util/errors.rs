#![allow(unknown_lints)]

use std::fmt;
use std::process::{Output, ExitStatus};
use std::str;

use core::TargetKind;
use failure::{Context, Error, Fail};

pub use failure::Error as CargoError;
pub type CargoResult<T> = Result<T, Error>;

pub trait CargoResultExt<T, E> {
    fn chain_err<F, D>(self, f: F) -> Result<T, Context<D>>
        where F: FnOnce() -> D,
              D: fmt::Display + Send + Sync + 'static;
}

impl<T, E> CargoResultExt<T, E> for Result<T, E>
	where E: Into<Error>,
{
    fn chain_err<F, D>(self, f: F) -> Result<T, Context<D>>
        where F: FnOnce() -> D,
              D: fmt::Display + Send + Sync + 'static,
    {
        self.map_err(|failure| {
            let context = f();
            failure.into().context(context)
        })
    }
}

#[derive(Debug, Fail)]
#[fail(display = "failed to get 200 response from `{}`, got {}", url, code)]
pub struct HttpNot200 {
    pub code: u32,
    pub url: String,
}

pub struct Internal {
    inner: Error,
}

impl Internal {
    pub fn new(inner: Error) -> Internal {
        Internal { inner }
    }
}

impl Fail for Internal {
    fn cause(&self) -> Option<&Fail> {
        self.inner.cause().cause()
    }
}

impl fmt::Debug for Internal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl fmt::Display for Internal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.inner.fmt(f)
    }
}

// =============================================================================
// Process errors
#[derive(Debug, Fail)]
#[fail(display = "{}", desc)]
pub struct ProcessError {
    pub desc: String,
    pub exit: Option<ExitStatus>,
    pub output: Option<Output>,
}

// =============================================================================
// Cargo test errors.

/// Error when testcases fail
#[derive(Debug, Fail)]
#[fail(display = "{}", desc)]
pub struct CargoTestError {
    pub test: Test,
    pub desc: String,
    pub exit: Option<ExitStatus>,
    pub causes: Vec<ProcessError>,
}

#[derive(Debug)]
pub enum Test {
    Multiple,
    Doc,
    UnitTest(TargetKind, String)
}

impl CargoTestError {
    pub fn new(test: Test, errors: Vec<ProcessError>) -> Self {
        if errors.is_empty() {
            panic!("Cannot create CargoTestError from empty Vec")
        }
        let desc = errors.iter().map(|error| error.desc.clone())
                                .collect::<Vec<String>>()
                                .join("\n");
        CargoTestError {
            test: test,
            desc: desc,
            exit: errors[0].exit,
            causes: errors,
        }
    }

    pub fn hint(&self) -> String {
        match self.test {
            Test::UnitTest(ref kind, ref name) => {
                match *kind {
                    TargetKind::Bench => format!("test failed, to rerun pass '--bench {}'", name),
                    TargetKind::Bin => format!("test failed, to rerun pass '--bin {}'", name),
                    TargetKind::Lib(_) => "test failed, to rerun pass '--lib'".into(),
                    TargetKind::Test => format!("test failed, to rerun pass '--test {}'", name),
                    TargetKind::ExampleBin | TargetKind::ExampleLib(_) =>
                        format!("test failed, to rerun pass '--example {}", name),
                    _ => "test failed.".into()
                }
            },
            Test::Doc => "test failed, to rerun pass '--doc'".into(),
            _ => "test failed.".into()
        }
    }
}

// =============================================================================
// CLI errors

pub type CliResult = Result<(), CliError>;

#[derive(Debug)]
pub struct CliError {
    pub error: Option<CargoError>,
    pub unknown: bool,
    pub exit_code: i32
}

impl CliError {
    pub fn new(error: CargoError, code: i32) -> CliError {
        let unknown = error.downcast_ref::<Internal>().is_some();
        CliError { error: Some(error), exit_code: code, unknown }
    }

    pub fn code(code: i32) -> CliError {
        CliError { error: None, exit_code: code, unknown: false }
    }
}

impl From<CargoError> for CliError {
    fn from(err: CargoError) -> CliError {
        CliError::new(err, 101)
    }
}


// =============================================================================
// Construction helpers

pub fn process_error(msg: &str,
                     status: Option<&ExitStatus>,
                     output: Option<&Output>) -> ProcessError
{
    let exit = match status {
        Some(s) => status_to_string(s),
        None => "never executed".to_string(),
    };
    let mut desc = format!("{} ({})", &msg, exit);

    if let Some(out) = output {
        match str::from_utf8(&out.stdout) {
            Ok(s) if !s.trim().is_empty() => {
                desc.push_str("\n--- stdout\n");
                desc.push_str(s);
            }
            Ok(..) | Err(..) => {}
        }
        match str::from_utf8(&out.stderr) {
            Ok(s) if !s.trim().is_empty() => {
                desc.push_str("\n--- stderr\n");
                desc.push_str(s);
            }
            Ok(..) | Err(..) => {}
        }
    }

    return ProcessError {
        desc: desc,
        exit: status.cloned(),
        output: output.cloned(),
    };

    #[cfg(unix)]
    fn status_to_string(status: &ExitStatus) -> String {
        use std::os::unix::process::*;
        use libc;

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
                libc::SIGQUIT => ", SIGQUIT: terminal quite signal",
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
    fn status_to_string(status: &ExitStatus) -> String {
        status.to_string()
    }
}

pub fn internal<S: fmt::Display>(error: S) -> CargoError {
    _internal(&error)
}

fn _internal(error: &fmt::Display) -> CargoError {
    Internal::new(format_err!("{}", error)).into()
}
