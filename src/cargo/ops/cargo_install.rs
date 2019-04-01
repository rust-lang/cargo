use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::{env, fs};

use tempfile::Builder as TempFileBuilder;

use crate::core::compiler::{DefaultExecutor, Executor};
use crate::core::{Edition, Package, Source, SourceId};
use crate::core::{PackageId, Workspace};
use crate::ops::common_for_install_and_uninstall::*;
use crate::ops::{self, CompileFilter};
use crate::sources::{GitSource, SourceConfigMap};
use crate::util::errors::{CargoResult, CargoResultExt};
use crate::util::paths;
use crate::util::Config;
use crate::util::Filesystem;

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
        let dst = metadata(opts.config, &root)?.parent().join("bin");
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
        failure::bail!("some crates failed to install");
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
    is_first_install: bool,
) -> CargoResult<()> {
    let config = opts.config;

    let (pkg, source) = if source_id.is_git() {
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
            failure::bail!(
                "`{}` is not a directory. \
                 --path must point to a directory containing a Cargo.toml file.",
                src.path().display()
            )
        }
        if !src.path().join("Cargo.toml").exists() {
            if from_cwd {
                failure::bail!(
                    "`{}` is not a crate root; specify a crate to \
                     install from crates.io, or use --path or --git to \
                     specify an alternate source",
                    src.path().display()
                );
            } else {
                failure::bail!(
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
                failure::bail!(
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

    let ws = match overidden_target_dir {
        Some(dir) => Workspace::ephemeral(pkg, config, Some(dir), false)?,
        None => {
            let mut ws = Workspace::new(pkg.manifest_path(), config)?;
            ws.set_require_optional_deps(false);
            ws
        }
    };
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
            failure::bail!(
                "Using `cargo install` to install the binaries for the \
                 package in current working directory is no longer supported, \
                 use `cargo install --path .` instead. \
                 Use `cargo build` if you want to simply build the package."
            )
        }
    };

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

    let exec: Arc<dyn Executor> = Arc::new(DefaultExecutor);
    let compile = ops::compile_ws(&ws, Some(source), opts, &exec).chain_err(|| {
        if let Some(td) = td_opt.take() {
            // preserve the temporary directory, so the user can inspect it
            td.into_path();
        }

        failure::format_err!(
            "failed to compile `{}`, intermediate artifacts can be \
             found at `{}`",
            pkg,
            ws.target_dir().display()
        )
    })?;
    let binaries: Vec<(&str, &Path)> = compile
        .binaries
        .iter()
        .map(|bin| {
            let name = bin.file_name().unwrap();
            if let Some(s) = name.to_str() {
                Ok((s, bin.as_ref()))
            } else {
                failure::bail!("Binary `{:?}` name can't be serialized into string", name)
            }
        })
        .collect::<CargoResult<_>>()?;
    if binaries.is_empty() {
        failure::bail!(
            "no binaries are available for install using the selected \
             features"
        );
    }

    let metadata = metadata(config, root)?;
    let mut list = read_crate_list(&metadata)?;
    let dst = metadata.parent().join("bin");
    let duplicates = check_overwrites(&dst, pkg, &opts.filter, &list, force)?;

    fs::create_dir_all(&dst)?;

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
            failure::format_err!("failed to copy `{}` to `{}`", src.display(), dst.display())
        })?;
    }

    let (to_replace, to_install): (Vec<&str>, Vec<&str>) = binaries
        .iter()
        .map(|&(bin, _)| bin)
        .partition(|&bin| duplicates.contains_key(bin));

    let mut installed = Transaction { bins: Vec::new() };

    // Move the temporary copies into `dst` starting with new binaries.
    for bin in to_install.iter() {
        let src = staging_dir.path().join(bin);
        let dst = dst.join(bin);
        config.shell().status("Installing", dst.display())?;
        fs::rename(&src, &dst).chain_err(|| {
            failure::format_err!("failed to move `{}` to `{}`", src.display(), dst.display())
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
                    failure::format_err!(
                        "failed to move `{}` to `{}`",
                        src.display(),
                        dst.display()
                    )
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
            if let Some(set) = list.v1_mut().get_mut(p) {
                set.remove(bin);
            }
        }
        // Failsafe to force replacing metadata for git packages
        // https://github.com/rust-lang/cargo/issues/4582
        if let Some(set) = list.v1_mut().remove(&pkg.package_id()) {
            list.v1_mut().insert(pkg.package_id(), set);
        }
        list.v1_mut()
            .entry(pkg.package_id())
            .or_insert_with(BTreeSet::new)
            .insert(bin.to_string());
    }

    // Remove empty metadata lines.
    let pkgs = list
        .v1()
        .iter()
        .filter_map(|(&p, set)| if set.is_empty() { Some(p) } else { None })
        .collect::<Vec<_>>();
    for p in pkgs.iter() {
        list.v1_mut().remove(p);
    }

    // If installation was successful record newly installed binaries.
    if result.is_ok() {
        list.v1_mut()
            .entry(pkg.package_id())
            .or_insert_with(BTreeSet::new)
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
        paths::remove_dir_all(&target_dir)?;
    }

    Ok(())
}

