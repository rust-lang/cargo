use std::io;
use core::errors::{CLIError,CLIResult};
use toml;

/*
 * CargoResult should be used in libcargo. CargoCliResult should be used in the
 * various executables.
 */

pub type CargoResult<T> = Result<T, CargoError>;

pub fn other_error(desc: &'static str) -> CargoError {
    CargoError {
        kind: OtherCargoError,
        desc: StaticDescription(desc),
        detail: None,
        cause: None
    }
}

pub fn human_error(desc: ~str, detail: ~str, cause: CargoError) -> CargoError {
    CargoError {
        kind: HumanReadableError,
        desc: BoxedDescription(desc),
        detail: Some(detail),
        cause: Some(box cause)
    }
}

pub fn toml_error(desc: &'static str, error: toml::Error) -> CargoError {
    CargoError {
        kind: TomlError(error),
        desc: StaticDescription(desc),
        detail: None,
        cause: None
    }
}

#[deriving(Show,Clone)]
pub struct CargoError {
    kind: CargoErrorKind,
    desc: CargoErrorDescription,
    detail: Option<~str>,
    cause: Option<Box<CargoError>>
}

#[deriving(Show,Clone)]
enum CargoErrorDescription {
    StaticDescription(&'static str),
    BoxedDescription(~str)
}

impl CargoError {
    pub fn get_desc<'a>(&'a self) -> &'a str {
        match self.desc {
            StaticDescription(desc) => desc,
            BoxedDescription(ref desc) => desc.as_slice()
        }
    }

    pub fn get_detail<'a>(&'a self) -> Option<&'a str> {
        self.detail.as_ref().map(|s| s.as_slice())
    }

    pub fn with_detail(mut self, detail: ~str) -> CargoError {
        self.detail = Some(detail);
        self
    }

    pub fn to_cli(self, exit_code: uint) -> CLIError {
        match self {
            CargoError { kind: HumanReadableError, desc: BoxedDescription(desc), detail: detail, .. } => {
                CLIError::new(desc, detail, exit_code)
            },
            CargoError { kind: InternalError, desc: StaticDescription(desc), detail: None, .. } => {
                CLIError::new("An unexpected error occurred", Some(desc.to_owned()), exit_code)
            },
            CargoError { kind: InternalError, desc: StaticDescription(desc), detail: Some(detail), .. } => {
                CLIError::new("An unexpected error occurred", Some(format!("{}\n{}", desc, detail)), exit_code)
            },
            _ => {
                CLIError::new("An unexpected error occurred", None, exit_code)
            }
        }
    }
}

#[deriving(Show,Clone)]
pub enum CargoErrorKind {
    HumanReadableError,
    InternalError,
    IoError(io::IoError),
    TomlError(toml::Error),
    OtherCargoError
}

type CargoCliResult<T> = Result<T, CargoCliError>;

#[deriving(Show,Clone)]
pub struct CargoCliError {
    kind: CargoCliErrorKind,
    exit_status: uint,
    desc: &'static str,
    detail: Option<~str>,
    cause: Option<CargoError>
}

#[deriving(Show,Clone)]
pub enum CargoCliErrorKind {
    OtherCargoCliError
}

pub trait Wrap {
    fn wrap(self, desc: &'static str) -> Self;
}

impl<T> Wrap for Result<T, CargoError> {
    fn wrap(self, desc: &'static str) -> Result<T, CargoError> {
        match self {
            Ok(x) => Ok(x),
            Err(e) => {
                Err(CargoError {
                    kind: e.kind.clone(),
                    desc: StaticDescription(desc),
                    detail: None,
                    cause: Some(box e)
                })
            }
        }
    }
}

pub trait Require<T> {
    fn require(self, err: CargoError) -> CargoResult<T>;
}

impl<T> Require<T> for Option<T> {
    fn require(self, err: CargoError) -> CargoResult<T> {
        match self {
            Some(x) => Ok(x),
            None => Err(err)
        }
    }
}

pub trait ToCLI<T> {
    fn to_cli(self, exit_code: uint) -> CLIResult<T>;
}

impl<T> ToCLI<T> for Result<T, CargoError> {
    fn to_cli(self, exit_code: uint) -> CLIResult<T> {
        match self {
            Ok(val) => Ok(val),
            Err(err) => Err(err.to_cli(exit_code))
        }
    }
}
