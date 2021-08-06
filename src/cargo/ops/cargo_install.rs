use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::{env, fs};

use crate::core::compiler::{CompileKind, DefaultExecutor, Executor, Freshness, UnitOutput};
use crate::core::{Dependency, Edition, Package, PackageId, Source, SourceId, Workspace};
use crate::ops::common_for_install_and_uninstall::*;
use crate::sources::{GitSource, PathSource, SourceConfigMap};
use crate::util::errors::CargoResult;
use crate::util::{Config, Filesystem, Rustc, ToSemver, VersionReqExt};
use crate::{drop_println, ops};

use anyhow::{bail, format_err, Context as _};
use cargo_util::paths;
use semver::VersionReq;
use tempfile::Builder as TempFileBuilder;

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
            let _ = paths::remove_file(bin);
        }
    }
}

pub fn install(
    config: &Config,
    root: Option<&str>,
    krates: Vec<&str>,
    source_id: SourceId,
    from_cwd: bool,
    vers: Option<&str>,
    opts: &ops::CompileOptions,
    force: bool,
    no_track: bool,
) -> CargoResult<()> {
    let root = resolve_root(root, config)?;
    let map = SourceConfigMap::new(config)?;

    let (installed_anything, scheduled_error) = if krates.len() <= 1 {
        install_one(
            config,
            &root,
            &map,
            krates.into_iter().next(),
            source_id,
            from_cwd,
            vers,
            opts,
            force,
            no_track,
            true,
        )?;
        (true, false)
    } else {
        let mut succeeded = vec![];
        let mut failed = vec![];
        // "Tracks whether or not the source (such as a registry or git repo) has been updated.
        // This is used to avoid updating it multiple times when installing multiple crates.
        let mut did_update = false;
        for krate in krates {
            let root = root.clone();
            let map = map.clone();
            match install_one(
                config,
                &root,
                &map,
                Some(krate),
                source_id,
                from_cwd,
                vers,
                opts,
                force,
                no_track,
                !did_update,
            ) {
                Ok(still_needs_update) => {
                    succeeded.push(krate);
                    did_update |= !still_needs_update;
                }
                Err(e) => {
                    crate::display_error(&e, &mut config.shell());
                    failed.push(krate);
                    // We assume an update was performed if we got an error.
                    did_update = true;
                }
            }
        }

        let mut summary = vec![];
        if !succeeded.is_empty() {
            summary.push(format!("Successfully installed {}!", succeeded.join(", ")));
        }
        if !failed.is_empty() {
            summary.push(format!(
                "Failed to install {} (see error(s) above).",
                failed.join(", ")
            ));
        }
        if !succeeded.is_empty() || !failed.is_empty() {
            config.shell().status("Summary", summary.join(" "))?;
        }

        (!succeeded.is_empty(), !failed.is_empty())
    };

    if installed_anything {
        // Print a warning that if this directory isn't in PATH that they won't be
        // able to run these commands.
        let dst = root.join("bin").into_path_unlocked();
        let path = env::var_os("PATH").unwrap_or_default();
        let dst_in_path = env::split_paths(&path).any(|path| path == dst);

        if !dst_in_path {
            config.shell().warn(&format!(
                "be sure to add `{}` to your PATH to be \
             able to run the installed binaries",
                dst.display()
            ))?;
        }
    }

    if scheduled_error {
        bail!("some crates failed to install");
    }

    Ok(())
}

