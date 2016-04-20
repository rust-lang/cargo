use std::error::Error;
use std::fmt;
use std::str;

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
// Human errors

#[derive(Debug)]
pub struct Human<E>(E);

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

impl<E: CargoError> From<Human<E>> for Box<CargoError> {
    fn from(t: Human<E>) -> Box<CargoError> { Box::new(t) }
}

// =============================================================================
// Internal errors

#[derive(Debug)]
pub struct Internal<E>(E);

impl<E: Error> Error for Internal<E> {
    fn description(&self) -> &str { self.0.description() }
    fn cause(&self) -> Option<&Error> { self.0.cause() }
}

impl<E: fmt::Display> fmt::Display for Internal<E> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl<E: CargoError> CargoError for Internal<E> {
    fn is_human(&self) -> bool { false }
    fn cargo_cause(&self) -> Option<&CargoError> { self.0.cargo_cause() }
}

impl<E: CargoError> From<Internal<E>> for Box<CargoError> {
    fn from(t: Internal<E>) -> Box<CargoError> { Box::new(t) }
}

// =============================================================================
// Concrete errors

struct ConcreteCargoError {
    description: String,
    cause: Option<Box<Error+Send>>,
    is_human: bool,
}

impl fmt::Display for ConcreteCargoError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(f, "{}", self.description));
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
// Strings as errors

#[derive(Debug)]
pub struct StringError(String);

impl Error for StringError {
    fn description(&self) -> &str { &self.0 }
    fn cause(&self) -> Option<&Error> { None }
}

impl fmt::Display for StringError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl CargoError for StringError {
    fn is_human(&self) -> bool { true }
    fn cargo_cause(&self) -> Option<&CargoError> { None }
}

impl From<StringError> for Box<CargoError> {
    fn from(t: StringError) -> Box<CargoError> { Box::new(t) }
}

// =============================================================================
// Stuff

#[macro_export]
macro_rules! from_error {
    ($($p:ty,)*) => (
        $(impl From<$p> for Box<CargoError> {
            fn from(t: $p) -> Box<CargoError> { Box::new(t) }
        })*
    )
}

pub fn internal<S: fmt::Display>(error: S) -> Box<CargoError> {
    Box::new(Internal(StringError(error.to_string())))
}

pub fn caused_internal<S, E>(error: S, cause: E) -> Box<CargoError>
    where S: fmt::Display,
          E: Error + Send + 'static
{
    Box::new(ChainedError {
        error: internal(error),
        cause: Box::new(ConcreteCargoError {
            description: cause.description().to_string(),
            cause: Some(Box::new(cause)),
            is_human: false,
        })
    })
}

pub fn human<S: fmt::Display>(error: S) -> Box<CargoError> {
    Box::new(Human(StringError(error.to_string())))
}

pub fn caused_human<S, E>(error: S, cause: E) -> Box<CargoError>
    where S: fmt::Display,
          E: Error + Send + 'static
{
    Box::new(ChainedError {
        error: human(error),
        cause: Box::new(ConcreteCargoError {
            description: cause.description().to_string(),
            cause: Some(Box::new(cause)),
            is_human: false,
        })
    })
}
