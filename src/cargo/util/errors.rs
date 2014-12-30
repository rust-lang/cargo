use std::error::{FromError, Error};
use std::fmt::{mod, Show};
use std::io::IoError;
use std::io::process::{ProcessOutput, ProcessExit, ExitStatus, ExitSignal};
use std::str;

use semver;
use rustc_serialize::json;

use curl;
use toml::Error as TomlError;
use url;
use git2;

pub type CargoResult<T> = Result<T, Box<CargoError>>;

// =============================================================================
// CargoError trait

pub trait CargoError: Error {
    fn is_human(&self) -> bool { false }
}

impl Show for Box<CargoError> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(f, "{}", self.description()));
        Ok(())
    }
}

impl Error for Box<CargoError> {
    fn description(&self) -> &str { (**self).description() }
    fn detail(&self) -> Option<String> { (**self).detail() }
    fn cause(&self) -> Option<&Error> { (**self).cause() }
}

impl CargoError for Box<CargoError> {
    fn is_human(&self) -> bool { (**self).is_human() }
}

// =============================================================================
// Chaining errors

pub trait ChainError<T> {
    fn chain_error<E, F>(self, callback: F) -> CargoResult<T>
                         where E: CargoError, F: FnOnce() -> E;
}

struct ChainedError<E> {
    error: E,
    cause: Box<Error>,
}

impl<'a, T, F> ChainError<T> for F where F: FnOnce() -> CargoResult<T> {
    fn chain_error<E, C>(self, callback: C) -> CargoResult<T>
                         where E: CargoError, C: FnOnce() -> E {
        self().chain_error(callback)
    }
}

impl<T, E: Error> ChainError<T> for Result<T, E> {
    fn chain_error<E2, C>(self, callback: C) -> CargoResult<T>
                         where E2: CargoError, C: FnOnce() -> E2 {
        self.map_err(move |err| {
            box ChainedError {
                error: callback(),
                cause: box err,
            } as Box<CargoError>
        })
    }
}

impl<T> ChainError<T> for Option<T> {
    fn chain_error<E, C>(self, callback: C) -> CargoResult<T>
                         where E: CargoError, C: FnOnce() -> E {
        match self {
            Some(t) => Ok(t),
            None => Err(box callback() as Box<CargoError>),
        }
    }
}

impl<E: Error> Error for ChainedError<E> {
    fn description(&self) -> &str { self.error.description() }
    fn detail(&self) -> Option<String> { self.error.detail() }
    fn cause(&self) -> Option<&Error> { Some(&*self.cause) }
}

impl<E: CargoError> CargoError for ChainedError<E> {
    fn is_human(&self) -> bool { self.error.is_human() }
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
    fn detail(&self) -> Option<String> { None }
    fn cause(&self) -> Option<&Error> {
        self.cause.as_ref().map(|s| s as &Error)
    }
}

impl fmt::Show for ProcessError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.desc.fmt(f)
    }
}

// =============================================================================
// Concrete errors

struct ConcreteCargoError {
    description: String,
    detail: Option<String>,
    cause: Option<Box<Error>>,
    is_human: bool,
}

impl fmt::Show for ConcreteCargoError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.description)
    }
}

impl Error for ConcreteCargoError {
    fn description(&self) -> &str { self.description.as_slice() }
    fn detail(&self) -> Option<String> { self.detail.clone() }
    fn cause(&self) -> Option<&Error> {
        self.cause.as_ref().map(|c| &**c)
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
    fn detail(&self) -> Option<String> { self.0.detail() }
    fn cause(&self) -> Option<&Error> { self.0.cause() }
}

impl<E: Error> CargoError for Human<E> {
    fn is_human(&self) -> bool { true }
}

// =============================================================================
// CLI errors

pub type CliResult<T> = Result<T, CliError>;

#[deriving(Show)]
pub struct CliError {
    pub error: Box<CargoError>,
    pub unknown: bool,
    pub exit_code: uint
}

impl Error for CliError {
    fn description(&self) -> &str { self.error.description() }
    fn detail(&self) -> Option<String> { self.error.detail() }
    fn cause(&self) -> Option<&Error> { self.error.cause() }
}

impl CliError {
    pub fn new<S: Str>(error: S, code: uint) -> CliError {
        let error = human(error.as_slice().to_string());
        CliError::from_boxed(error, code)
    }

    pub fn from_error<E: CargoError + 'static>(error: E, code: uint) -> CliError {
        let error = box error as Box<CargoError>;
        CliError::from_boxed(error, code)
    }

    pub fn from_boxed(error: Box<CargoError>, code: uint) -> CliError {
        let human = error.is_human();
        CliError { error: error, exit_code: code, unknown: !human }
    }
}

// =============================================================================
// various impls

macro_rules! from_error {
    ($($p:ty,)*) => (
        $(impl FromError<$p> for Box<CargoError> {
            fn from_error(t: $p) -> Box<CargoError> { box t }
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
    TomlError,
    url::ParseError,
}

impl<E: Error> FromError<Human<E>> for Box<CargoError> {
    fn from_error(t: Human<E>) -> Box<CargoError> { box t }
}

impl CargoError for semver::ReqParseError {}
impl CargoError for IoError {}
impl CargoError for git2::Error {}
impl CargoError for json::DecoderError {}
impl CargoError for curl::ErrCode {}
impl CargoError for ProcessError {}
impl CargoError for CliError {}
impl CargoError for TomlError {}
impl CargoError for url::ParseError {}

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
    box ConcreteCargoError {
        description: error.as_slice().to_string(),
        detail: Some(detail.as_slice().to_string()),
        cause: None,
        is_human: false
    }
}

pub fn internal<S: Show>(error: S) -> Box<CargoError> {
    box ConcreteCargoError {
        description: error.to_string(),
        detail: None,
        cause: None,
        is_human: false
    }
}

pub fn human<S: Show>(error: S) -> Box<CargoError> {
    box ConcreteCargoError {
        description: error.to_string(),
        detail: None,
        cause: None,
        is_human: true
    }
}

pub fn caused_human<S: Show, E: Error>(error: S, cause: E) -> Box<CargoError> {
    box ConcreteCargoError {
        description: error.to_string(),
        detail: None,
        cause: Some(box cause as Box<Error>),
        is_human: true
    }
}
