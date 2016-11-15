use std::fs::{self, File};
use std::io::SeekFrom;
use std::io::prelude::*;
use std::path::{self, Path};

use flate2::read::GzDecoder;
use flate2::{GzBuilder, Compression};
use git2;
use tar::{Archive, Builder, Header};

use core::{SourceId, Package, PackageId, Workspace, Source};
use sources::PathSource;
use util::{self, CargoResult, human, internal, ChainError, Config, FileLock};
use ops;

pub struct PackageOpts<'cfg> {
    pub config: &'cfg Config,
    pub list: bool,
    pub check_metadata: bool,
    pub allow_dirty: bool,
    pub verify: bool,
    pub jobs: Option<u32>,
}

pub fn package(ws: &Workspace,
               opts: &PackageOpts) -> CargoResult<Option<FileLock>> {
    let pkg = ws.current()?;
    let config = ws.config();
    let mut src = PathSource::new(pkg.root(),
                                  pkg.package_id().source_id(),
                                  config);
    src.update()?;

    if opts.check_metadata {
        check_metadata(pkg, config)?;
    }

    verify_dependencies(&pkg)?;

    if opts.list {
        let root = pkg.root();
        let mut list: Vec<_> = src.list_files(&pkg)?.iter().map(|file| {
            util::without_prefix(&file, &root).unwrap().to_path_buf()
        }).collect();
        list.sort();
        for file in list.iter() {
            println!("{}", file.display());
        }
        return Ok(None)
    }

    if !opts.allow_dirty {
        check_not_dirty(&pkg, &src)?;
    }

    let filename = format!("{}-{}.crate", pkg.name(), pkg.version());
    let dir = ws.target_dir().join("package");
    let mut dst = {
        let tmp = format!(".{}", filename);
        dir.open_rw(&tmp, config, "package scratch space")?
    };

    // Package up and test a temporary tarball and only move it to the final
    // location if it actually passes all our tests. Any previously existing
    // tarball can be assumed as corrupt or invalid, so we just blow it away if
    // it exists.
    config.shell().status("Packaging", pkg.package_id().to_string())?;
    dst.file().set_len(0)?;
    tar(ws, &src, dst.file(), &filename).chain_error(|| {
        human("failed to prepare local package for uploading")
    })?;
    if opts.verify {
        dst.seek(SeekFrom::Start(0))?;
        run_verify(ws, dst.file(), opts).chain_error(|| {
            human("failed to verify package tarball")
        })?
    }
    dst.seek(SeekFrom::Start(0))?;
    {
        let src_path = dst.path();
        let dst_path = dst.parent().join(&filename);
        fs::rename(&src_path, &dst_path).chain_error(|| {
            human("failed to move temporary tarball into final location")
        })?;
    }
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

        config.shell().warn(
            &format!("manifest has no {things}.\n\
                    See http://doc.crates.io/manifest.html#package-metadata for more info.",
                    things = things))?
    }
    Ok(())
}

// check that the package dependencies are safe to deploy.
fn verify_dependencies(pkg: &Package) -> CargoResult<()> {
    for dep in pkg.dependencies() {
        if dep.source_id().is_path() {
            if !dep.specified_req() {
                bail!("all path dependencies must have a version specified \
                       when packaging.\ndependency `{}` does not specify \
                       a version.", dep.name())
            }
        }
    }
    Ok(())
}

fn check_not_dirty(p: &Package, src: &PathSource) -> CargoResult<()> {
    if let Ok(repo) = git2::Repository::discover(p.root()) {
        if let Some(workdir) = repo.workdir() {
            debug!("found a git repo at {:?}, checking if index present",
                   workdir);
            let path = p.manifest_path();
            let path = path.strip_prefix(workdir).unwrap_or(path);
            if let Ok(status) = repo.status_file(path) {
                if (status & git2::STATUS_IGNORED).is_empty() {
                    debug!("Cargo.toml found in repo, checking if dirty");
                    return git(p, src, &repo)
                }
            }
        }
    }

    // No VCS recognized, we don't know if the directory is dirty or not, so we
    // have to assume that it's clean.
    return Ok(());

    fn git(p: &Package,
           src: &PathSource,
           repo: &git2::Repository) -> CargoResult<()> {
        let workdir = repo.workdir().unwrap();
        let dirty = src.list_files(p)?.iter().filter(|file| {
            let relative = file.strip_prefix(workdir).unwrap();
            if let Ok(status) = repo.status_file(relative) {
                status != git2::STATUS_CURRENT
            } else {
                false
            }
        }).map(|path| {
            path.strip_prefix(p.root()).unwrap_or(path).display().to_string()
        }).collect::<Vec<_>>();
        if dirty.is_empty() {
            Ok(())
        } else {
            bail!("{} dirty files found in the working directory:\n\n{}\n\n\
                   to proceed despite this, pass the `--allow-dirty` flag",
                  dirty.len(), dirty.join("\n"))
        }
    }
}

