use util::{CargoResult, human};

pub fn find_project(pwd: Path, file: &str) -> CargoResult<Path> {
    let mut current = pwd.clone();

    loop {
        if current.join(file.clone()).exists() {
            return Ok(current)
        }

        if !current.pop() { break; }
    }

    Err(human(format!("no manifest found in `{}`", pwd.display())))
}
