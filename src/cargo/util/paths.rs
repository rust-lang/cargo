use std::{io,os};
use std::io::fs;

pub fn realpath(original: &Path) -> io::IoResult<Path> {
    static MAX_LINKS_FOLLOWED: uint = 256;
    let original = os::make_absolute(original);

    // Right now lstat on windows doesn't work quite well
    if cfg!(windows) {
        return Ok(original)
    }

    let result = original.root_path();
    let mut result = result.expect("make_absolute has no root_path");
    let mut followed = 0;

    for part in original.components() {
        result.push(part);

        loop {
            if followed == MAX_LINKS_FOLLOWED {
                return Err(io::standard_error(io::InvalidInput))
            }

            match fs::lstat(&result) {
                Err(..) => break,
                Ok(ref stat) if stat.kind != io::TypeSymlink => break,
                Ok(..) => {
                    followed += 1;
                    let path = try!(fs::readlink(&result));
                    result.pop();
                    result.push(path);
                }
            }
        }
    }

    return Ok(result);
}

