use std::io::{fs, File};
use std::io::fs::PathExtensions;
use std::path;

use tar::Archive;
use flate2::{GzBuilder, BestCompression};
use flate2::reader::GzDecoder;

use core::source::{Source, SourceId};
use core::{Package, MultiShell, Summary, Dependency};
use sources::PathSource;
use util::{CargoResult, human, internal, ChainError, Require};
use ops;

struct Bomb { path: Option<Path> }

impl Drop for Bomb {
    fn drop(&mut self) {
        match self.path.as_ref() {
            Some(path) => { let _ = fs::unlink(path); }
            None => {}
        }
    }
}

pub fn package(manifest_path: &Path,
               shell: &mut MultiShell,
               verify: bool) -> CargoResult<Path> {
    let mut src = try!(PathSource::for_path(&manifest_path.dir_path()));
    try!(src.update());
    let pkg = try!(src.get_root_package());

    let filename = format!("{}-{}.tar.gz", pkg.get_name(), pkg.get_version());
    let dst = pkg.get_manifest_path().dir_path().join(filename);
    if dst.exists() { return Ok(dst) }

    let mut bomb = Bomb { path: Some(dst.clone()) };

    try!(shell.status("Packaging", pkg.get_package_id().to_string()));
    try!(tar(&pkg, &src, shell, &dst).chain_error(|| {
        human("failed to prepare local package for uploading")
    }));
    if verify {
        try!(run_verify(&pkg, shell, &dst).chain_error(|| {
            human("failed to verify package tarball")
        }))
    }
    Ok(bomb.path.take().unwrap())
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
        if file == dst { continue }
        let relative = file.path_relative_from(&dst.dir_path()).unwrap();
        let relative = try!(relative.as_str().require(|| {
            human(format!("non-utf8 path in source directory: {}",
                          relative.display()))
        }));
        let mut file = try!(File::open(file));
        try!(shell.verbose(|shell| {
            shell.status("Archiving", relative.as_slice())
        }));
        let path = format!("{}-{}{}{}", pkg.get_name(),
                           pkg.get_version(), path::SEP, relative);
        try!(ar.append(path.as_slice(), &mut file).chain_error(|| {
            internal(format!("could not archive source file `{}`", relative))
        }));
    }
    try!(ar.finish());
    Ok(())
}

fn run_verify(pkg: &Package, shell: &mut MultiShell, tar: &Path)
              -> CargoResult<()> {
    try!(shell.status("Verifying", pkg));

    let f = try!(GzDecoder::new(try!(File::open(tar))));
    let dst = pkg.get_root().join("target/package");
    if dst.exists() {
        try!(fs::rmdir_recursive(&dst));
    }
    let mut archive = Archive::new(f);
    try!(archive.unpack(&dst));
    let manifest_path = dst.join(format!("{}-{}/Cargo.toml", pkg.get_name(),
                                         pkg.get_version()));

    // When packages are uploaded to the registry, all path dependencies are
    // implicitly converted to registry-based dependencies, so we rewrite those
    // dependencies here.
    let registry = try!(SourceId::for_central());
    let new_deps = pkg.get_dependencies().iter().map(|d| {
        if !d.get_source_id().is_path() { return d.clone() }
        Dependency::parse(d.get_name(), d.get_specified_req(), &registry)
                   .unwrap()
                   .transitive(d.is_transitive())
                   .features(d.get_features().to_vec())
                   .default_features(d.uses_default_features())
                   .optional(d.is_optional())
    }).collect::<Vec<_>>();
    let new_summary = Summary::new(pkg.get_package_id().clone(),
                                   new_deps,
                                   pkg.get_summary().get_features().clone());
    let mut new_manifest = pkg.get_manifest().clone();
    new_manifest.set_summary(new_summary.unwrap());
    let new_pkg = Package::new(new_manifest,
                               &manifest_path,
                               pkg.get_package_id().get_source_id());

    // Now that we've rewritten all our path dependencies, compile it!
    try!(ops::compile_pkg(&new_pkg, &mut ops::CompileOptions {
        env: "compile",
        shell: shell,
        jobs: None,
        target: None,
        dev_deps: false,
        features: [],
        no_default_features: false,
        spec: None,
    }));

    Ok(())
}
