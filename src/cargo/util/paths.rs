use std::env;
use std::old_io::fs;
use std::old_io;
use std::old_path::BytesContainer;
use std::os;

use util::{human, internal, CargoResult, ChainError};

pub fn realpath(original: &Path) -> old_io::IoResult<Path> {
    const MAX_LINKS_FOLLOWED: usize = 256;
    let cwd = try!(env::current_dir());
    let original = cwd.join(original);

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
                return Err(old_io::standard_error(old_io::InvalidInput))
            }

            match fs::lstat(&result) {
                Err(..) => break,
                Ok(ref stat) if stat.kind != old_io::FileType::Symlink => break,
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

#[allow(deprecated)] // need an OsStr-based Command first
pub fn join_paths<T: BytesContainer>(paths: &[T], env: &str)
                                     -> CargoResult<Vec<u8>> {
    os::join_paths(paths).or_else(|e| {
        let paths = paths.iter().map(|p| Path::new(p)).collect::<Vec<_>>();
        internal(format!("failed to join path array: {:?}", paths)).chain_error(|| {
            human(format!("failed to join search paths together: {}\n\
                           Does ${} have an unterminated quote character?",
                          e, env))
        })
    })
}
