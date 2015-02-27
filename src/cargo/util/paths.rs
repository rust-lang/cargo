use std::env;
use std::dynamic_lib::DynamicLibrary;
use std::ffi::{AsOsStr, OsString};
use std::path::{Path, PathBuf, Component};

use util::{human, internal, CargoResult, ChainError};

pub fn join_paths<T: AsOsStr>(paths: &[T], env: &str) -> CargoResult<OsString> {
    env::join_paths(paths.iter()).or_else(|e| {
        let paths = paths.iter().map(|p| {
            Path::new(p.as_os_str())
        }).collect::<Vec<_>>();
        internal(format!("failed to join path array: {:?}", paths)).chain_error(|| {
            human(format!("failed to join search paths together: {}\n\
                           Does ${} have an unterminated quote character?",
                          e, env))
        })
    })
}

pub fn dylib_path() -> Vec<PathBuf> {
    match env::var_os(DynamicLibrary::envvar()) {
        Some(var) => env::split_paths(&var).collect(),
        None => Vec::new(),
    }
}

pub fn normalize_path(path: &Path) -> PathBuf {
    let mut components = path.components();
    let mut ret = if let Some(c @ Component::Prefix { .. }) = components.peek() {
        components.next();
        PathBuf::new(c.as_os_str())
    } else {
        PathBuf::new("")
    };

    for component in components {
        match component {
            Component::Prefix { .. } => unreachable!(),
            Component::Empty => { ret.push(""); }
            Component::RootDir => { ret.push(component.as_os_str()); }
            Component::CurDir => {}
            Component::ParentDir => { ret.pop(); }
            Component::Normal(c) => { ret.push(c); }
        }
    }
    return ret;
}

/// Chop off the trailing slash of a path
pub fn lose_the_slash(path: &Path) -> &Path {
    let mut components = path.components();
    match components.next_back() {
        Some(Component::CurDir) => components.as_path(),
        _ => path,
    }
}

#[cfg(unix)]
pub fn path2bytes(path: &Path) -> CargoResult<&[u8]> {
    use std::os::unix::prelude::*;
    Ok(path.as_os_str().as_bytes())
}
#[cfg(windows)]
pub fn path2bytes(path: &Path) -> CargoResult<&[u8]> {
    match path.as_os_str().to_str() {
        Some(s) => Ok(s.as_bytes()),
        None => Err(human(format!("invalid non-unicode path: {}",
                                  path.display())))
    }
}

#[cfg(unix)]
pub fn bytes2path(bytes: &[u8]) -> CargoResult<PathBuf> {
    use std::os::unix::prelude::*;
    use std::ffi::OsStr;
    Ok(PathBuf::new(<OsStr as OsStrExt>::from_bytes(bytes)))
}
#[cfg(windows)]
pub fn bytes2path(bytes: &[u8]) -> CargoResult<PathBuf> {
    use std::str;
    match str::from_utf8(bytes) {
        Ok(s) => Ok(PathBuf::new(s)),
        Err(..) => Err(human("invalid non-unicode path")),
    }
}
