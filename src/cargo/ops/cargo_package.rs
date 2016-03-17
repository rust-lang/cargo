use std::io::prelude::*;
use std::fs::{self, File};
use std::path::{self, Path, PathBuf};

use tar::{Archive, Builder, Header};
use flate2::{GzBuilder, Compression};
use flate2::read::GzDecoder;

use core::{SourceId, Package, PackageId};
use sources::PathSource;
use util::{self, CargoResult, human, internal, ChainError, Config};
use ops;

pub fn package(manifest_path: &Path,
               config: &Config,
               verify: bool,
               list: bool,
               metadata: bool) -> CargoResult<Option<PathBuf>> {
    let path = manifest_path.parent().unwrap();
    let id = try!(SourceId::for_path(path));
    let mut src = PathSource::new(path, &id, config);
    let pkg = try!(src.root_package());

    if metadata {
        try!(check_metadata(&pkg, config));
    }

    if list {
        let root = pkg.root();
        let mut list: Vec<_> = try!(src.list_files(&pkg)).iter().map(|file| {
            util::without_prefix(&file, &root).unwrap().to_path_buf()
        }).collect();
        list.sort();
        for file in list.iter() {
            println!("{}", file.display());
        }
        return Ok(None)
    }

    let filename = format!("{}-{}.crate", pkg.name(), pkg.version());
    let dir = config.target_dir(&pkg).join("package");
    let dst = dir.join(&filename);
    if fs::metadata(&dst).is_ok() {
        return Ok(Some(dst))
    }

    // Package up and test a temporary tarball and only move it to the final
    // location if it actually passes all our tests. Any previously existing
    // tarball can be assumed as corrupt or invalid, so we just blow it away if
    // it exists.
    try!(config.shell().status("Packaging", pkg.package_id().to_string()));
    let tmp_dst = dir.join(format!(".{}", filename));
    let _ = fs::remove_file(&tmp_dst);
    try!(tar(&pkg, &src, config, &tmp_dst, &filename).chain_error(|| {
        human("failed to prepare local package for uploading")
    }));
    if verify {
        try!(run_verify(config, &pkg, &tmp_dst).chain_error(|| {
            human("failed to verify package tarball")
        }))
    }
    try!(fs::rename(&tmp_dst, &dst).chain_error(|| {
        human("failed to move temporary tarball into final location")
    }));
    Ok(Some(dst))
}

// check that the package has some piece of metadata that a human can
// use to tell what the package is about.
fn check_metadata(pkg: &Package, config: &Config) -> CargoResult<()> {
    let md = pkg.manifest().metadata();

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
    lacking!(description, license || license_file, documentation || homepage || repository);

    if !missing.is_empty() {
        let mut things = missing[..missing.len() - 1].join(", ");
        // things will be empty if and only if length == 1 (i.e. the only case
        // to have no `or`).
        if !things.is_empty() {
            things.push_str(" or ");
        }
        things.push_str(&missing.last().unwrap());

        try!(config.shell().warn(
            &format!("manifest has no {things}. \
                    See http://doc.crates.io/manifest.html#package-metadata for more info.",
                    things = things)))
    }
    Ok(())
}

