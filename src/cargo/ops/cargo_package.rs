use std::io::{fs, File, USER_DIR};
use std::io::fs::PathExtensions;
use std::path;

use tar::Archive;
use flate2::{GzBuilder, BestCompression};
use flate2::reader::GzDecoder;

use core::source::{Source, SourceId};
use core::{Package, MultiShell};
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
               verify: bool,
               list: bool,
               metadata: bool) -> CargoResult<Option<Path>> {
    let mut src = try!(PathSource::for_path(&manifest_path.dir_path()));
    try!(src.update());
    let pkg = try!(src.get_root_package());

    if metadata {
        try!(check_metadata(&pkg, shell));
    }

    if list {
        let root = pkg.get_manifest_path().dir_path();
        let mut list: Vec<_> = try!(src.list_files(&pkg)).iter().map(|file| {
            file.path_relative_from(&root).unwrap()
        }).collect();
        list.sort();
        for file in list.iter() {
            println!("{}", file.display());
        }
        return Ok(None)
    }

    let filename = format!("package/{}-{}.crate", pkg.get_name(),
                           pkg.get_version());
    let dst = pkg.get_absolute_target_dir().join(filename);
    if dst.exists() { return Ok(Some(dst)) }

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
    Ok(Some(bomb.path.take().unwrap()))
}

// check that the package has some piece of metadata that a human can
// use to tell what the package is about.
fn check_metadata(pkg: &Package, shell: &mut MultiShell) -> CargoResult<()> {
    let md = pkg.get_manifest().get_metadata();

    let mut missing = vec![];

    macro_rules! lacking {
        ($( $($field: ident)||* ),*) => {{
            $(
                if $(md.$field.as_ref().map_or(true, |s| s.is_empty()))&&* {
                    $(missing.push(stringify!($field).replace("_", "-"));)*
                }
            )*
        }}
    }
    lacking!(description, license || license_file, documentation || homepage || repository)

    if !missing.is_empty() {
        let mut things = missing.slice_to(missing.len() - 1).connect(", ");
        // things will be empty if and only if length == 1 (i.e. the only case to have no `or`).
        if !things.is_empty() {
            things.push_str(" or ");
        }
        things.push_str(missing.last().unwrap().as_slice());

        try!(shell.warn(
            format!("warning: manifest has no {things}. \
                    See http://doc.crates.io/manifest.html#package-metadata for more info.",
                    things = things).as_slice()))
    }
    Ok(())
}

fn tar(pkg: &Package, src: &PathSource, shell: &mut MultiShell,
       dst: &Path) -> CargoResult<()> {

    if dst.exists() {
        return Err(human(format!("destination already exists: {}",
                                 dst.display())))
    }

    try!(fs::mkdir_recursive(&dst.dir_path(), USER_DIR))

    let tmpfile = try!(File::create(dst));

    // Prepare the encoder and its header
    let encoder = GzBuilder::new().filename(dst.filename().unwrap())
                                  .writer(tmpfile, BestCompression);

    // Put all package files into a compressed archive
    let ar = Archive::new(encoder);
    let root = pkg.get_manifest_path().dir_path();
    for file in try!(src.list_files(pkg)).iter() {
        if file == dst { continue }
        let relative = file.path_relative_from(&root).unwrap();
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
    let dst = pkg.get_root().join(format!("target/package/{}-{}",
                                          pkg.get_name(), pkg.get_version()));
    if dst.exists() {
        try!(fs::rmdir_recursive(&dst));
    }
    let mut archive = Archive::new(f);
    try!(archive.unpack(&dst.dir_path()));
    let manifest_path = dst.join("Cargo.toml");

    // When packages are uploaded to the registry, all path dependencies are
    // implicitly converted to registry-based dependencies, so we rewrite those
    // dependencies here.
    let registry = try!(SourceId::for_central());
    let new_summary = pkg.get_summary().clone().map_dependencies(|d| {
        if !d.get_source_id().is_path() { return d }
        d.source_id(registry.clone())
    });
    let mut new_manifest = pkg.get_manifest().clone();
    new_manifest.set_summary(new_summary);
    new_manifest.set_target_dir(dst.join("target"));
    let new_pkg = Package::new(new_manifest, &manifest_path,
                               pkg.get_package_id().get_source_id());

    // Now that we've rewritten all our path dependencies, compile it!
    try!(ops::compile_pkg(&new_pkg, &mut ops::CompileOptions {
        env: "compile",
        shell: shell,
        jobs: None,
        target: None,
        dev_deps: false,
        features: &[],
        no_default_features: false,
        spec: None,
        lib_only: false,
    }));

    Ok(())
}
