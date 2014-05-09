use util::{other_error,CargoResult,CargoError};

pub fn find_project(pwd: Path, file: ~str) -> CargoResult<Path> {
    let mut current = pwd.clone();

    loop {
        if current.join(file.clone()).exists() {
            return Ok(current)
        }

        if !current.pop() { break; }
    }

    Err(manifest_missing_err(&pwd, file.as_slice()))
}

fn manifest_missing_err(pwd: &Path, file: &str) -> CargoError {
    other_error("manifest not found")
        .with_detail(format!("pwd={}; file={}", pwd.display(), file))
}
