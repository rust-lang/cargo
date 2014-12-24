use std::io::process::{ProcessOutput, ProcessExit, ExitStatus, ExitSignal};
use std::io::IoError;
use std::fmt::{mod, Show, Formatter};
use std::str;
use rustc_serialize::json;
use semver;

use curl;
use docopt;
use toml::Error as TomlError;
use url;
use git2;

pub trait CargoError: Send {
    fn description(&self) -> String;
    fn detail(&self) -> Option<String> { None }
    fn cause(&self) -> Option<&CargoError> { None }
    fn is_human(&self) -> bool { false }

    fn concrete(&self) -> ConcreteCargoError {
        ConcreteCargoError {
            description: self.description(),
            detail: self.detail(),
            cause: self.cause().map(|c| box c.concrete() as Box<CargoError>),
            is_human: self.is_human()
        }
    }
}

pub trait FromError<E> {
    fn from_error(error: E) -> Self;
}

impl<E: CargoError> FromError<E> for Box<CargoError> {
    fn from_error(error: E) -> Box<CargoError> {
        box error as Box<CargoError>
    }
}

macro_rules! from_error {
    ($ty:ty) => {
        impl FromError<$ty> for $ty {
            fn from_error(error: $ty) -> $ty {
                error
            }
        }
    }
}

impl Show for Box<CargoError> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        try!(write!(f, "{}", self.description()));
        Ok(())
    }
}

impl CargoError for Box<CargoError> {
    fn description(&self) -> String { (**self).description() }
    fn detail(&self) -> Option<String> { (**self).detail() }
    fn cause(&self) -> Option<&CargoError> { (**self).cause() }
    fn is_human(&self) -> bool { (**self).is_human() }
}

impl CargoError for semver::ReqParseError {
    fn description(&self) -> String {
        self.to_string()
    }
}

pub type CargoResult<T> = Result<T, Box<CargoError>>;

pub trait BoxError<T> {
    fn box_error(self) -> CargoResult<T>;
}

pub trait ChainError<T> {
    fn chain_error<E: CargoError>(self, callback: || -> E) -> CargoResult<T> ;
}

impl<'a, T> ChainError<T> for ||:'a -> CargoResult<T> {
    fn chain_error<E: CargoError>(self, callback: || -> E) -> CargoResult<T> {
        self().map_err(|err| callback().concrete().with_cause(err))
    }
}

impl<T, E: CargoError> BoxError<T> for Result<T, E> {
    fn box_error(self) -> CargoResult<T> {
        self.map_err(|err| box err as Box<CargoError>)
    }
}

impl<T, E: CargoError> ChainError<T> for Result<T, E> {
    fn chain_error<E: CargoError>(self, callback: || -> E) -> CargoResult<T>  {
        self.map_err(|err| callback().concrete().with_cause(err))
    }
}

impl CargoError for IoError {
    fn description(&self) -> String { self.to_string() }
}

from_error!(IoError);

impl CargoError for TomlError {
    fn description(&self) -> String { self.to_string() }
}

from_error!(TomlError);

impl CargoError for fmt::Error {
    fn description(&self) -> String {
        "formatting failed".to_string()
    }
}

from_error!(fmt::Error);

impl CargoError for curl::ErrCode {
    fn description(&self) -> String { self.to_string() }
}

from_error!(curl::ErrCode);

impl CargoError for json::DecoderError {
    fn description(&self) -> String { self.to_string() }
}

from_error!(json::DecoderError);

pub struct ProcessError {
    pub msg: String,
    pub exit: Option<ProcessExit>,
    pub output: Option<ProcessOutput>,
    pub detail: Option<String>,
    pub cause: Option<Box<CargoError>>
}

from_error!(ProcessError);

impl Show for ProcessError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let exit = match self.exit {
            Some(ExitStatus(i)) | Some(ExitSignal(i)) => i.to_string(),
            None => "never executed".to_string()
        };
        try!(write!(f, "{} (status={})", self.msg, exit));
        if let Some(out) = self.output() {
            try!(write!(f, "{}", out));
        }
        Ok(())
    }
}

