use core::errors::{CargoResult,CargoError,MissingManifest};

pub fn find_project(pwd: Path, file: ~str) -> CargoResult<Path> {
    let mut current = pwd.clone();

    loop {
        if current.join(file.clone()).exists() {
            return Ok(current)
        }

        if !current.pop() { break; }
    }

    Err(CargoError::internal(MissingManifest(pwd, file)))
}
