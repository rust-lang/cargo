use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::{env, fs};

use failure::{bail, format_err};
use tempfile::Builder as TempFileBuilder;

use crate::core::compiler::Freshness;
use crate::core::compiler::{DefaultExecutor, Executor};
use crate::core::resolver::ResolveOpts;
use crate::core::{Edition, Package, PackageId, PackageIdSpec, Source, SourceId, Workspace};
use crate::ops;
use crate::ops::common_for_install_and_uninstall::*;
use crate::sources::{GitSource, SourceConfigMap};
use crate::util::errors::{CargoResult, CargoResultExt};
use crate::util::{paths, Config, Filesystem};

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
    root: Option<&str>,
    krates: Vec<&str>,
    source_id: SourceId,
    from_cwd: bool,
    vers: Option<&str>,
    opts: &ops::CompileOptions<'_>,
    force: bool,
    no_track: bool,
) -> CargoResult<()> {
    let root = resolve_root(root, opts.config)?;
    let map = SourceConfigMap::new(opts.config)?;

    let (installed_anything, scheduled_error) = if krates.len() <= 1 {
        install_one(
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
        let mut first = true;
        for krate in krates {
            let root = root.clone();
            let map = map.clone();
            match install_one(
                &root,
                &map,
                Some(krate),
                source_id,
                from_cwd,
                vers,
                opts,
                force,
                no_track,
                first,
            ) {
                Ok(()) => succeeded.push(krate),
                Err(e) => {
                    crate::handle_error(&e, &mut opts.config.shell());
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
            summary.push(format!(
                "Failed to install {} (see error(s) above).",
                failed.join(", ")
            ));
        }
        if !succeeded.is_empty() || !failed.is_empty() {
            opts.config.shell().status("Summary", summary.join(" "))?;
        }

        (!succeeded.is_empty(), !failed.is_empty())
    };

    if installed_anything {
        // Print a warning that if this directory isn't in PATH that they won't be
        // able to run these commands.
        let dst = root.join("bin").into_path_unlocked();
        let path = env::var_os("PATH").unwrap_or_default();
        for path in env::split_paths(&path) {
            if path == dst {
                return Ok(());
            }
        }

        opts.config.shell().warn(&format!(
            "be sure to add `{}` to your PATH to be \
             able to run the installed binaries",
            dst.display()
        ))?;
    }

    if scheduled_error {
        bail!("some crates failed to install");
    }

    Ok(())
}

fn install_one(
    root: &Filesystem,
    map: &SourceConfigMap<'_>,
    krate: Option<&str>,
    source_id: SourceId,
    from_cwd: bool,
    vers: Option<&str>,
    opts: &ops::CompileOptions<'_>,
    force: bool,
    no_track: bool,
    is_first_install: bool,
) -> CargoResult<()> {
    let config = opts.config;

    let pkg = if source_id.is_git() {
        select_pkg(
            GitSource::new(source_id, config)?,
            krate,
            vers,
            config,
            true,
            &mut |git| git.read_packages(),
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
            } else {
                bail!(
                    "`{}` does not contain a Cargo.toml file. \
                     --path must point to a directory containing a Cargo.toml file.",
                    src.path().display()
                )
            }
        }
        src.update()?;
        select_pkg(src, krate, vers, config, false, &mut |path| {
            path.read_packages()
        })?
    } else {
        select_pkg(
            map.load(source_id, &HashSet::new())?,
            krate,
            vers,
            config,
            is_first_install,
            &mut |_| {
                bail!(
                    "must specify a crate to install from \
                     crates.io, or use --path or --git to \
                     specify alternate source"
                )
            },
        )?
    };

    let mut td_opt = None;
    let mut needs_cleanup = false;
    let overidden_target_dir = if source_id.is_path() {
        None
    } else if let Some(dir) = config.target_dir()? {
        Some(dir)
    } else if let Ok(td) = TempFileBuilder::new().prefix("cargo-install").tempdir() {
        let p = td.path().to_owned();
        td_opt = Some(td);
        Some(Filesystem::new(p))
    } else {
        needs_cleanup = true;
        Some(Filesystem::new(config.cwd().join("target-install")))
    };

    let mut ws = match overidden_target_dir {
        Some(dir) => Workspace::ephemeral(pkg, config, Some(dir), false)?,
        None => {
            let mut ws = Workspace::new(pkg.manifest_path(), config)?;
            ws.set_require_optional_deps(false);
            ws
        }
    };
    ws.set_ignore_lock(config.lock_update_allowed());
    let pkg = ws.current()?;

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
        bail!("specified package `{}` has no binaries", pkg);
    }

    // Preflight checks to check up front whether we'll overwrite something.
    // We have to check this again afterwards, but may as well avoid building
    // anything if we're gonna throw it away anyway.
    let dst = root.join("bin").into_path_unlocked();
    let rustc = config.load_global_rustc(Some(&ws))?;
    let target = opts
        .build_config
        .requested_target
        .as_ref()
        .unwrap_or(&rustc.host)
        .clone();

    // Helper for --no-track flag to make sure it doesn't overwrite anything.
    let no_track_duplicates = || -> CargoResult<BTreeMap<String, Option<PackageId>>> {
        let duplicates: BTreeMap<String, Option<PackageId>> = exe_names(pkg, &opts.filter)
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
    } else {
        let tracker = InstallTracker::load(config, root)?;
        let (freshness, _duplicates) =
            tracker.check_upgrade(&dst, pkg, force, opts, &target, &rustc.verbose_version)?;
        if freshness == Freshness::Fresh {
            let msg = format!(
                "package `{}` is already installed, use --force to override",
                pkg
            );
            config.shell().status("Ignored", &msg)?;
            return Ok(());
        }
        // Unlock while building.
        drop(tracker);
    }

    config.shell().status("Installing", pkg)?;

    check_yanked_install(&ws)?;

    let exec: Arc<dyn Executor> = Arc::new(DefaultExecutor);
    let compile = ops::compile_ws(&ws, opts, &exec).chain_err(|| {
        if let Some(td) = td_opt.take() {
            // preserve the temporary directory, so the user can inspect it
            td.into_path();
        }

        format_err!(
            "failed to compile `{}`, intermediate artifacts can be \
             found at `{}`",
            pkg,
            ws.target_dir().display()
        )
    })?;
    let mut binaries: Vec<(&str, &Path)> = compile
        .binaries
        .iter()
        .map(|bin| {
            let name = bin.file_name().unwrap();
            if let Some(s) = name.to_str() {
                Ok((s, bin.as_ref()))
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
            tracker.check_upgrade(&dst, pkg, force, opts, &target, &rustc.verbose_version)?;
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
        fs::copy(src, &dst).chain_err(|| {
            format_err!("failed to copy `{}` to `{}`", src.display(), dst.display())
        })?;
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
        fs::rename(&src, &dst).chain_err(|| {
            format_err!("failed to move `{}` to `{}`", src.display(), dst.display())
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
                fs::rename(&src, &dst).chain_err(|| {
                    format_err!("failed to move `{}` to `{}`", src.display(), dst.display())
                })?;
                successful_bins.insert(bin.to_string());
            }
            Ok(())
        };
        try_install()
    };

    if let Some(mut tracker) = tracker {
        tracker.mark_installed(
            pkg,
            &successful_bins,
            vers.map(|s| s.to_string()),
            opts,
            target,
            rustc.verbose_version,
        );

        if let Err(e) = remove_orphaned_bins(&ws, &mut tracker, &duplicates, pkg, &dst) {
            // Don't hard error on remove.
            config
                .shell()
                .warn(format!("failed to remove orphan: {:?}", e))?;
        }

        match tracker.save() {
            Err(err) => replace_result.chain_err(|| err)?,
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
        Ok(())
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
        Ok(())
    }
}

fn check_yanked_install(ws: &Workspace<'_>) -> CargoResult<()> {
    if ws.ignore_lock() || !ws.root().join("Cargo.lock").exists() {
        return Ok(());
    }
    let specs = vec![PackageIdSpec::from_package_id(ws.current()?.package_id())];
    // It would be best if `source` could be passed in here to avoid a
    // duplicate "Updating", but since `source` is taken by value, then it
    // wouldn't be available for `compile_ws`.
    let (pkg_set, resolve) = ops::resolve_ws_with_opts(ws, ResolveOpts::everything(), &specs)?;
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
        println!("{}:", k);
        for bin in v {
            println!("    {}", bin);
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
    for other_pkg in duplicates.values() {
        // Only for packages with the same name.
        if let Some(other_pkg) = other_pkg {
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
                    .chain_err(|| format!("failed to remove {:?}", full_path))?;
            }
        }
    }
    Ok(())
}
