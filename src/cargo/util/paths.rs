use std::{io,os};
use std::io::fs;
use std::path::BytesContainer;

use util::{human, CargoResult};

pub fn realpath(original: &Path) -> io::IoResult<Path> {
    const MAX_LINKS_FOLLOWED: usize = 256;
    let original = try!(os::make_absolute(original));

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
                Ok(ref stat) if stat.kind != io::FileType::Symlink => break,
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

pub fn join_paths<T: BytesContainer>(paths: &[T], env: &str)
                                     -> CargoResult<Vec<u8>> {
    os::join_paths(paths).map_err(|e| {
        human(format!("failed to join search paths together: {}\n\
                       Does ${} have an unterminated quote character?", e, env))
    })
}
