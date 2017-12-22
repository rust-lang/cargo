use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};
use std::{env, fs};
use std::io::prelude::*;
use std::io::SeekFrom;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use semver::{Version, VersionReq};
use tempdir::TempDir;
use toml;

use core::{SourceId, Source, Package, Dependency, PackageIdSpec};
use core::{PackageId, Workspace};
use ops::{self, CompileFilter, DefaultExecutor};
use sources::{GitSource, PathSource, SourceConfigMap};
use util::{Config, internal};
use util::{Filesystem, FileLock};
use util::errors::{CargoResult, CargoResultExt};

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
enum CrateListing {
    V1(CrateListingV1),
    Empty(Empty),
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Empty {}

#[derive(Deserialize, Serialize)]
struct CrateListingV1 {
    v1: BTreeMap<PackageId, BTreeSet<String>>,
}

struct Transaction {
    bins: Vec<PathBuf>,
}

impl Transaction {
    fn success(mut self) {
        self.bins.clear();
    }
}

impl Drop for Transaction {
    fn drop(&mut self) {
        for bin in self.bins.iter() {
            let _ = fs::remove_file(bin);
        }
    }
}

pub fn install(root: Option<&str>,
               krates: Vec<&str>,
               source_id: &SourceId,
               vers: Option<&str>,
               opts: &ops::CompileOptions,
               force: bool) -> CargoResult<()> {
    let root = resolve_root(root, opts.config)?;
    let map = SourceConfigMap::new(opts.config)?;

    let (installed_anything, scheduled_error) = if krates.len() <= 1 {
        install_one(&root, &map, krates.into_iter().next(), source_id, vers, opts,
                    force, true)?;
        (true, false)
    } else {
        let mut succeeded = vec![];
        let mut failed = vec![];
        let mut first = true;
        for krate in krates {
            let root = root.clone();
            let map = map.clone();
            match install_one(&root, &map, Some(krate), source_id, vers,
                              opts, force, first) {
                Ok(()) => succeeded.push(krate),
                Err(e) => {
                    ::handle_error(e, &mut opts.config.shell());
                    failed.push(krate)
                }
            }
            first = false;
        }

        let mut summary = vec![];
        if !succeeded.is_empty() {
            summary.push(format!("Successfully installed {}!", succeeded.join(", ")));
        }
        if !failed.is_empty() {
            summary.push(format!("Failed to install {} (see error(s) above).", failed.join(", ")));
        }
        if !succeeded.is_empty() || !failed.is_empty() {
            opts.config.shell().status("Summary", summary.join(" "))?;
        }

        (!succeeded.is_empty(), !failed.is_empty())
    };

    if installed_anything {
        // Print a warning that if this directory isn't in PATH that they won't be
        // able to run these commands.
        let dst = metadata(opts.config, &root)?.parent().join("bin");
        let path = env::var_os("PATH").unwrap_or_default();
        for path in env::split_paths(&path) {
            if path == dst {
                return Ok(())
            }
        }

        opts.config.shell().warn(&format!("be sure to add `{}` to your PATH to be \
                                           able to run the installed binaries",
                                           dst.display()))?;
    }

    if scheduled_error {
        bail!("some crates failed to install");
    }

    Ok(())
}

fn install_one(root: &Filesystem,
               map: &SourceConfigMap,
               krate: Option<&str>,
               source_id: &SourceId,
               vers: Option<&str>,
               opts: &ops::CompileOptions,
               force: bool,
               is_first_install: bool) -> CargoResult<()> {

    let config = opts.config;

    let (pkg, source) = if source_id.is_git() {
        select_pkg(GitSource::new(source_id, config)?,
                   krate, vers, config, is_first_install,
                   &mut |git| git.read_packages())?
    } else if source_id.is_path() {
        let path = source_id.url().to_file_path().map_err(|()| {
            format_err!("path sources must have a valid path")
        })?;
        let mut src = PathSource::new(&path, source_id, config);
        src.update().chain_err(|| {
            format_err!("`{}` is not a crate root; specify a crate to \
                         install from crates.io, or use --path or --git to \
                         specify an alternate source", path.display())
        })?;
        select_pkg(PathSource::new(&path, source_id, config),
                   krate, vers, config, is_first_install,
                   &mut |path| path.read_packages())?
    } else {
        select_pkg(map.load(source_id)?,
                   krate, vers, config, is_first_install,
                   &mut |_| {
                        bail!("must specify a crate to install from \
                               crates.io, or use --path or --git to \
                               specify alternate source")
                   })?
    };

    let mut td_opt = None;
    let mut needs_cleanup = false;
    let overidden_target_dir = if source_id.is_path() {
        None
    } else if let Some(dir) = config.target_dir()? {
        Some(dir)
    } else if let Ok(td) = TempDir::new("cargo-install") {
        let p = td.path().to_owned();
        td_opt = Some(td);
        Some(Filesystem::new(p))
    } else {
        needs_cleanup = true;
        Some(Filesystem::new(config.cwd().join("target-install")))
    };

    let ws = match overidden_target_dir {
        Some(dir) => Workspace::ephemeral(pkg, config, Some(dir), false)?,
        None => Workspace::new(pkg.manifest_path(), config)?,
    };
    let pkg = ws.current()?;

    config.shell().status("Installing", pkg)?;

    // Preflight checks to check up front whether we'll overwrite something.
    // We have to check this again afterwards, but may as well avoid building
    // anything if we're gonna throw it away anyway.
    {
        let metadata = metadata(config, root)?;
        let list = read_crate_list(&metadata)?;
        let dst = metadata.parent().join("bin");
        check_overwrites(&dst, pkg, &opts.filter, &list, force)?;
    }

    let compile = ops::compile_ws(&ws,
                                  Some(source),
                                  opts,
                                  Arc::new(DefaultExecutor)).chain_err(|| {
        if let Some(td) = td_opt.take() {
            // preserve the temporary directory, so the user can inspect it
            td.into_path();
        }

        format_err!("failed to compile `{}`, intermediate artifacts can be \
                     found at `{}`", pkg, ws.target_dir().display())
    })?;
    let binaries: Vec<(&str, &Path)> = compile.binaries.iter().map(|bin| {
        let name = bin.file_name().unwrap();
        if let Some(s) = name.to_str() {
            Ok((s, bin.as_ref()))
        } else {
            bail!("Binary `{:?}` name can't be serialized into string", name)
        }
    }).collect::<CargoResult<_>>()?;
    if binaries.is_empty() {
        bail!("no binaries are available for install using the selected \
              features");
    }

    let metadata = metadata(config, root)?;
    let mut list = read_crate_list(&metadata)?;
    let dst = metadata.parent().join("bin");
    let duplicates = check_overwrites(&dst, pkg, &opts.filter,
                                           &list, force)?;

    fs::create_dir_all(&dst)?;

    // Copy all binaries to a temporary directory under `dst` first, catching
    // some failure modes (e.g. out of space) before touching the existing
    // binaries. This directory will get cleaned up via RAII.
    let staging_dir = TempDir::new_in(&dst, "cargo-install")?;
    for &(bin, src) in binaries.iter() {
        let dst = staging_dir.path().join(bin);
        // Try to move if `target_dir` is transient.
        if !source_id.is_path() && fs::rename(src, &dst).is_ok() {
            continue
        }
        fs::copy(src, &dst).chain_err(|| {
            format_err!("failed to copy `{}` to `{}`", src.display(),
                        dst.display())
        })?;
    }

    let (to_replace, to_install): (Vec<&str>, Vec<&str>) =
        binaries.iter().map(|&(bin, _)| bin)
                       .partition(|&bin| duplicates.contains_key(bin));

    let mut installed = Transaction { bins: Vec::new() };

    // Move the temporary copies into `dst` starting with new binaries.
    for bin in to_install.iter() {
        let src = staging_dir.path().join(bin);
        let dst = dst.join(bin);
        config.shell().status("Installing", dst.display())?;
        fs::rename(&src, &dst).chain_err(|| {
            format_err!("failed to move `{}` to `{}`", src.display(),
                        dst.display())
        })?;
        installed.bins.push(dst);
    }

    // Repeat for binaries which replace existing ones but don't pop the error
    // up until after updating metadata.
    let mut replaced_names = Vec::new();
    let result = {
        let mut try_install = || -> CargoResult<()> {
            for &bin in to_replace.iter() {
                let src = staging_dir.path().join(bin);
                let dst = dst.join(bin);
                config.shell().status("Replacing", dst.display())?;
                fs::rename(&src, &dst).chain_err(|| {
                    format_err!("failed to move `{}` to `{}`", src.display(),
                                dst.display())
                })?;
                replaced_names.push(bin);
            }
            Ok(())
        };
        try_install()
    };

    // Update records of replaced binaries.
    for &bin in replaced_names.iter() {
        if let Some(&Some(ref p)) = duplicates.get(bin) {
            if let Some(set) = list.v1.get_mut(p) {
                set.remove(bin);
            }
        }
        list.v1.entry(pkg.package_id().clone())
               .or_insert_with(|| BTreeSet::new())
               .insert(bin.to_string());
    }

    // Remove empty metadata lines.
    let pkgs = list.v1.iter()
                      .filter_map(|(p, set)| if set.is_empty() { Some(p.clone()) } else { None })
                      .collect::<Vec<_>>();
    for p in pkgs.iter() {
        list.v1.remove(p);
    }

    // If installation was successful record newly installed binaries.
    if result.is_ok() {
        list.v1.entry(pkg.package_id().clone())
               .or_insert_with(|| BTreeSet::new())
               .extend(to_install.iter().map(|s| s.to_string()));
    }

    let write_result = write_crate_list(&metadata, list);
    match write_result {
        // Replacement error (if any) isn't actually caused by write error
        // but this seems to be the only way to show both.
        Err(err) => result.chain_err(|| err)?,
        Ok(_) => result?,
    }

    // Reaching here means all actions have succeeded. Clean up.
    installed.success();
    if needs_cleanup {
        // Don't bother grabbing a lock as we're going to blow it all away
        // anyway.
        let target_dir = ws.target_dir().into_path_unlocked();
        fs::remove_dir_all(&target_dir)?;
    }

    Ok(())
}

fn select_pkg<'a, T>(mut source: T,
                     name: Option<&str>,
                     vers: Option<&str>,
                     config: &Config,
                     needs_update: bool,
                     list_all: &mut FnMut(&mut T) -> CargoResult<Vec<Package>>)
                     -> CargoResult<(Package, Box<Source + 'a>)>
    where T: Source + 'a
{
    if needs_update {
        source.update()?;
    }

    match name {
        Some(name) => {
            let vers = match vers {
                Some(v) => {

                    // If the version begins with character <, >, =, ^, ~ parse it as a
                    // version range, otherwise parse it as a specific version
                    let first = v.chars()
                        .nth(0)
                        .ok_or(format_err!("no version provided for the `--vers` flag"))?;

                    match first {
                        '<' | '>' | '=' | '^' | '~' => match v.parse::<VersionReq>() {
                            Ok(v) => Some(v.to_string()),
                            Err(_) => {
                                bail!("the `--vers` provided, `{}`, is \
                                       not a valid semver version requirement\n\n
                                       Please have a look at \
                                       http://doc.crates.io/specifying-dependencies.html \
                                       for the correct format", v)
                            }
                        },
                        _ => match v.parse::<Version>() {
                            Ok(v) => Some(format!("={}", v)),
                            Err(_) => {
                                let mut msg = format!("\
                                    the `--vers` provided, `{}`, is \
                                    not a valid semver version\n\n\
                                    historically Cargo treated this \
                                    as a semver version requirement \
                                    accidentally\nand will continue \
                                    to do so, but this behavior \
                                    will be removed eventually", v
                                );

                                // If it is not a valid version but it is a valid version
                                // requirement, add a note to the warning
                                if v.parse::<VersionReq>().is_ok() {
                                    msg.push_str(&format!("\nif you want to specify semver range, \
                                                  add an explicit qualifier, like ^{}", v));
                                }
                                config.shell().warn(&msg)?;
                                Some(v.to_string())
                            }
                        }
                    }
                }
                None => None,
            };
            let vers = vers.as_ref().map(|s| &**s);
            let dep = Dependency::parse_no_deprecated(name, vers, source.source_id())?;
            let deps = source.query_vec(&dep)?;
            match deps.iter().map(|p| p.package_id()).max() {
                Some(pkgid) => {
                    let pkg = source.download(pkgid)?;
                    Ok((pkg, Box::new(source)))
                }
                None => {
                    let vers_info = vers.map(|v| format!(" with version `{}`", v))
                                        .unwrap_or_default();
                    Err(format_err!("could not find `{}` in {}{}", name,
                                    source.source_id(), vers_info))
                }
            }
        }
        None => {
            let candidates = list_all(&mut source)?;
            let binaries = candidates.iter().filter(|cand| {
                cand.targets().iter().filter(|t| t.is_bin()).count() > 0
            });
            let examples = candidates.iter().filter(|cand| {
                cand.targets().iter().filter(|t| t.is_example()).count() > 0
            });
            let pkg = match one(binaries, |v| multi_err("binaries", v))? {
                Some(p) => p,
                None => {
                    match one(examples, |v| multi_err("examples", v))? {
                        Some(p) => p,
                        None => bail!("no packages found with binaries or \
                                       examples"),
                    }
                }
            };
            return Ok((pkg.clone(), Box::new(source)));

            fn multi_err(kind: &str, mut pkgs: Vec<&Package>) -> String {
                pkgs.sort_by(|a, b| a.name().cmp(b.name()));
                format!("multiple packages with {} found: {}", kind,
                        pkgs.iter().map(|p| p.name()).collect::<Vec<_>>()
                            .join(", "))
            }
        }
    }
}

fn one<I, F>(mut i: I, f: F) -> CargoResult<Option<I::Item>>
    where I: Iterator,
          F: FnOnce(Vec<I::Item>) -> String
{
    match (i.next(), i.next()) {
        (Some(i1), Some(i2)) => {
            let mut v = vec![i1, i2];
            v.extend(i);
            Err(format_err!("{}", f(v)))
        }
        (Some(i), None) => Ok(Some(i)),
        (None, _) => Ok(None)
    }
}

fn check_overwrites(dst: &Path,
                    pkg: &Package,
                    filter: &ops::CompileFilter,
                    prev: &CrateListingV1,
                    force: bool) -> CargoResult<BTreeMap<String, Option<PackageId>>> {
    // If explicit --bin or --example flags were passed then those'll
    // get checked during cargo_compile, we only care about the "build
    // everything" case here
    if !filter.is_specific() && !pkg.targets().iter().any(|t| t.is_bin()) {
        bail!("specified package has no binaries")
    }
    let duplicates = find_duplicates(dst, pkg, filter, prev);
    if force || duplicates.is_empty() {
        return Ok(duplicates)
    }
    // Format the error message.
    let mut msg = String::new();
    for (bin, p) in duplicates.iter() {
        msg.push_str(&format!("binary `{}` already exists in destination", bin));
        if let Some(p) = p.as_ref() {
            msg.push_str(&format!(" as part of `{}`\n", p));
        } else {
            msg.push_str("\n");
        }
    }
    msg.push_str("Add --force to overwrite");
    Err(format_err!("{}", msg))
}

fn find_duplicates(dst: &Path,
                   pkg: &Package,
                   filter: &ops::CompileFilter,
                   prev: &CrateListingV1) -> BTreeMap<String, Option<PackageId>> {
    let check = |name: String| {
        // Need to provide type, works around Rust Issue #93349
        let name = format!("{}{}", name, env::consts::EXE_SUFFIX);
        if fs::metadata(dst.join(&name)).is_err() {
            None
        } else if let Some((p, _)) = prev.v1.iter().find(|&(_, v)| v.contains(&name)) {
            Some((name, Some(p.clone())))
        } else {
            Some((name, None))
        }
    };
    match *filter {
        CompileFilter::Default { .. } => {
            pkg.targets().iter()
                         .filter(|t| t.is_bin())
                         .filter_map(|t| check(t.name().to_string()))
                         .collect()
        }
        CompileFilter::Only { bins, examples, .. } => {
            let all_bins: Vec<String> = bins.try_collect().unwrap_or_else(|| {
                pkg.targets().iter().filter(|t| t.is_bin())
                                    .map(|t| t.name().to_string())
                                    .collect()
            });
            let all_examples: Vec<String> = examples.try_collect().unwrap_or_else(|| {
                pkg.targets().iter().filter(|t| t.is_bin_example())
                                    .map(|t| t.name().to_string())
                                    .collect()
            });

            all_bins.iter().chain(all_examples.iter())
                           .filter_map(|t| check(t.clone()))
                           .collect::<BTreeMap<String, Option<PackageId>>>()
        }
    }
}

fn read_crate_list(file: &FileLock) -> CargoResult<CrateListingV1> {
    let listing = (|| -> CargoResult<_> {
        let mut contents = String::new();
        file.file().read_to_string(&mut contents)?;
        let listing = toml::from_str(&contents).chain_err(|| {
            internal("invalid TOML found for metadata")
        })?;
        match listing {
            CrateListing::V1(v1) => Ok(v1),
            CrateListing::Empty(_) => {
                Ok(CrateListingV1 { v1: BTreeMap::new() })
            }
        }
    })().chain_err(|| {
        format_err!("failed to parse crate metadata at `{}`",
                    file.path().to_string_lossy())
    })?;
    Ok(listing)
}

fn write_crate_list(file: &FileLock, listing: CrateListingV1) -> CargoResult<()> {
    (|| -> CargoResult<_> {
        let mut file = file.file();
        file.seek(SeekFrom::Start(0))?;
        file.set_len(0)?;
        let data = toml::to_string(&CrateListing::V1(listing))?;
        file.write_all(data.as_bytes())?;
        Ok(())
    })().chain_err(|| {
        format_err!("failed to write crate metadata at `{}`",
                    file.path().to_string_lossy())
    })?;
    Ok(())
}

pub fn install_list(dst: Option<&str>, config: &Config) -> CargoResult<()> {
    let dst = resolve_root(dst, config)?;
    let dst = metadata(config, &dst)?;
    let list = read_crate_list(&dst)?;
    for (k, v) in list.v1.iter() {
        println!("{}:", k);
        for bin in v {
            println!("    {}", bin);
        }
    }
    Ok(())
}

pub fn uninstall(root: Option<&str>,
                 specs: Vec<&str>,
                 bins: &[String],
                 config: &Config) -> CargoResult<()> {
    if specs.len() > 1 && bins.len() > 0 {
        bail!("A binary can only be associated with a single installed package, specifying multiple specs with --bin is redundant.");
    }

    let root = resolve_root(root, config)?;
    let scheduled_error = if specs.len() == 1 {
        uninstall_one(root, specs[0], bins, config)?;
        false
    } else {
        let mut succeeded = vec![];
        let mut failed = vec![];
        for spec in specs {
            let root = root.clone();
            match uninstall_one(root, spec, bins, config) {
                Ok(()) => succeeded.push(spec),
                Err(e) => {
                    ::handle_error(e, &mut config.shell());
                    failed.push(spec)
                }
            }
        }

        let mut summary = vec![];
        if !succeeded.is_empty() {
            summary.push(format!("Successfully uninstalled {}!", succeeded.join(", ")));
        }
        if !failed.is_empty() {
            summary.push(format!("Failed to uninstall {} (see error(s) above).", failed.join(", ")));
        }

        if !succeeded.is_empty() || !failed.is_empty() {
            config.shell().status("Summary", summary.join(" "))?;
        }

        !failed.is_empty()
    };

    if scheduled_error {
        bail!("some packages failed to uninstall");
    }

    Ok(())
}

pub fn uninstall_one(root: Filesystem,
                     spec: &str,
                     bins: &[String],
                     config: &Config) -> CargoResult<()> {
    let crate_metadata = metadata(config, &root)?;
    let mut metadata = read_crate_list(&crate_metadata)?;
    let mut to_remove = Vec::new();
    {
        let result = PackageIdSpec::query_str(spec, metadata.v1.keys())?
                                        .clone();
        let mut installed = match metadata.v1.entry(result.clone()) {
            Entry::Occupied(e) => e,
            Entry::Vacant(..) => panic!("entry not found: {}", result),
        };
        let dst = crate_metadata.parent().join("bin");
        for bin in installed.get() {
            let bin = dst.join(bin);
            if fs::metadata(&bin).is_err() {
                bail!("corrupt metadata, `{}` does not exist when it should",
                      bin.display())
            }
        }

        let bins = bins.iter().map(|s| {
            if s.ends_with(env::consts::EXE_SUFFIX) {
                s.to_string()
            } else {
                format!("{}{}", s, env::consts::EXE_SUFFIX)
            }
        }).collect::<Vec<_>>();

        for bin in bins.iter() {
            if !installed.get().contains(bin) {
                bail!("binary `{}` not installed as part of `{}`", bin, result)
            }
        }

        if bins.is_empty() {
            to_remove.extend(installed.get().iter().map(|b| dst.join(b)));
            installed.get_mut().clear();
        } else {
            for bin in bins.iter() {
                to_remove.push(dst.join(bin));
                installed.get_mut().remove(bin);
            }
        }
        if installed.get().is_empty() {
            installed.remove();
        }
    }
    write_crate_list(&crate_metadata, metadata)?;
    for bin in to_remove {
        config.shell().status("Removing", bin.display())?;
        fs::remove_file(bin)?;
    }

    Ok(())
}

fn metadata(config: &Config, root: &Filesystem) -> CargoResult<FileLock> {
    root.open_rw(Path::new(".crates.toml"), config, "crate metadata")
}

fn resolve_root(flag: Option<&str>,
                config: &Config) -> CargoResult<Filesystem> {
    let config_root = config.get_path("install.root")?;
    Ok(flag.map(PathBuf::from).or_else(|| {
        env::var_os("CARGO_INSTALL_ROOT").map(PathBuf::from)
    }).or_else(move || {
        config_root.map(|v| v.val)
    }).map(Filesystem::new).unwrap_or_else(|| {
        config.home().clone()
    }))
}
