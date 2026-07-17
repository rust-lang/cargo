use crate::core::PackageId;
use crate::core::{PackageIdSpec, PackageIdSpecQuery, SourceId};
use crate::ops::common_for_install_and_uninstall::*;
use crate::sources::PathSource;
use crate::util::Filesystem;
use crate::util::GlobalContext;
use crate::util::errors::CargoResult;
use anyhow::bail;
use std::collections::BTreeSet;
use std::env;

pub fn uninstall(
    root: Option<&str>,
    specs: Vec<&str>,
    bins: &[String],
    gctx: &GlobalContext,
) -> CargoResult<()> {
    if specs.len() > 1 && !bins.is_empty() {
        bail!(
            "A binary can only be associated with a single installed package, specifying multiple specs with --bin is redundant."
        );
    }

    let root = resolve_root(root, gctx)?;
    let scheduled_error = if specs.len() == 1 {
        uninstall_one(&root, specs[0], bins, gctx)?;
        false
    } else if specs.is_empty() {
        uninstall_cwd(&root, bins, gctx)?;
        false
    } else {
        let mut succeeded = vec![];
        let mut failed = vec![];
        for spec in specs {
            let root = root.clone();
            match uninstall_one(&root, spec, bins, gctx) {
                Ok(()) => succeeded.push(spec),
                Err(e) => {
                    crate::display_error(&e, &mut gctx.shell());
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
            gctx.shell().status("Summary", summary.join(" "))?;
        }

        !failed.is_empty()
    };

    if scheduled_error {
        bail!("some packages failed to uninstall");
    }

    Ok(())
}

pub fn uninstall_one(
    root: &Filesystem,
    spec: &str,
    bins: &[String],
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let tracker = InstallTracker::load(gctx, root)?;
    let all_pkgs = tracker.all_installed_bins().map(|(pkg_id, _set)| *pkg_id);
    let pkgid = PackageIdSpec::query_str(spec, all_pkgs)?;
    uninstall_pkgid(root, tracker, pkgid, bins, gctx)
}

fn uninstall_cwd(root: &Filesystem, bins: &[String], gctx: &GlobalContext) -> CargoResult<()> {
    let tracker = InstallTracker::load(gctx, root)?;
    let source_id = SourceId::for_path(gctx.cwd())?;
    let mut src = path_source(source_id, gctx)?;
    let pkg = select_pkg(
        &mut src,
        None,
        |path: &mut PathSource<'_>| path.root_package().map(|p| vec![p]),
        gctx,
        None,
    )?;
    let pkgid = pkg.package_id();
    uninstall_pkgid(root, tracker, pkgid, bins, gctx)
}

fn uninstall_pkgid(
    root: &Filesystem,
    mut tracker: InstallTracker,
    pkgid: PackageId,
    bins: &[String],
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let installed = match tracker.installed_bins(pkgid) {
        Some(bins) => bins.clone(),
        None => bail!("package `{}` is not installed", pkgid),
    };

    let dst = root.join("bin").into_path_unlocked();
    for bin in &installed {
        let bin = dst.join(bin);
        if !bin.exists() {
            bail!(
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
        .collect::<BTreeSet<_>>();

    for bin in bins.iter() {
        if !installed.contains(bin) {
            bail!("binary `{}` not installed as part of `{}`", bin, pkgid)
        }
    }

    let to_remove = { if bins.is_empty() { installed } else { bins } };

    for bin in to_remove {
        let bin_path = dst.join(&bin);
        gctx.shell().status("Removing", bin_path.display())?;
        tracker.remove_bin_then_save(pkgid, &bin, &bin_path)?;
    }

    Ok(())
}
