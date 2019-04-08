use std::collections::HashMap;
use std::fs::{self, File};
use std::io::prelude::*;
use std::io::SeekFrom;
use std::path::{self, Path, PathBuf};
use std::sync::Arc;

use flate2::read::GzDecoder;
use flate2::{Compression, GzBuilder};
use log::debug;
use serde_json::{self, json};
use tar::{Archive, Builder, EntryType, Header};

use crate::core::compiler::{BuildConfig, CompileMode, DefaultExecutor, Executor};
use crate::core::{Package, Source, SourceId, Workspace};
use crate::ops;
use crate::sources::PathSource;
use crate::util::errors::{CargoResult, CargoResultExt};
use crate::util::paths;
use crate::util::{self, internal, Config, FileLock};

pub struct PackageOpts<'cfg> {
    pub config: &'cfg Config,
    pub list: bool,
    pub check_metadata: bool,
    pub allow_dirty: bool,
    pub verify: bool,
    pub jobs: Option<u32>,
    pub target: Option<String>,
    pub features: Vec<String>,
    pub all_features: bool,
    pub no_default_features: bool,
}

static VCS_INFO_FILE: &'static str = ".cargo_vcs_info.json";

pub fn package(ws: &Workspace<'_>, opts: &PackageOpts<'_>) -> CargoResult<Option<FileLock>> {
    ops::resolve_ws(ws)?;
    let pkg = ws.current()?;
    let config = ws.config();

    let mut src = PathSource::new(pkg.root(), pkg.package_id().source_id(), config);
    src.update()?;

    if opts.check_metadata {
        check_metadata(pkg, config)?;
    }

    verify_dependencies(pkg)?;

    // `list_files` outputs warnings as a side effect, so only do it once.
    let src_files = src.list_files(pkg)?;

    // Make sure a VCS info file is not included in source, regardless of if
    // we produced the file above, and in particular if we did not.
    check_vcs_file_collision(pkg, &src_files)?;

    // Check (git) repository state, getting the current commit hash if not
    // dirty. This will `bail!` if dirty, unless allow_dirty. Produce json
    // info for any sha1 (HEAD revision) returned.
    let vcs_info = if !opts.allow_dirty {
        check_repo_state(pkg, &src_files, config, opts.allow_dirty)?
            .map(|h| json!({"git":{"sha1": h}}))
    } else {
        None
    };

    if opts.list {
        let root = pkg.root();
        let mut list: Vec<_> = src
            .list_files(pkg)?
            .iter()
            .map(|file| file.strip_prefix(root).unwrap().to_path_buf())
            .collect();
        if include_lockfile(pkg) {
            list.push("Cargo.lock".into());
        }
        if vcs_info.is_some() {
            list.push(Path::new(VCS_INFO_FILE).to_path_buf());
        }
        list.sort_unstable();
        for file in list.iter() {
            println!("{}", file.display());
        }
        return Ok(None);
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
    config
        .shell()
        .status("Packaging", pkg.package_id().to_string())?;
    dst.file().set_len(0)?;
    tar(ws, &src_files, vcs_info.as_ref(), dst.file(), &filename)
        .chain_err(|| failure::format_err!("failed to prepare local package for uploading"))?;
    if opts.verify {
        dst.seek(SeekFrom::Start(0))?;
        run_verify(ws, &dst, opts).chain_err(|| "failed to verify package tarball")?
    }
    dst.seek(SeekFrom::Start(0))?;
    {
        let src_path = dst.path();
        let dst_path = dst.parent().join(&filename);
        fs::rename(&src_path, &dst_path)
            .chain_err(|| "failed to move temporary tarball into final location")?;
    }
    Ok(Some(dst))
}

fn include_lockfile(pkg: &Package) -> bool {
    pkg.manifest().publish_lockfile() && pkg.targets().iter().any(|t| t.is_example() || t.is_bin())
}

// Checks that the package has some piece of metadata that a human can
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
    lacking!(
        description,
        license || license_file,
        documentation || homepage || repository
    );

    if !missing.is_empty() {
        let mut things = missing[..missing.len() - 1].join(", ");
        // `things` will be empty if and only if its length is 1 (i.e., the only case
        // to have no `or`).
        if !things.is_empty() {
            things.push_str(" or ");
        }
        things.push_str(missing.last().unwrap());

        config.shell().warn(&format!(
            "manifest has no {things}.\n\
             See https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info.",
            things = things
        ))?
    }
    Ok(())
}