// Returns whether a subsequent call should attempt to update again.
// The `needs_update_if_source_is_index` parameter indicates whether or not the source index should
// be updated. This is used ensure it is only updated once when installing multiple crates.
// The return value here is used so that the caller knows what to pass to the
// `needs_update_if_source_is_index` parameter when `install_one` is called again.
fn install_one(
    config: &Config,
    root: &Filesystem,
    map: &SourceConfigMap<'_>,
    krate: Option<&str>,
    source_id: SourceId,
    from_cwd: bool,
    vers: Option<&str>,
    opts: &ops::CompileOptions,
    force: bool,
    no_track: bool,
    needs_update_if_source_is_index: bool,
) -> CargoResult<bool> {
    if let Some(name) = krate {
        if name == "." {
            bail!(
                "To install the binaries for the package in current working \
                 directory use `cargo install --path .`. \
                 Use `cargo build` if you want to simply build the package."
            )
        }
    }

    let dst = root.join("bin").into_path_unlocked();

    let pkg = {
        let dep = {
            if let Some(krate) = krate {
                let vers = if let Some(vers_flag) = vers {
                    Some(parse_semver_flag(vers_flag)?.to_string())
                } else if source_id.is_registry() {
                    // Avoid pre-release versions from crate.io
                    // unless explicitly asked for
                    Some(String::from("*"))
                } else {
                    None
                };
                Some(Dependency::parse(krate, vers.as_deref(), source_id)?)
            } else {
                None
            }
        };

        if source_id.is_git() {
            let mut source = GitSource::new(source_id, config)?;
            select_pkg(
                &mut source,
                dep,
                |git: &mut GitSource<'_>| git.read_packages(),
                config,
            )?
        } else if source_id.is_path() {
            let mut src = path_source(source_id, config)?;
            if !src.path().is_dir() {
                bail!(
                    "`{}` is not a directory. \
                 --path must point to a directory containing a Cargo.toml file.",
                    src.path().display()
                )
            }
            if !src.path().join("Cargo.toml").exists() {
                if from_cwd {
                    bail!(
                        "`{}` is not a crate root; specify a crate to \
                     install from crates.io, or use --path or --git to \
                     specify an alternate source",
                        src.path().display()
                    );
                } else if src.path().join("cargo.toml").exists() {
                    bail!(
                        "`{}` does not contain a Cargo.toml file, but found cargo.toml please try to rename it to Cargo.toml. \
                 --path must point to a directory containing a Cargo.toml file.",
                        src.path().display()
                    )
                } else {
                    bail!(
                        "`{}` does not contain a Cargo.toml file. \
                 --path must point to a directory containing a Cargo.toml file.",
                        src.path().display()
                    )
                }
            }
            select_pkg(
                &mut src,
                dep,
                |path: &mut PathSource<'_>| path.read_packages(),
                config,
            )?
        } else if let Some(dep) = dep {
            let mut source = map.load(source_id, &HashSet::new())?;
            if let Ok(Some(pkg)) =
                installed_exact_package(dep.clone(), &mut source, config, opts, root, &dst, force)
            {
                let msg = format!(
                    "package `{}` is already installed, use --force to override",
                    pkg
                );
                config.shell().status("Ignored", &msg)?;
                return Ok(true);
            }
            select_dep_pkg(&mut source, dep, config, needs_update_if_source_is_index)?
        } else {
            bail!(
                "must specify a crate to install from \
                     crates.io, or use --path or --git to \
                     specify alternate source"
            )
        }
    };

    let (mut ws, rustc, target) = make_ws_rustc_target(config, opts, &source_id, pkg.clone())?;
    // If we're installing in --locked mode and there's no `Cargo.lock` published
    // ie. the bin was published before https://github.com/rust-lang/cargo/pull/7026
    if config.locked() && !ws.root().join("Cargo.lock").exists() {
        config.shell().warn(format!(
            "no Cargo.lock file published in {}",
            pkg.to_string()
        ))?;
    }
    let pkg = if source_id.is_git() {
        // Don't use ws.current() in order to keep the package source as a git source so that
        // install tracking uses the correct source.
        pkg
    } else {
        ws.current()?.clone()
    };

    let mut td_opt = None;
    let mut needs_cleanup = false;
    if !source_id.is_path() {
        let target_dir = if let Some(dir) = config.target_dir()? {
            dir
        } else if let Ok(td) = TempFileBuilder::new().prefix("cargo-install").tempdir() {
            let p = td.path().to_owned();
            td_opt = Some(td);
            Filesystem::new(p)
        } else {
            needs_cleanup = true;
            Filesystem::new(config.cwd().join("target-install"))
        };
        ws.set_target_dir(target_dir);
    }

    if from_cwd {
        if pkg.manifest().edition() == Edition::Edition2015 {
            config.shell().warn(
                "Using `cargo install` to install the binaries for the \
                 package in current working directory is deprecated, \
                 use `cargo install --path .` instead. \
                 Use `cargo build` if you want to simply build the package.",
            )?
        } else {
            bail!(
                "Using `cargo install` to install the binaries for the \
                 package in current working directory is no longer supported, \
                 use `cargo install --path .` instead. \
                 Use `cargo build` if you want to simply build the package."
            )
        }
    };

    // For bare `cargo install` (no `--bin` or `--example`), check if there is
    // *something* to install. Explicit `--bin` or `--example` flags will be
    // checked at the start of `compile_ws`.
    if !opts.filter.is_specific() && !pkg.targets().iter().any(|t| t.is_bin()) {
        bail!(
            "there is nothing to install in `{}`, because it has no binaries\n\
             `cargo install` is only for installing programs, and can't be used with libraries.\n\
             To use a library crate, add it as a dependency in a Cargo project instead.",
            pkg
        );
    }

    // Helper for --no-track flag to make sure it doesn't overwrite anything.
    let no_track_duplicates = || -> CargoResult<BTreeMap<String, Option<PackageId>>> {
        let duplicates: BTreeMap<String, Option<PackageId>> = exe_names(&pkg, &opts.filter)
            .into_iter()
            .filter(|name| dst.join(name).exists())
            .map(|name| (name, None))
            .collect();
        if !force && !duplicates.is_empty() {
            let mut msg: Vec<String> = duplicates
                .iter()
                .map(|(name, _)| format!("binary `{}` already exists in destination", name))
                .collect();
            msg.push("Add --force to overwrite".to_string());
            bail!("{}", msg.join("\n"));
        }
        Ok(duplicates)
    };

    // WARNING: no_track does not perform locking, so there is no protection
    // of concurrent installs.
    if no_track {
        // Check for conflicts.
        no_track_duplicates()?;
    } else if is_installed(&pkg, config, opts, &rustc, &target, root, &dst, force)? {
        let msg = format!(
            "package `{}` is already installed, use --force to override",
            pkg
        );
        config.shell().status("Ignored", &msg)?;
        return Ok(false);
    }

    config.shell().status("Installing", &pkg)?;

    check_yanked_install(&ws)?;

    let exec: Arc<dyn Executor> = Arc::new(DefaultExecutor);
    let compile = ops::compile_ws(&ws, opts, &exec).with_context(|| {
        if let Some(td) = td_opt.take() {
            // preserve the temporary directory, so the user can inspect it
            td.into_path();
        }

        format!(
            "failed to compile `{}`, intermediate artifacts can be \
             found at `{}`",
            pkg,
            ws.target_dir().display()
        )
    })?;
    let mut binaries: Vec<(&str, &Path)> = compile
        .binaries
        .iter()
        .map(|UnitOutput { path, .. }| {
            let name = path.file_name().unwrap();
            if let Some(s) = name.to_str() {
                Ok((s, path.as_ref()))
            } else {
                bail!("Binary `{:?}` name can't be serialized into string", name)
            }
        })
        .collect::<CargoResult<_>>()?;
    if binaries.is_empty() {
        bail!("no binaries are available for install using the selected features");
    }
    // This is primarily to make testing easier.
    binaries.sort_unstable();

    let (tracker, duplicates) = if no_track {
        (None, no_track_duplicates()?)
    } else {
        let tracker = InstallTracker::load(config, root)?;
        let (_freshness, duplicates) =
            tracker.check_upgrade(&dst, &pkg, force, opts, &target, &rustc.verbose_version)?;
        (Some(tracker), duplicates)
    };

    paths::create_dir_all(&dst)?;

    // Copy all binaries to a temporary directory under `dst` first, catching
    // some failure modes (e.g., out of space) before touching the existing
    // binaries. This directory will get cleaned up via RAII.
    let staging_dir = TempFileBuilder::new()
        .prefix("cargo-install")
        .tempdir_in(&dst)?;
    for &(bin, src) in binaries.iter() {
        let dst = staging_dir.path().join(bin);
        // Try to move if `target_dir` is transient.
        if !source_id.is_path() && fs::rename(src, &dst).is_ok() {
            continue;
        }
        paths::copy(src, &dst)?;
    }

    let (to_replace, to_install): (Vec<&str>, Vec<&str>) = binaries
        .iter()
        .map(|&(bin, _)| bin)
        .partition(|&bin| duplicates.contains_key(bin));

    let mut installed = Transaction { bins: Vec::new() };
    let mut successful_bins = BTreeSet::new();

    // Move the temporary copies into `dst` starting with new binaries.
    for bin in to_install.iter() {
        let src = staging_dir.path().join(bin);
        let dst = dst.join(bin);
        config.shell().status("Installing", dst.display())?;
        fs::rename(&src, &dst).with_context(|| {
            format!("failed to move `{}` to `{}`", src.display(), dst.display())
        })?;
        installed.bins.push(dst);
        successful_bins.insert(bin.to_string());
    }

    // Repeat for binaries which replace existing ones but don't pop the error
    // up until after updating metadata.
    let replace_result = {
        let mut try_install = || -> CargoResult<()> {
            for &bin in to_replace.iter() {
                let src = staging_dir.path().join(bin);
                let dst = dst.join(bin);
                config.shell().status("Replacing", dst.display())?;
                fs::rename(&src, &dst).with_context(|| {
                    format!("failed to move `{}` to `{}`", src.display(), dst.display())
                })?;
                successful_bins.insert(bin.to_string());
            }
            Ok(())
        };
        try_install()
    };

    if let Some(mut tracker) = tracker {
        tracker.mark_installed(
            &pkg,
            &successful_bins,
            vers.map(|s| s.to_string()),
            opts,
            &target,
            &rustc.verbose_version,
        );

        if let Err(e) = remove_orphaned_bins(&ws, &mut tracker, &duplicates, &pkg, &dst) {
            // Don't hard error on remove.
            config
                .shell()
                .warn(format!("failed to remove orphan: {:?}", e))?;
        }

        match tracker.save() {
            Err(err) => replace_result.with_context(|| err)?,
            Ok(_) => replace_result?,
        }
    }

    // Reaching here means all actions have succeeded. Clean up.
    installed.success();
    if needs_cleanup {
        // Don't bother grabbing a lock as we're going to blow it all away
        // anyway.
        let target_dir = ws.target_dir().into_path_unlocked();
        paths::remove_dir_all(&target_dir)?;
    }

    // Helper for creating status messages.
    fn executables<T: AsRef<str>>(mut names: impl Iterator<Item = T> + Clone) -> String {
        if names.clone().count() == 1 {
            format!("(executable `{}`)", names.next().unwrap().as_ref())
        } else {
            format!(
                "(executables {})",
                names
                    .map(|b| format!("`{}`", b.as_ref()))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    }

    if duplicates.is_empty() {
        config.shell().status(
            "Installed",
            format!("package `{}` {}", pkg, executables(successful_bins.iter())),
        )?;
        Ok(false)
    } else {
        if !to_install.is_empty() {
            config.shell().status(
                "Installed",
                format!("package `{}` {}", pkg, executables(to_install.iter())),
            )?;
        }
        // Invert the duplicate map.
        let mut pkg_map = BTreeMap::new();
        for (bin_name, opt_pkg_id) in &duplicates {
            let key = opt_pkg_id.map_or_else(|| "unknown".to_string(), |pkg_id| pkg_id.to_string());
            pkg_map.entry(key).or_insert_with(Vec::new).push(bin_name);
        }
        for (pkg_descr, bin_names) in &pkg_map {
            config.shell().status(
                "Replaced",
                format!(
                    "package `{}` with `{}` {}",
                    pkg_descr,
                    pkg,
                    executables(bin_names.iter())
                ),
            )?;
        }
        Ok(false)
    }
}

fn is_installed(
    pkg: &Package,
    config: &Config,
    opts: &ops::CompileOptions,
    rustc: &Rustc,
    target: &str,
    root: &Filesystem,
    dst: &Path,
    force: bool,
) -> CargoResult<bool> {
    let tracker = InstallTracker::load(config, root)?;
    let (freshness, _duplicates) =
        tracker.check_upgrade(dst, pkg, force, opts, target, &rustc.verbose_version)?;
    Ok(freshness == Freshness::Fresh)
}

/// Checks if vers can only be satisfied by exactly one version of a package in a registry, and it's
/// already installed. If this is the case, we can skip interacting with a registry to check if
/// newer versions may be installable, as no newer version can exist.
fn installed_exact_package<T>(
    dep: Dependency,
    source: &mut T,
    config: &Config,
    opts: &ops::CompileOptions,
    root: &Filesystem,
    dst: &Path,
    force: bool,
) -> CargoResult<Option<Package>>
where
    T: Source,
{
    if !dep.is_locked() {
        // If the version isn't exact, we may need to update the registry and look for a newer
        // version - we can't know if the package is installed without doing so.
        return Ok(None);
    }
    // Try getting the package from the registry  without updating it, to avoid a potentially
    // expensive network call in the case that the package is already installed.
    // If this fails, the caller will possibly do an index update and try again, this is just a
    // best-effort check to see if we can avoid hitting the network.
    if let Ok(pkg) = select_dep_pkg(source, dep, config, false) {
        let (_ws, rustc, target) =
            make_ws_rustc_target(config, opts, &source.source_id(), pkg.clone())?;
        if let Ok(true) = is_installed(&pkg, config, opts, &rustc, &target, root, dst, force) {
            return Ok(Some(pkg));
        }
    }
    Ok(None)
}

fn make_ws_rustc_target<'cfg>(
    config: &'cfg Config,
    opts: &ops::CompileOptions,
    source_id: &SourceId,
    pkg: Package,
) -> CargoResult<(Workspace<'cfg>, Rustc, String)> {
    let mut ws = if source_id.is_git() || source_id.is_path() {
        Workspace::new(pkg.manifest_path(), config)?
    } else {
        Workspace::ephemeral(pkg, config, None, false)?
    };
    ws.set_ignore_lock(config.lock_update_allowed());
    ws.set_require_optional_deps(false);

    let rustc = config.load_global_rustc(Some(&ws))?;
    let target = match &opts.build_config.single_requested_kind()? {
        CompileKind::Host => rustc.host.as_str().to_owned(),
        CompileKind::Target(target) => target.short_name().to_owned(),
    };

    Ok((ws, rustc, target))
}

/// Parses x.y.z as if it were =x.y.z, and gives CLI-specific error messages in the case of invalid
/// values.
fn parse_semver_flag(v: &str) -> CargoResult<VersionReq> {
    // If the version begins with character <, >, =, ^, ~ parse it as a
    // version range, otherwise parse it as a specific version
    let first = v
        .chars()
        .next()
        .ok_or_else(|| format_err!("no version provided for the `--vers` flag"))?;

    let is_req = "<>=^~".contains(first) || v.contains('*');
    if is_req {
        match v.parse::<VersionReq>() {
            Ok(v) => Ok(v),
            Err(_) => bail!(
                "the `--vers` provided, `{}`, is \
                     not a valid semver version requirement\n\n\
                     Please have a look at \
                     https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html \
                     for the correct format",
                v
            ),
        }
    } else {
        match v.to_semver() {
            Ok(v) => Ok(VersionReq::exact(&v)),
            Err(e) => {
                let mut msg = format!(
                    "the `--vers` provided, `{}`, is \
                         not a valid semver version: {}\n",
                    v, e
                );

                // If it is not a valid version but it is a valid version
                // requirement, add a note to the warning
                if v.parse::<VersionReq>().is_ok() {
                    msg.push_str(&format!(
                        "\nif you want to specify semver range, \
                             add an explicit qualifier, like ^{}",
                        v
                    ));
                }
                bail!(msg);
            }
        }
    }
}

fn check_yanked_install(ws: &Workspace<'_>) -> CargoResult<()> {
    if ws.ignore_lock() || !ws.root().join("Cargo.lock").exists() {
        return Ok(());
    }
    // It would be best if `source` could be passed in here to avoid a
    // duplicate "Updating", but since `source` is taken by value, then it
    // wouldn't be available for `compile_ws`.
    let (pkg_set, resolve) = ops::resolve_ws(ws)?;
    let mut sources = pkg_set.sources_mut();

    // Checking the yanked status involves taking a look at the registry and
    // maybe updating files, so be sure to lock it here.
    let _lock = ws.config().acquire_package_cache_lock()?;

    for pkg_id in resolve.iter() {
        if let Some(source) = sources.get_mut(pkg_id.source_id()) {
            if source.is_yanked(pkg_id)? {
                ws.config().shell().warn(format!(
                    "package `{}` in Cargo.lock is yanked in registry `{}`, \
                     consider running without --locked",
                    pkg_id,
                    pkg_id.source_id().display_registry_name()
                ))?;
            }
        }
    }

    Ok(())
}

/// Display a list of installed binaries.
pub fn install_list(dst: Option<&str>, config: &Config) -> CargoResult<()> {
    let root = resolve_root(dst, config)?;
    let tracker = InstallTracker::load(config, &root)?;
    for (k, v) in tracker.all_installed_bins() {
        drop_println!(config, "{}:", k);
        for bin in v {
            drop_println!(config, "    {}", bin);
        }
    }
    Ok(())
}

/// Removes executables that are no longer part of a package that was
/// previously installed.
fn remove_orphaned_bins(
    ws: &Workspace<'_>,
    tracker: &mut InstallTracker,
    duplicates: &BTreeMap<String, Option<PackageId>>,
    pkg: &Package,
    dst: &Path,
) -> CargoResult<()> {
    let filter = ops::CompileFilter::new_all_targets();
    let all_self_names = exe_names(pkg, &filter);
    let mut to_remove: HashMap<PackageId, BTreeSet<String>> = HashMap::new();
    // For each package that we stomped on.
    for other_pkg in duplicates.values().flatten() {
        // Only for packages with the same name.
        if other_pkg.name() == pkg.name() {
            // Check what the old package had installed.
            if let Some(installed) = tracker.installed_bins(*other_pkg) {
                // If the old install has any names that no longer exist,
                // add them to the list to remove.
                for installed_name in installed {
                    if !all_self_names.contains(installed_name.as_str()) {
                        to_remove
                            .entry(*other_pkg)
                            .or_default()
                            .insert(installed_name.clone());
                    }
                }
            }
        }
    }

    for (old_pkg, bins) in to_remove {
        tracker.remove(old_pkg, &bins);
        for bin in bins {
            let full_path = dst.join(bin);
            if full_path.exists() {
                ws.config().shell().status(
                    "Removing",
                    format!(
                        "executable `{}` from previous version {}",
                        full_path.display(),
                        old_pkg
                    ),
                )?;
                paths::remove_file(&full_path)
                    .with_context(|| format!("failed to remove {:?}", full_path))?;
            }
        }
    }
    Ok(())
}
