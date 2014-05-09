use std::io;

pub type CargoResult<T> = Result<T, CargoError>;

pub fn other_error(desc: &'static str) -> CargoError {
    CargoError {
        kind: OtherCargoError,
        desc: desc,
        detail: None,
        cause: None
    }
}

#[deriving(Show,Clone)]
pub struct CargoError {
    kind: CargoErrorKind,
    desc: &'static str,
    detail: Option<~str>,
    cause: Option<Box<CargoError>>
}

impl CargoError {
    pub fn get_desc(&self) -> &'static str {
        self.desc
    }

    pub fn get_detail<'a>(&'a self) -> Option<&'a str> {
        self.detail.as_ref().map(|s| s.as_slice())
    }

    pub fn with_detail(mut self, detail: ~str) -> CargoError {
        self.detail = Some(detail);
        self
    }
}

#[deriving(Show,Clone)]
pub enum CargoErrorKind {
    InternalError,
    IoError(io::IoError),
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
        self
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