// Checks that the package dependencies are safe to deploy.
fn verify_dependencies(pkg: &Package) -> CargoResult<()> {
    for dep in pkg.dependencies() {
        if dep.source_id().is_path() && !dep.specified_req() {
            failure::bail!(
                "all path dependencies must have a version specified \
                 when packaging.\ndependency `{}` does not specify \
                 a version.",
                dep.name_in_toml()
            )
        }
    }
    Ok(())
}

// Checks if the package source is in a *git* DVCS repository. If *git*, and
// the source is *dirty* (e.g., has uncommited changes) and not `allow_dirty`
// then `bail!` with an informative message. Otherwise return the sha1 hash of
// the current *HEAD* commit, or `None` if *dirty*.
fn check_repo_state(
    p: &Package,
    src_files: &[PathBuf],
    config: &Config,
    allow_dirty: bool,
) -> CargoResult<Option<String>> {
    if let Ok(repo) = git2::Repository::discover(p.root()) {
        if let Some(workdir) = repo.workdir() {
            debug!("found a git repo at {:?}", workdir);
            let path = p.manifest_path();
            let path = path.strip_prefix(workdir).unwrap_or(path);
            if let Ok(status) = repo.status_file(path) {
                if (status & git2::Status::IGNORED).is_empty() {
                    debug!(
                        "found (git) Cargo.toml at {:?} in workdir {:?}",
                        path, workdir
                    );
                    return git(p, src_files, &repo, allow_dirty);
                }
            }
            config.shell().verbose(|shell| {
                shell.warn(format!(
                    "No (git) Cargo.toml found at `{}` in workdir `{}`",
                    path.display(),
                    workdir.display()
                ))
            })?;
        }
    } else {
        config.shell().verbose(|shell| {
            shell.warn(format!("No (git) VCS found for `{}`", p.root().display()))
        })?;
    }

    // No VCS with a checked in `Cargo.toml` found, so we don't know if the
    // directory is dirty or not, thus we have to assume that it's clean.
    return Ok(None);

    fn git(
        p: &Package,
        src_files: &[PathBuf],
        repo: &git2::Repository,
        allow_dirty: bool,
    ) -> CargoResult<Option<String>> {
        let workdir = repo.workdir().unwrap();
        let dirty = src_files
            .iter()
            .filter(|file| {
                let relative = file.strip_prefix(workdir).unwrap();
                if let Ok(status) = repo.status_file(relative) {
                    status != git2::Status::CURRENT
                } else {
                    false
                }
            })
            .map(|path| {
                path.strip_prefix(p.root())
                    .unwrap_or(path)
                    .display()
                    .to_string()
            })
            .collect::<Vec<_>>();
        if dirty.is_empty() {
            let rev_obj = repo.revparse_single("HEAD")?;
            Ok(Some(rev_obj.id().to_string()))
        } else {
            if !allow_dirty {
                failure::bail!(
                    "{} files in the working directory contain changes that were \
                     not yet committed into git:\n\n{}\n\n\
                     to proceed despite this, pass the `--allow-dirty` flag",
                    dirty.len(),
                    dirty.join("\n")
                )
            }
            Ok(None)
        }
    }
}

// Checks for and `bail!` if a source file matches `ROOT/VCS_INFO_FILE`, since
// this is now a Cargo reserved file name, and we don't want to allow forgery.
fn check_vcs_file_collision(pkg: &Package, src_files: &[PathBuf]) -> CargoResult<()> {
    let root = pkg.root();
    let vcs_info_path = Path::new(VCS_INFO_FILE);
    let collision = src_files
        .iter()
        .find(|&p| p.strip_prefix(root).unwrap() == vcs_info_path);
    if collision.is_some() {
        failure::bail!(
            "Invalid inclusion of reserved file name \
             {} in package source",
            VCS_INFO_FILE
        );
    }
    Ok(())
}

