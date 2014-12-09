use util::errors::{CargoResult, CargoError};

pub trait Wrap {
    fn wrap<E: CargoError>(self, error: E) -> Self;
}

impl<T> Wrap for Result<T, Box<CargoError>> {
    fn wrap<E: CargoError>(self, error: E) -> CargoResult<T> {
        match self {
            Ok(x) => Ok(x),
            Err(e) => Err(error.concrete().with_cause(e))
        }
    }
}

pub trait Require<T> {
    fn require<E: CargoError>(self, err: || -> E) -> CargoResult<T>;
}

impl<T> Require<T> for Option<T> {
    fn require<E: CargoError>(self, err: || -> E) -> CargoResult<T> {
        match self {
            Some(x) => Ok(x),
            None => Err(box err().concrete() as Box<CargoError>)
        }
    }
}
