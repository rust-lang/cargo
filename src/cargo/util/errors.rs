use std::error::Error;
use std::ffi;
use std::fmt;
use std::io;
use std::process::{Output, ExitStatus};
use std::str;

use curl;
use git2;
use rustc_serialize::json;
use semver;
use term;
use toml;
use url;

pub type CargoResult<T> = Result<T, Box<CargoError>>;

// =============================================================================
// CargoError trait

pub trait CargoError: Error + Send + 'static {
    fn is_human(&self) -> bool { false }
    fn cargo_cause(&self) -> Option<&CargoError>{ None }
}

impl Error for Box<CargoError> {
    fn description(&self) -> &str { (**self).description() }
    fn cause(&self) -> Option<&Error> { (**self).cause() }
}

impl CargoError for Box<CargoError> {
    fn is_human(&self) -> bool { (**self).is_human() }
    fn cargo_cause(&self) -> Option<&CargoError> { (**self).cargo_cause() }
}

// =============================================================================
// Chaining errors

pub trait ChainError<T> {
    fn chain_error<E, F>(self, callback: F) -> CargoResult<T>
                         where E: CargoError, F: FnOnce() -> E;
}

#[derive(Debug)]
struct ChainedError<E> {
    error: E,
    cause: Box<CargoError>,
}

impl<'a, T, F> ChainError<T> for F where F: FnOnce() -> CargoResult<T> {
    fn chain_error<E, C>(self, callback: C) -> CargoResult<T>
                         where E: CargoError, C: FnOnce() -> E {
        self().chain_error(callback)
    }
}

impl<T, E: CargoError + 'static> ChainError<T> for Result<T, E> {
    fn chain_error<E2: 'static, C>(self, callback: C) -> CargoResult<T>
                         where E2: CargoError, C: FnOnce() -> E2 {
        self.map_err(move |err| {
            Box::new(ChainedError {
                error: callback(),
                cause: Box::new(err),
            }) as Box<CargoError>
        })
    }
}

impl<T> ChainError<T> for Box<CargoError> {
    fn chain_error<E2, C>(self, callback: C) -> CargoResult<T>
                         where E2: CargoError, C: FnOnce() -> E2 {
        Err(Box::new(ChainedError {
            error: callback(),
            cause: self,
        }))
    }
}

impl<T> ChainError<T> for Option<T> {
    fn chain_error<E: 'static, C>(self, callback: C) -> CargoResult<T>
                         where E: CargoError, C: FnOnce() -> E {
        match self {
            Some(t) => Ok(t),
            None => Err(Box::new(callback())),
        }
    }
}

impl<E: Error> Error for ChainedError<E> {
    fn description(&self) -> &str { self.error.description() }
}

impl<E: fmt::Display> fmt::Display for ChainedError<E> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.error, f)
    }
}

impl<E: CargoError> CargoError for ChainedError<E> {
    fn is_human(&self) -> bool { self.error.is_human() }
    fn cargo_cause(&self) -> Option<&CargoError> { Some(&*self.cause) }
}

// =============================================================================
// Process errors

pub struct ProcessError {
    pub desc: String,
    pub exit: Option<ExitStatus>,
    pub output: Option<Output>,
    cause: Option<io::Error>,
}

impl Error for ProcessError {
    fn description(&self) -> &str { &self.desc }
    fn cause(&self) -> Option<&Error> {
        self.cause.as_ref().map(|s| s as &Error)
    }
}

impl fmt::Display for ProcessError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.desc, f)
    }
}
impl fmt::Debug for ProcessError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

// =============================================================================
// Cargo test errors.

/// Error when testcases fail
pub struct CargoTestError {
    pub desc: String,
    pub exit: Option<ExitStatus>,
    pub causes: Vec<ProcessError>,
}

impl CargoTestError {
    pub fn new(errors: Vec<ProcessError>) -> Self {
        if errors.is_empty() {
            panic!("Cannot create CargoTestError from empty Vec")
        }
        let desc = errors.iter().map(|error| error.desc.clone())
                                .collect::<Vec<String>>()
                                .join("\n");
        CargoTestError {
            desc: desc,
            exit: errors[0].exit,
            causes: errors,
        }
    }
}

impl fmt::Display for CargoTestError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.desc, f)
    }
}

impl fmt::Debug for CargoTestError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl Error for CargoTestError {
    fn description(&self) -> &str { &self.desc }
    fn cause(&self) -> Option<&Error> {
        self.causes.get(0).map(|s| s as &Error)
    }
}


// =============================================================================
// Concrete errors

struct ConcreteCargoError {
    description: String,
    detail: Option<String>,
    cause: Option<Box<Error+Send>>,
    is_human: bool,
}

impl fmt::Display for ConcreteCargoError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(f, "{}", self.description));
        if let Some(ref s) = self.detail {
            try!(write!(f, " ({})", s));
        }
        Ok(())
    }
}
impl fmt::Debug for ConcreteCargoError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl Error for ConcreteCargoError {
    fn description(&self) -> &str { &self.description }
    fn cause(&self) -> Option<&Error> {
        self.cause.as_ref().map(|c| {
            let e: &Error = &**c; e
        })
    }
}