fn tar(
    ws: &Workspace<'_>,
    src_files: &[PathBuf],
    vcs_info: Option<&serde_json::Value>,
    dst: &File,
    filename: &str,
) -> CargoResult<()> {
    // Prepare the encoder and its header.
    let filename = Path::new(filename);
    let encoder = GzBuilder::new()
        .filename(util::path2bytes(filename)?)
        .write(dst, Compression::best());

    // Put all package files into a compressed archive.
    let mut ar = Builder::new(encoder);
    let pkg = ws.current()?;
    let config = ws.config();
    let root = pkg.root();
    for file in src_files.iter() {
        let relative = file.strip_prefix(root)?;
        check_filename(relative)?;
        let relative = relative.to_str().ok_or_else(|| {
            failure::format_err!("non-utf8 path in source directory: {}", relative.display())
        })?;
        config
            .shell()
            .verbose(|shell| shell.status("Archiving", &relative))?;
        let path = format!(
            "{}-{}{}{}",
            pkg.name(),
            pkg.version(),
            path::MAIN_SEPARATOR,
            relative
        );

        // The `tar::Builder` type by default will build GNU archives, but
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
        // look at rust-lang/cargo#2326.
        let mut header = Header::new_ustar();
        header
            .set_path(&path)
            .chain_err(|| format!("failed to add to archive: `{}`", relative))?;
        let mut file = File::open(file)
            .chain_err(|| format!("failed to open for archiving: `{}`", file.display()))?;
        let metadata = file
            .metadata()
            .chain_err(|| format!("could not learn metadata for: `{}`", relative))?;
        header.set_metadata(&metadata);

        if relative == "Cargo.toml" {
            let orig = Path::new(&path).with_file_name("Cargo.toml.orig");
            header.set_path(&orig)?;
            header.set_cksum();
            ar.append(&header, &mut file)
                .chain_err(|| internal(format!("could not archive source file `{}`", relative)))?;

            let mut header = Header::new_ustar();
            let toml = pkg.to_registry_toml(ws.config())?;
            header.set_path(&path)?;
            header.set_entry_type(EntryType::file());
            header.set_mode(0o644);
            header.set_size(toml.len() as u64);
            header.set_cksum();
            ar.append(&header, toml.as_bytes())
                .chain_err(|| internal(format!("could not archive source file `{}`", relative)))?;
        } else {
            header.set_cksum();
            ar.append(&header, &mut file)
                .chain_err(|| internal(format!("could not archive source file `{}`", relative)))?;
        }
    }

    if let Some(json) = vcs_info {
        let filename: PathBuf = Path::new(VCS_INFO_FILE).into();
        debug_assert!(check_filename(&filename).is_ok());
        let fnd = filename.display();
        config
            .shell()
            .verbose(|shell| shell.status("Archiving", &fnd))?;
        let path = format!(
            "{}-{}{}{}",
            pkg.name(),
            pkg.version(),
            path::MAIN_SEPARATOR,
            fnd
        );
        let mut header = Header::new_ustar();
        header
            .set_path(&path)
            .chain_err(|| format!("failed to add to archive: `{}`", fnd))?;
        let json = format!("{}\n", serde_json::to_string_pretty(json)?);
        let mut header = Header::new_ustar();
        header.set_path(&path)?;
        header.set_entry_type(EntryType::file());
        header.set_mode(0o644);
        header.set_size(json.len() as u64);
        header.set_cksum();
        ar.append(&header, json.as_bytes())
            .chain_err(|| internal(format!("could not archive source file `{}`", fnd)))?;
    }

    if include_lockfile(pkg) {
        let toml = paths::read(&ws.root().join("Cargo.lock"))?;
        let path = format!(
            "{}-{}{}Cargo.lock",
            pkg.name(),
            pkg.version(),
            path::MAIN_SEPARATOR
        );
        let mut header = Header::new_ustar();
        header.set_path(&path)?;
        header.set_entry_type(EntryType::file());
        header.set_mode(0o644);
        header.set_size(toml.len() as u64);
        header.set_cksum();
        ar.append(&header, toml.as_bytes())
            .chain_err(|| internal("could not archive source file `Cargo.lock`"))?;
    }

    let encoder = ar.into_inner()?;
    encoder.finish()?;
    Ok(())
}

