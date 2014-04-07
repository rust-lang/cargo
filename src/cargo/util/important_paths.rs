use std::os;
use super::super::{CargoResult,CargoError};

pub fn find_project(pwd: Path, file: ~str) -> CargoResult<Path> {
    let mut current = pwd.clone();

    loop {
        if current.join(file.clone()).exists() {
            return Ok(current)
        }

        if !current.pop() { break; }
    }

    Err(CargoError::new(format!("Could not find a Cargo manifest ({}) in your current directory or any parent directory", file), 1))
}
