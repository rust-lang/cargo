use util::{CargoResult, CargoError, internal_error};

pub fn find_project(pwd: Path, file: &str) -> CargoResult<Path> {
    let mut current = pwd.clone();

    loop {
        if current.join(file.clone()).exists() {
            return Ok(current)
        }

        if !current.pop() { break; }
    }

    Err(manifest_missing_err(&pwd, file.as_slice()))
}

fn manifest_missing_err(pwd: &Path, file: &str) -> Box<CargoError> {
    internal_error("manifest not found",
                   format!("pwd={}; file={}", pwd.display(), file))
}
