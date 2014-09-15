use std::io::File;
use std::io::fs::PathExtensions;

use tar::Archive;
use flate2::{GzBuilder, BestCompression};

use core::source::Source;
use core::{Package, MultiShell};
use sources::PathSource;
use util::{CargoResult, human, internal, ChainError, Require};

pub fn package(manifest_path: &Path,
               shell: &mut MultiShell) -> CargoResult<Path> {
    let mut src = try!(PathSource::for_path(&manifest_path.dir_path()));
    try!(src.update());
    let pkg = try!(src.get_root_package());

    let filename = format!("{}-{}.tar.gz", pkg.get_name(), pkg.get_version());
    let dst = pkg.get_manifest_path().dir_path().join(filename);
    try!(shell.status("Packaging", pkg.get_package_id().to_string()));
    try!(tar(&pkg, &src, shell, &dst).chain_error(|| {
        human("failed to prepare local package for uploading")
    }));

    Ok(dst)
}

fn tar(pkg: &Package, src: &PathSource, shell: &mut MultiShell,
       dst: &Path) -> CargoResult<()> {

    if dst.exists() {
        return Err(human(format!("destination already exists: {}",
                                 dst.display())))
    }
    let tmpfile = try!(File::create(dst));

    // Prepare the encoder and its header
    let encoder = GzBuilder::new().filename(dst.filename().unwrap())
                                  .writer(tmpfile, BestCompression);

    // Put all package files into a compressed archive
    let ar = Archive::new(encoder);
    for file in try!(src.list_files(pkg)).iter() {
        let relative = file.path_relative_from(&dst.dir_path()).unwrap();
        let relative = try!(relative.as_str().require(|| {
            human(format!("non-utf8 path in source directory: {}",
                          relative.display()))
        }));
        let mut file = try!(File::open(file));
        try!(shell.verbose(|shell| {
            shell.status("Archiving", relative.as_slice())
        }));
        let path = format!("{}-{}/{}", pkg.get_name(),
                           pkg.get_version(), relative);
        try!(ar.append(path.as_slice(), &mut file).chain_error(|| {
            internal(format!("could not archive source file `{}`", relative))
        }));
    }

    Ok(())
}