fn run_verify(ws: &Workspace<'_>, tar: &FileLock, opts: &PackageOpts<'_>) -> CargoResult<()> {
    let config = ws.config();
    let pkg = ws.current()?;

    config.shell().status("Verifying", pkg)?;

    let f = GzDecoder::new(tar.file());
    let dst = tar
        .parent()
        .join(&format!("{}-{}", pkg.name(), pkg.version()));
    if dst.exists() {
        paths::remove_dir_all(&dst)?;
    }
    let mut archive = Archive::new(f);
    // We don't need to set the Modified Time, as it's not relevant to verification
    // and it errors on filesystems that don't support setting a modified timestamp
    archive.set_preserve_mtime(false);
    archive.unpack(dst.parent().unwrap())?;

    // Manufacture an ephemeral workspace to ensure that even if the top-level
    // package has a workspace we can still build our new crate.
    let id = SourceId::for_path(&dst)?;
    let mut src = PathSource::new(&dst, id, ws.config());
    let new_pkg = src.root_package()?;
    let pkg_fingerprint = hash_all(&dst)?;
    let ws = Workspace::ephemeral(new_pkg, config, None, true)?;

    let exec: Arc<dyn Executor> = Arc::new(DefaultExecutor);
    ops::compile_ws(
        &ws,
        None,
        &ops::CompileOptions {
            config,
            build_config: BuildConfig::new(config, opts.jobs, &opts.target, CompileMode::Build)?,
            features: opts.features.clone(),
            no_default_features: opts.no_default_features,
            all_features: opts.all_features,
            spec: ops::Packages::Packages(Vec::new()),
            filter: ops::CompileFilter::Default {
                required_features_filterable: true,
            },
            target_rustdoc_args: None,
            target_rustc_args: None,
            local_rustdoc_args: None,
            export_dir: None,
        },
        &exec,
    )?;

    // Check that `build.rs` didn't modify any files in the `src` directory.
    let ws_fingerprint = hash_all(&dst)?;
    if pkg_fingerprint != ws_fingerprint {
        let changes = report_hash_difference(&pkg_fingerprint, &ws_fingerprint);
        failure::bail!(
            "Source directory was modified by build.rs during cargo publish. \
             Build scripts should not modify anything outside of OUT_DIR.\n\
             {}\n\n\
             To proceed despite this, pass the `--no-verify` flag.",
            changes
        )
    }

    Ok(())
}

fn hash_all(path: &Path) -> CargoResult<HashMap<PathBuf, u64>> {
    fn wrap(path: &Path) -> CargoResult<HashMap<PathBuf, u64>> {
        let mut result = HashMap::new();
        let walker = walkdir::WalkDir::new(path).into_iter();
        for entry in walker.filter_entry(|e| {
            !(e.depth() == 1 && (e.file_name() == "target" || e.file_name() == "Cargo.lock"))
        }) {
            let entry = entry?;
            let file_type = entry.file_type();
            if file_type.is_file() {
                let contents = fs::read(entry.path())?;
                let hash = util::hex::hash_u64(&contents);
                result.insert(entry.path().to_path_buf(), hash);
            } else if file_type.is_symlink() {
                let hash = util::hex::hash_u64(&fs::read_link(entry.path())?);
                result.insert(entry.path().to_path_buf(), hash);
            }
        }
        Ok(result)
    }
    let result = wrap(path).chain_err(|| format!("failed to verify output at {:?}", path))?;
    Ok(result)
}

fn report_hash_difference(
    orig: &HashMap<PathBuf, u64>,
    after: &HashMap<PathBuf, u64>,
) -> String {
    let mut changed = Vec::new();
    let mut removed = Vec::new();
    for (key, value) in orig {
        match after.get(key) {
            Some(after_value) => {
                if value != after_value {
                    changed.push(key.to_string_lossy());
                }
            }
            None => removed.push(key.to_string_lossy()),
        }
    }
    let mut added: Vec<_> = after
        .keys()
        .filter(|key| !orig.contains_key(*key))
        .map(|key| key.to_string_lossy())
        .collect();
    let mut result = Vec::new();
    if !changed.is_empty() {
        changed.sort_unstable();
        result.push(format!("Changed: {}", changed.join("\n\t")));
    }
    if !added.is_empty() {
        added.sort_unstable();
        result.push(format!("Added: {}", added.join("\n\t")));
    }
    if !removed.is_empty() {
        removed.sort_unstable();
        result.push(format!("Removed: {}", removed.join("\n\t")));
    }
    assert!(!result.is_empty(), "unexpected empty change detection");
    result.join("\n")
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
        None => failure::bail!(
            "path does not have a unicode filename which may not unpack \
             on all platforms: {}",
            file.display()
        ),
    };
    let bad_chars = ['/', '\\', '<', '>', ':', '"', '|', '?', '*'];
    if let Some(c) = bad_chars.iter().find(|c| name.contains(**c)) {
        failure::bail!(
            "cannot package a filename with a special character `{}`: {}",
            c,
            file.display()
        )
    }
    Ok(())
}
