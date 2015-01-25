use std::error::{FromError, Error};
use std::fmt;
use std::io::IoError;
use std::io::process::{ProcessOutput, ProcessExit, ExitStatus, ExitSignal};
use std::str;

use semver;
use rustc_serialize::json;

use curl;
use git2;
use toml;
use url;

pub type CargoResult<T> = Result<T, Box<CargoError>>;

// =============================================================================
// CargoError trait

pub trait CargoError: Error + Send {
    fn is_human(&self) -> bool { false }
    fn cargo_cause(&self) -> Option<&CargoError>{ None }
}

impl fmt::Debug for Box<CargoError> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
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

impl<T, E: CargoError> ChainError<T> for Result<T, E> {
    fn chain_error<E2, C>(self, callback: C) -> CargoResult<T>
                         where E2: CargoError, C: FnOnce() -> E2 {
        self.map_err(move |err| {
            Box::new(ChainedError {
                error: callback(),
                cause: Box::new(err),
            }) as Box<CargoError>
        })
    }
}

impl<T> ChainError<T> for Option<T> {
    fn chain_error<E, C>(self, callback: C) -> CargoResult<T>
                         where E: CargoError, C: FnOnce() -> E {
        match self {
            Some(t) => Ok(t),
            None => Err(Box::new(callback()) as Box<CargoError>),
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
    pub exit: Option<ProcessExit>,
    pub output: Option<ProcessOutput>,
    cause: Option<IoError>,
}

impl Error for ProcessError {
    fn description(&self) -> &str { self.desc.as_slice() }
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
    fn description(&self) -> &str { self.description.as_slice() }
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
    pub exit_code: u32
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
    pub fn new<S: Str>(error: S, code: u32) -> CliError {
        let error = human(error.as_slice().to_string());
        CliError::from_boxed(error, code)
    }

    pub fn from_error<E: CargoError + 'static>(error: E, code: u32) -> CliError {
        let error = Box::new(error) as Box<CargoError>;
        CliError::from_boxed(error, code)
    }

    pub fn from_boxed(error: Box<CargoError>, code: u32) -> CliError {
        let human = error.is_human();
        CliError { error: error, exit_code: code, unknown: !human }
    }
}

// =============================================================================
// various impls

macro_rules! from_error {
    ($($p:ty,)*) => (
        $(impl FromError<$p> for Box<CargoError> {
            fn from_error(t: $p) -> Box<CargoError> { Box::new(t) }
        })*
    )
}

from_error! {
    semver::ReqParseError,
    IoError,
    ProcessError,
    git2::Error,
    json::DecoderError,
    curl::ErrCode,
    CliError,
    toml::Error,
    url::ParseError,
    toml::DecodeError,
}

impl<E: CargoError> FromError<Human<E>> for Box<CargoError> {
    fn from_error(t: Human<E>) -> Box<CargoError> { Box::new(t) }
}

impl CargoError for semver::ReqParseError {}
impl CargoError for IoError {}
impl CargoError for git2::Error {}
impl CargoError for json::DecoderError {}
impl CargoError for curl::ErrCode {}
impl CargoError for ProcessError {}
impl CargoError for CliError {}
impl CargoError for toml::Error {}
impl CargoError for toml::DecodeError {}
impl CargoError for url::ParseError {}
impl CargoError for str::Utf8Error {}

// =============================================================================
// Construction helpers

pub fn process_error<S: Str>(msg: S,
                             cause: Option<IoError>,
                             status: Option<&ProcessExit>,
                             output: Option<&ProcessOutput>) -> ProcessError {
    let exit = match status {
        Some(&ExitStatus(i)) | Some(&ExitSignal(i)) => i.to_string(),
        None => "never executed".to_string(),
    };
    let mut desc = format!("{} (status={})", msg.as_slice(), exit);

    if let Some(out) = output {
        match str::from_utf8(out.output.as_slice()) {
            Ok(s) if s.trim().len() > 0 => {
                desc.push_str("\n--- stdout\n");
                desc.push_str(s);
            }
            Ok(..) | Err(..) => {}
        }
        match str::from_utf8(out.error.as_slice()) {
            Ok(s) if s.trim().len() > 0 => {
                desc.push_str("\n--- stderr\n");
                desc.push_str(s);
            }
            Ok(..) | Err(..) => {}
        }
    }

    ProcessError {
        desc: desc,
        exit: status.map(|a| a.clone()),
        output: output.map(|a| a.clone()),
        cause: cause,
    }
}

pub fn internal_error<S1: Str, S2: Str>(error: S1,
                                        detail: S2) -> Box<CargoError> {
    Box::new(ConcreteCargoError {
        description: error.as_slice().to_string(),
        detail: Some(detail.as_slice().to_string()),
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

pub fn caused_human<S: fmt::Display, E: Error + Send>(error: S, cause: E) -> Box<CargoError> {
    Box::new(ConcreteCargoError {
        description: error.to_string(),
        detail: None,
        cause: Some(Box::new(cause) as Box<Error + Send>),
        is_human: true
    })
}