impl ProcessError {
    pub fn output(&self) -> Option<String> {
        match self.output {
            Some(ref out) => {
                let mut string = String::new();
                match str::from_utf8(out.output.as_slice()) {
                    Ok(s) if s.trim().len() > 0 => {
                        string.push_str("\n--- stdout\n");
                        string.push_str(s);
                    }
                    Ok(..) | Err(..) => {}
                }
                match str::from_utf8(out.error.as_slice()) {
                    Ok(s) if s.trim().len() > 0 => {
                        string.push_str("\n--- stderr\n");
                        string.push_str(s);
                    }
                    Ok(..) | Err(..) => {}
                }
                Some(string)
            },
            None => None
        }
    }
}

impl CargoError for ProcessError {
    fn description(&self) -> String { self.to_string() }

    fn detail(&self) -> Option<String> {
        self.detail.clone()
    }

    fn cause(&self) -> Option<&CargoError> {
        self.cause.as_ref().map(|c| { let err: &CargoError = &**c; err })
    }
}

pub struct ConcreteCargoError {
    description: String,
    detail: Option<String>,
    cause: Option<Box<CargoError>>,
    is_human: bool
}

impl ConcreteCargoError {
    pub fn with_cause<E: CargoError>(mut self, err: E) -> Box<CargoError> {
        self.cause = Some(box err as Box<CargoError>);
        box self as Box<CargoError>
    }

    pub fn mark_human(mut self) -> Box<CargoError> {
        self.is_human = true;
        box self as Box<CargoError>
    }
}

impl Show for ConcreteCargoError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.description)
    }
}

impl CargoError for ConcreteCargoError {
    fn description(&self) -> String {
        self.description.clone()
    }

    fn detail(&self) -> Option<String> {
        self.detail.clone()
    }

    fn cause(&self) -> Option<&CargoError> {
        self.cause.as_ref().map(|c| { let err: &CargoError = &**c; err })
    }

    fn is_human(&self) -> bool {
        self.is_human
    }
}

pub type CliResult<T> = Result<T, CliError>;

#[deriving(Show)]
pub struct CliError {
    pub error: Box<CargoError>,
    pub unknown: bool,
    pub exit_code: uint
}

impl CargoError for CliError {
    fn description(&self) -> String {
        self.error.to_string()
    }
}

from_error!(CliError);

impl CargoError for docopt::Error {
    fn description(&self) -> String {
        match *self {
            docopt::Error::WithProgramUsage(ref other, _) => other.description(),
            ref e if e.fatal() => self.to_string(),
            _ => "".to_string(),
        }
    }

    fn detail(&self) -> Option<String> {
        match *self {
            docopt::Error::WithProgramUsage(_, ref usage) => Some(usage.clone()),
            ref e if e.fatal() => None,
            ref e => Some(e.to_string()),
        }
    }

    fn is_human(&self) -> bool { true }
}

from_error!(docopt::Error);

impl CargoError for url::ParseError {
    fn description(&self) -> String { self.to_string() }
}

from_error!(url::ParseError);

impl CargoError for git2::Error {
    fn description(&self) -> String { self.to_string() }
}

from_error!(git2::Error);

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

pub fn process_error<S: Str>(msg: S,
                             cause: Option<IoError>,
                             status: Option<&ProcessExit>,
                             output: Option<&ProcessOutput>) -> ProcessError {
    ProcessError {
        msg: msg.as_slice().to_string(),
        exit: status.map(|o| o.clone()),
        output: output.map(|o| o.clone()),
        detail: None,
        cause: cause.map(|c| box c as Box<CargoError>)
    }
}

pub fn internal_error<S1: Str, S2: Str>(error: S1,
                                        detail: S2) -> Box<CargoError> {
    box ConcreteCargoError {
        description: error.as_slice().to_string(),
        detail: Some(detail.as_slice().to_string()),
        cause: None,
        is_human: false
    } as Box<CargoError>
}

pub fn internal<S: Show>(error: S) -> Box<CargoError> {
    box ConcreteCargoError {
        description: error.to_string(),
        detail: None,
        cause: None,
        is_human: false
    } as Box<CargoError>
}

pub fn human<S: Show>(error: S) -> Box<CargoError> {
    box ConcreteCargoError {
        description: error.to_string(),
        detail: None,
        cause: None,
        is_human: true
    } as Box<CargoError>
}

pub fn caused_human<S: Show, E: CargoError>(error: S, cause: E) -> Box<CargoError> {
    box ConcreteCargoError {
        description: error.to_string(),
        detail: None,
        cause: Some(box cause as Box<CargoError>),
        is_human: true
    } as Box<CargoError>
}