impl CargoError for ConcreteCargoError {
    fn is_human(&self) -> bool {
        self.is_human
    }
}

// =============================================================================
// Human errors

#[derive(Debug)]
pub struct Human<E>(pub E);

impl<E: Error> Error for Human<E> {
    fn description(&self) -> &str { self.0.description() }
    fn cause(&self) -> Option<&Error> { self.0.cause() }
}

impl<E: fmt::Display> fmt::Display for Human<E> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl<E: CargoError> CargoError for Human<E> {
    fn is_human(&self) -> bool { true }
    fn cargo_cause(&self) -> Option<&CargoError> { self.0.cargo_cause() }
}

// =============================================================================
// CLI errors

pub type CliResult<T> = Result<T, CliError>;

#[derive(Debug)]
pub struct CliError {
    pub error: Box<CargoError>,
    pub unknown: bool,
    pub exit_code: i32
}

impl Error for CliError {
    fn description(&self) -> &str { self.error.description() }
    fn cause(&self) -> Option<&Error> { self.error.cause() }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.error, f)
    }
}

impl CliError {
    pub fn new(error: &str, code: i32) -> CliError {
        let error = human(error.to_string());
        CliError::from_boxed(error, code)
    }

    pub fn from_error<E: CargoError>(error: E, code: i32) -> CliError {
        let error = Box::new(error);
        CliError::from_boxed(error, code)
    }

    pub fn from_boxed(error: Box<CargoError>, code: i32) -> CliError {
        let human = error.is_human();
        CliError { error: error, exit_code: code, unknown: !human }
    }
}

impl From<Box<CargoError>> for CliError {
    fn from(err: Box<CargoError>) -> CliError {
        CliError::from_boxed(err, 101)
    }
}

// =============================================================================
// various impls

macro_rules! from_error {
    ($($p:ty,)*) => (
        $(impl From<$p> for Box<CargoError> {
            fn from(t: $p) -> Box<CargoError> { Box::new(t) }
        })*
    )
}

from_error! {
    semver::ReqParseError,
    io::Error,
    ProcessError,
    git2::Error,
    json::DecoderError,
    json::EncoderError,
    curl::ErrCode,
    CliError,
    toml::Error,
    url::ParseError,
    toml::DecodeError,
    ffi::NulError,
    term::Error,
}

impl<E: CargoError> From<Human<E>> for Box<CargoError> {
    fn from(t: Human<E>) -> Box<CargoError> { Box::new(t) }
}

impl CargoError for semver::ReqParseError {}
impl CargoError for io::Error {}
impl CargoError for git2::Error {}
impl CargoError for json::DecoderError {}
impl CargoError for json::EncoderError {}
impl CargoError for curl::ErrCode {}
impl CargoError for ProcessError {}
impl CargoError for CargoTestError {}
impl CargoError for CliError {}
impl CargoError for toml::Error {}
impl CargoError for toml::DecodeError {}
impl CargoError for url::ParseError {}
impl CargoError for ffi::NulError {}
impl CargoError for term::Error {}

// =============================================================================
// Construction helpers

pub fn process_error(msg: &str,
                     cause: Option<io::Error>,
                     status: Option<&ExitStatus>,
                     output: Option<&Output>) -> ProcessError {
    let exit = match status {
        Some(s) => status_to_string(s),
        None => "never executed".to_string(),
    };
    let mut desc = format!("{} ({})", &msg, exit);

    if let Some(out) = output {
        match str::from_utf8(&out.stdout) {
            Ok(s) if s.trim().len() > 0 => {
                desc.push_str("\n--- stdout\n");
                desc.push_str(s);
            }
            Ok(..) | Err(..) => {}
        }
        match str::from_utf8(&out.stderr) {
            Ok(s) if s.trim().len() > 0 => {
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
        cause: cause,
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

pub fn internal_error(error: &str, detail: &str) -> Box<CargoError> {
    Box::new(ConcreteCargoError {
        description: error.to_string(),
        detail: Some(detail.to_string()),
        cause: None,
        is_human: false
    })
}

pub fn internal<S: fmt::Display>(error: S) -> Box<CargoError> {
    Box::new(ConcreteCargoError {
        description: error.to_string(),
        detail: None,
        cause: None,
        is_human: false
    })
}

pub fn human<S: fmt::Display>(error: S) -> Box<CargoError> {
    Box::new(ConcreteCargoError {
        description: error.to_string(),
        detail: None,
        cause: None,
        is_human: true
    })
}

pub fn caused_human<S, E>(error: S, cause: E) -> Box<CargoError>
    where S: fmt::Display,
          E: Error + Send + 'static
{
    Box::new(ConcreteCargoError {
        description: error.to_string(),
        detail: None,
        cause: Some(Box::new(cause)),
        is_human: true
    })
}