fn tar(ws: &Workspace,
       src: &PathSource,
       dst: &File,
       filename: &str) -> CargoResult<()> {
    // Prepare the encoder and its header
    let filename = Path::new(filename);
    let encoder = GzBuilder::new().filename(util::path2bytes(filename)?)
                                  .write(dst, Compression::Best);

    // Put all package files into a compressed archive
    let mut ar = Builder::new(encoder);
    let pkg = ws.current()?;
    let config = ws.config();
    let root = pkg.root();
    for file in src.list_files(pkg)?.iter() {
        let relative = util::without_prefix(&file, &root).unwrap();
        check_filename(relative)?;
        let relative = relative.to_str().chain_error(|| {
            human(format!("non-utf8 path in source directory: {}",
                          relative.display()))
        })?;
        let mut file = File::open(file).chain_error(|| {
            human(format!("failed to open for archiving: `{}`", file.display()))
        })?;
        config.shell().verbose(|shell| {
            shell.status("Archiving", &relative)
        })?;
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
        let metadata = file.metadata().chain_error(|| {
            human(format!("could not learn metadata for: `{}`", relative))
        })?;
        header.set_path(&path).chain_error(|| {
            human(format!("failed to add to archive: `{}`", relative))
        })?;
        header.set_metadata(&metadata);
        header.set_cksum();

        ar.append(&header, &mut file).chain_error(|| {
            internal(format!("could not archive source file `{}`", relative))
        })?;
    }
    let encoder = ar.into_inner()?;
    encoder.finish()?;
    Ok(())
}

fn run_verify(ws: &Workspace, tar: &File, opts: &PackageOpts) -> CargoResult<()> {
    let config = ws.config();
    let pkg = ws.current()?;

    config.shell().status("Verifying", pkg)?;

    let f = GzDecoder::new(tar)?;
    let dst = pkg.root().join(&format!("target/package/{}-{}",
                                       pkg.name(), pkg.version()));
    if fs::metadata(&dst).is_ok() {
        fs::remove_dir_all(&dst)?;
    }
    let mut archive = Archive::new(f);
    archive.unpack(dst.parent().unwrap())?;
    let manifest_path = dst.join("Cargo.toml");

    // When packages are uploaded to a registry, all path dependencies are
    // implicitly converted to registry dependencies, so we rewrite those
    // dependencies here.
    //
    // We also make sure to point all paths at `dst` instead of the previous
    // location that the package was originally read from. In locking the
    // `SourceId` we're telling it that the corresponding `PathSource` will be
    // considered updated and we won't actually read any packages.
    let cratesio = SourceId::crates_io(config)?;
    let precise = Some("locked".to_string());
    let new_src = SourceId::for_path(&dst)?.with_precise(precise);
    let new_pkgid = PackageId::new(pkg.name(), pkg.version(), &new_src)?;
    let new_summary = pkg.summary().clone().map_dependencies(|d| {
        if !d.source_id().is_path() { return d }
        d.clone_inner().set_source_id(cratesio.clone()).into_dependency()
    });
    let mut new_manifest = pkg.manifest().clone();
    new_manifest.set_summary(new_summary.override_id(new_pkgid));
    let new_pkg = Package::new(new_manifest, &manifest_path);

    // Now that we've rewritten all our path dependencies, compile it!
    let ws = Workspace::one(new_pkg, config, None)?;
    ops::compile_ws(&ws, None, &ops::CompileOptions {
        config: config,
        jobs: opts.jobs,
        target: None,
        features: &[],
        no_default_features: false,
        all_features: false,
        spec: &[],
        filter: ops::CompileFilter::Everything,
        release: false,
        message_format: ops::MessageFormat::Human,
        mode: ops::CompileMode::Build,
        target_rustdoc_args: None,
        target_rustc_args: None,
    })?;

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
