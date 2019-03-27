use std::collections::btree_map::Entry;
use std::{env, fs};

use crate::core::PackageId;
use crate::core::{PackageIdSpec, SourceId};
use crate::ops::common_for_install_and_uninstall::*;
use crate::util::errors::CargoResult;
use crate::util::paths;
use crate::util::Config;
use crate::util::{FileLock, Filesystem};

pub fn uninstall(
    root: Option<&str>,
    specs: Vec<&str>,
    bins: &[String],
    config: &Config,
) -> CargoResult<()> {
    if specs.len() > 1 && !bins.is_empty() {
        failure::bail!("A binary can only be associated with a single installed package, specifying multiple specs with --bin is redundant.");
    }

    let root = resolve_root(root, config)?;
    let scheduled_error = if specs.len() == 1 {
        uninstall_one(&root, specs[0], bins, config)?;
        false
    } else if specs.is_empty() {
        uninstall_cwd(&root, bins, config)?;
        false
    } else {
        let mut succeeded = vec![];
        let mut failed = vec![];
        for spec in specs {
            let root = root.clone();
            match uninstall_one(&root, spec, bins, config) {
                Ok(()) => succeeded.push(spec),
                Err(e) => {
                    crate::handle_error(&e, &mut config.shell());
                    failed.push(spec)
                }
            }
        }

        let mut summary = vec![];
        if !succeeded.is_empty() {
            summary.push(format!(
                "Successfully uninstalled {}!",
                succeeded.join(", ")
            ));
        }
        if !failed.is_empty() {
            summary.push(format!(
                "Failed to uninstall {} (see error(s) above).",
                failed.join(", ")
            ));
        }

        if !succeeded.is_empty() || !failed.is_empty() {
            config.shell().status("Summary", summary.join(" "))?;
        }

        !failed.is_empty()
    };

    if scheduled_error {
        failure::bail!("some packages failed to uninstall");
    }

    Ok(())
}

pub fn uninstall_one(
    root: &Filesystem,
    spec: &str,
    bins: &[String],
    config: &Config,
) -> CargoResult<()> {
    let crate_metadata = metadata(config, root)?;
    let metadata = read_crate_list(&crate_metadata)?;
    let pkgid = PackageIdSpec::query_str(spec, metadata.v1().keys().cloned())?;
    uninstall_pkgid(&crate_metadata, metadata, pkgid, bins, config)
}

fn uninstall_cwd(root: &Filesystem, bins: &[String], config: &Config) -> CargoResult<()> {
    let crate_metadata = metadata(config, root)?;
    let metadata = read_crate_list(&crate_metadata)?;
    let source_id = SourceId::for_path(config.cwd())?;
    let src = path_source(source_id, config)?;
    let (pkg, _source) = select_pkg(src, None, None, config, true, &mut |path| {
        path.read_packages()
    })?;
    let pkgid = pkg.package_id();
    uninstall_pkgid(&crate_metadata, metadata, pkgid, bins, config)
}

fn uninstall_pkgid(
    crate_metadata: &FileLock,
    mut metadata: CrateListingV1,
    pkgid: PackageId,
    bins: &[String],
    config: &Config,
) -> CargoResult<()> {
    let mut to_remove = Vec::new();
    {
        let mut installed = match metadata.v1_mut().entry(pkgid) {
            Entry::Occupied(e) => e,
            Entry::Vacant(..) => failure::bail!("package `{}` is not installed", pkgid),
        };

        let dst = crate_metadata.parent().join("bin");
        for bin in installed.get() {
            let bin = dst.join(bin);
            if fs::metadata(&bin).is_err() {
                failure::bail!(
                    "corrupt metadata, `{}` does not exist when it should",
                    bin.display()
                )
            }
        }

        let bins = bins
            .iter()
            .map(|s| {
                if s.ends_with(env::consts::EXE_SUFFIX) {
                    s.to_string()
                } else {
                    format!("{}{}", s, env::consts::EXE_SUFFIX)
                }
            })
            .collect::<Vec<_>>();

        for bin in bins.iter() {
            if !installed.get().contains(bin) {
                failure::bail!("binary `{}` not installed as part of `{}`", bin, pkgid)
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
    write_crate_list(crate_metadata, metadata)?;
    for bin in to_remove {
        config.shell().status("Removing", bin.display())?;
        paths::remove_file(bin)?;
    }

    Ok(())
}