fn check_overwrites(
    dst: &Path,
    pkg: &Package,
    filter: &ops::CompileFilter,
    prev: &CrateListingV1,
    force: bool,
) -> CargoResult<BTreeMap<String, Option<PackageId>>> {
    // If explicit --bin or --example flags were passed then those'll
    // get checked during cargo_compile, we only care about the "build
    // everything" case here
    if !filter.is_specific() && !pkg.targets().iter().any(|t| t.is_bin()) {
        failure::bail!("specified package has no binaries")
    }
    let duplicates = find_duplicates(dst, pkg, filter, prev);
    if force || duplicates.is_empty() {
        return Ok(duplicates);
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
    Err(failure::format_err!("{}", msg))
}

fn find_duplicates(
    dst: &Path,
    pkg: &Package,
    filter: &ops::CompileFilter,
    prev: &CrateListingV1,
) -> BTreeMap<String, Option<PackageId>> {
    let check = |name: String| {
        // Need to provide type, works around Rust Issue #93349
        let name = format!("{}{}", name, env::consts::EXE_SUFFIX);
        if fs::metadata(dst.join(&name)).is_err() {
            None
        } else if let Some((&p, _)) = prev.v1().iter().find(|&(_, v)| v.contains(&name)) {
            Some((name, Some(p)))
        } else {
            Some((name, None))
        }
    };
    match *filter {
        CompileFilter::Default { .. } => pkg
            .targets()
            .iter()
            .filter(|t| t.is_bin())
            .filter_map(|t| check(t.name().to_string()))
            .collect(),
        CompileFilter::Only {
            ref bins,
            ref examples,
            ..
        } => {
            let all_bins: Vec<String> = bins.try_collect().unwrap_or_else(|| {
                pkg.targets()
                    .iter()
                    .filter(|t| t.is_bin())
                    .map(|t| t.name().to_string())
                    .collect()
            });
            let all_examples: Vec<String> = examples.try_collect().unwrap_or_else(|| {
                pkg.targets()
                    .iter()
                    .filter(|t| t.is_exe_example())
                    .map(|t| t.name().to_string())
                    .collect()
            });

            all_bins
                .iter()
                .chain(all_examples.iter())
                .filter_map(|t| check(t.clone()))
                .collect::<BTreeMap<String, Option<PackageId>>>()
        }
    }
}

pub fn install_list(dst: Option<&str>, config: &Config) -> CargoResult<()> {
    let dst = resolve_root(dst, config)?;
    let dst = metadata(config, &dst)?;
    let list = read_crate_list(&dst)?;
    for (k, v) in list.v1().iter() {
        println!("{}:", k);
        for bin in v {
            println!("    {}", bin);
        }
    }
    Ok(())
}
