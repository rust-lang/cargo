use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::process::Command;

use core::PackageIdSpec;
use core::source::Source;
use ops;
use sources::PathSource;
use util::{CargoResult, human};

pub struct DocOptions<'a, 'b: 'a> {
    pub open_result: bool,
    pub compile_opts: ops::CompileOptions<'a, 'b>,
}

pub fn doc(manifest_path: &Path,
           options: &DocOptions) -> CargoResult<()> {
    let mut source = try!(PathSource::for_path(manifest_path.parent().unwrap(),
                                               options.compile_opts.config));
    try!(source.update());
    let package = try!(source.root_package());

    let mut lib_names = HashSet::new();
    let mut bin_names = HashSet::new();
    if options.compile_opts.spec.is_none() {
        for target in package.targets().iter().filter(|t| t.documented()) {
            if target.is_lib() {
                assert!(lib_names.insert(target.name()));
            } else {
                assert!(bin_names.insert(target.name()));
            }
        }
        for bin in bin_names.iter() {
            if lib_names.contains(bin) {
                return Err(human("Cannot document a package where a library \
                                  and a binary have the same name. Consider \
                                  renaming one or marking the target as \
                                  `doc = false`"))
            }
        }
    }

    try!(ops::compile(manifest_path, &options.compile_opts));

    if options.open_result {
        let name = match options.compile_opts.spec {
            Some(spec) => try!(PackageIdSpec::parse(spec)).name().to_string(),
            None => {
                match lib_names.iter().nth(0) {
                    Some(s) => s.to_string(),
                    None => return Ok(())
                }
            }
        };

        let path = package.absolute_target_dir().join("doc").join(&name)
                                                    .join("index.html");
        if fs::metadata(&path).is_ok() {
            open_docs(&path);
        }
    }

    Ok(())
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn open_docs(path: &Path) {
    // trying xdg-open
    match Command::new("xdg-open").arg(path).status() {
        Ok(_) => return,
        Err(_) => ()
    };

    // trying gnome-open
    match Command::new("gnome-open").arg(path).status() {
        Ok(_) => return,
        Err(_) => ()
    };

    // trying kde-open
    match Command::new("kde-open").arg(path).status() {
        Ok(_) => return,
        Err(_) => ()
    };
}

#[cfg(target_os = "windows")]
fn open_docs(path: &Path) {
    match Command::new("start").arg(path).status() {
        Ok(_) => return,
        Err(_) => ()
    };
}

#[cfg(target_os = "macos")]
fn open_docs(path: &Path) {
    match Command::new("open").arg(path).status() {
        Ok(_) => return,
        Err(_) => ()
    };
}