fn tar(pkg: &Package,
       src: &PathSource,
       config: &Config,
       dst: &Path,
       filename: &str) -> CargoResult<()> {
    if fs::metadata(&dst).is_ok() {
        bail!("destination already exists: {}", dst.display())
    }

    try!(fs::create_dir_all(dst.parent().unwrap()));

    let tmpfile = try!(File::create(dst));

    // Prepare the encoder and its header
    let filename = Path::new(filename);
    let encoder = GzBuilder::new().filename(try!(util::path2bytes(filename)))
                                  .write(tmpfile, Compression::Best);

    // Put all package files into a compressed archive
    let mut ar = Builder::new(encoder);
    let root = pkg.root();
    for file in try!(src.list_files(pkg)).iter() {
        if &**file == dst { continue }
        let relative = util::without_prefix(&file, &root).unwrap();
        try!(check_filename(relative));
        let relative = try!(relative.to_str().chain_error(|| {
            human(format!("non-utf8 path in source directory: {}",
                          relative.display()))
        }));
        let mut file = try!(File::open(file));
        try!(config.shell().verbose(|shell| {
            shell.status("Archiving", &relative)
        }));
        let path = format!("{}-{}{}{}", pkg.name(), pkg.version(),
                           path::MAIN_SEPARATOR, relative);

        // The tar::Builder type by default will build GNU archives, but
        // unfortunately we force it here to use UStar archives instead. The
        // UStar format has more limitations on the length of path name that it
        // can encode, so it's not quite as nice to use.
        //
        // Older cargos, however, had a bug where GNU archives were interpreted
        // as UStar archives. This bug means that if we publish a GNU archive
        // which has fully filled out metadata it'll be corrupt when unpacked by
        // older cargos.
        //
        // Hopefully in the future after enough cargos have been running around
        // with the bugfixed tar-rs library we'll be able to switch this over to
        // GNU archives, but for now we'll just say that you can't encode paths
        // in archives that are *too* long.
        //
        // For an instance of this in the wild, use the tar-rs 0.3.3 library to
        // unpack the selectors 0.4.0 crate on crates.io. Either that or take a
        // look at rust-lang/cargo#2326
        let mut header = Header::new_ustar();
        let metadata = try!(file.metadata().chain_error(|| {
            human(format!("could not learn metadata for: `{}`", relative))
        }));
        try!(header.set_path(&path).chain_error(|| {
            human(format!("failed to add to archive: `{}`", relative))
        }));
        header.set_metadata(&metadata);
        header.set_cksum();

        try!(ar.append(&header, &mut file).chain_error(|| {
            internal(format!("could not archive source file `{}`", relative))
        }));
    }
    let encoder = try!(ar.into_inner());
    try!(encoder.finish());
    Ok(())
}

fn run_verify(config: &Config, pkg: &Package, tar: &Path)
              -> CargoResult<()> {
    try!(config.shell().status("Verifying", pkg));

    let f = try!(GzDecoder::new(try!(File::open(tar))));
    let dst = pkg.root().join(&format!("target/package/{}-{}",
                                       pkg.name(), pkg.version()));
    if fs::metadata(&dst).is_ok() {
        try!(fs::remove_dir_all(&dst));
    }
    let mut archive = Archive::new(f);
    try!(archive.unpack(dst.parent().unwrap()));
    let manifest_path = dst.join("Cargo.toml");

    // When packages are uploaded to the registry, all path dependencies are
    // implicitly converted to registry-based dependencies, so we rewrite those
    // dependencies here.
    //
    // We also make sure to point all paths at `dst` instead of the previous
    // location that the package was originally read from. In locking the
    // `SourceId` we're telling it that the corresponding `PathSource` will be
    // considered updated and we won't actually read any packages.
    let registry = try!(SourceId::for_central(config));
    let precise = Some("locked".to_string());
    let new_src = try!(SourceId::for_path(&dst)).with_precise(precise);
    let new_pkgid = try!(PackageId::new(pkg.name(), pkg.version(), &new_src));
    let new_summary = pkg.summary().clone().map_dependencies(|d| {
        if !d.source_id().is_path() { return d }
        d.clone_inner().set_source_id(registry.clone()).into_dependency()
    });
    let mut new_manifest = pkg.manifest().clone();
    new_manifest.set_summary(new_summary.override_id(new_pkgid));
    let new_pkg = Package::new(new_manifest, &manifest_path);

    // Now that we've rewritten all our path dependencies, compile it!
    try!(ops::compile_pkg(&new_pkg, None, &ops::CompileOptions {
        config: config,
        jobs: None,
        target: None,
        features: &[],
        no_default_features: false,
        spec: &[],
        filter: ops::CompileFilter::Everything,
        exec_engine: None,
        release: false,
        mode: ops::CompileMode::Build,
        target_rustdoc_args: None,
        target_rustc_args: None,
    }));

    Ok(())
}

// It can often be the case that files of a particular name on one platform
// can't actually be created on another platform. For example files with colons
// in the name are allowed on Unix but not on Windows.
//
// To help out in situations like this, issue about weird filenames when
// packaging as a "heads up" that something may not work on other platforms.
fn check_filename(file: &Path) -> CargoResult<()> {
    let name = match file.file_name() {
        Some(name) => name,
        None => return Ok(()),
    };
    let name = match name.to_str() {
        Some(name) => name,
        None => {
            bail!("path does not have a unicode filename which may not unpack \
                   on all platforms: {}", file.display())
        }
    };
    let bad_chars = ['/', '\\', '<', '>', ':', '"', '|', '?', '*'];
    for c in bad_chars.iter().filter(|c| name.contains(**c)) {
        bail!("cannot package a filename with a special character `{}`: {}",
              c, file.display())
    }
    Ok(())
}
