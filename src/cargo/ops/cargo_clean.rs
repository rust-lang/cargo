use crate::core::InternedString;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::core::compiler::unit_dependencies;
use crate::core::compiler::{BuildConfig, BuildContext, CompileKind, CompileMode, Context};
use crate::core::compiler::{RustcTargetData, UnitInterner};
use crate::core::profiles::{Profiles, UnitFor};
use crate::core::resolver::features::HasDevUnits;
use crate::core::resolver::ResolveOpts;
use crate::core::{PackageIdSpec, Workspace};
use crate::ops;
use crate::ops::resolve::WorkspaceResolve;
use crate::util::errors::{CargoResult, CargoResultExt};
use crate::util::paths;
use crate::util::Config;

pub struct CleanOptions<'a> {
    pub config: &'a Config,
    /// A list of packages to clean. If empty, everything is cleaned.
    pub spec: Vec<String>,
    /// The target arch triple to clean, or None for the host arch
    pub target: Option<String>,
    /// Whether to clean the release directory
    pub profile_specified: bool,
    /// Whether to clean the directory of a certain build profile
    pub requested_profile: InternedString,
    /// Whether to just clean the doc directory
    pub doc: bool,
}

/// Cleans the package's build artifacts.
pub fn clean(ws: &Workspace<'_>, opts: &CleanOptions<'_>) -> CargoResult<()> {
    let mut target_dir = ws.target_dir();
    let config = ws.config();

    // If the doc option is set, we just want to delete the doc directory.
    if opts.doc {
        target_dir = target_dir.join("doc");
        return rm_rf(&target_dir.into_path_unlocked(), config);
    }

    let profiles = Profiles::new(ws.profiles(), config, opts.requested_profile, ws.features())?;

    if opts.profile_specified {
        // After parsing profiles we know the dir-name of the profile, if a profile
        // was passed from the command line. If so, delete only the directory of
        // that profile.
        let dir_name = profiles.get_dir_name();
        target_dir = target_dir.join(dir_name);
    }

    // If we have a spec, then we need to delete some packages, otherwise, just
    // remove the whole target directory and be done with it!
    //
    // Note that we don't bother grabbing a lock here as we're just going to
    // blow it all away anyway.
    if opts.spec.is_empty() {
        return rm_rf(&target_dir.into_path_unlocked(), config);
    }
    let mut build_config = BuildConfig::new(config, Some(1), &opts.target, CompileMode::Build)?;
    build_config.requested_profile = opts.requested_profile;
    let target_data = RustcTargetData::new(ws, build_config.requested_kind)?;
    // Resolve for default features. In the future, `cargo clean` should be rewritten
    // so that it doesn't need to guess filename hashes.
    let resolve_opts = ResolveOpts::new(
        /*dev_deps*/ true,
        &[],
        /*all features*/ false,
        /*default*/ true,
    );
    let specs = opts
        .spec
        .iter()
        .map(|spec| PackageIdSpec::parse(spec))
        .collect::<CargoResult<Vec<_>>>()?;
    let ws_resolve = ops::resolve_ws_with_opts(
        ws,
        &target_data,
        build_config.requested_kind,
        &resolve_opts,
        &specs,
        HasDevUnits::Yes,
    )?;
    let WorkspaceResolve {
        pkg_set,
        targeted_resolve: resolve,
        resolved_features: features,
        ..
    } = ws_resolve;

    let interner = UnitInterner::new();
    let bcx = BuildContext::new(
        ws,
        &pkg_set,
        opts.config,
        &build_config,
        profiles,
        &interner,
        HashMap::new(),
        target_data,
    )?;
    let mut units = Vec::new();

    for spec in opts.spec.iter() {
        // Translate the spec to a Package
        let pkgid = resolve.query(spec)?;
        let pkg = pkg_set.get_one(pkgid)?;

        // Generate all relevant `Unit` targets for this package
        for target in pkg.targets() {
            for kind in [CompileKind::Host, build_config.requested_kind].iter() {
                for mode in CompileMode::all_modes() {
                    if target.is_custom_build() && (mode.is_any_test() || !kind.is_host()) {
                        // Workaround where the UnitFor code will panic
                        // because it is not expecting strange combinations
                        // like "testing a build script".
                        continue;
                    }
                    for unit_for in UnitFor::all_values() {
                        let profile = if mode.is_run_custom_build() {
                            bcx.profiles
                                .get_profile_run_custom_build(&bcx.profiles.get_profile(
                                    pkg.package_id(),
                                    ws.is_member(pkg),
                                    *unit_for,
                                    CompileMode::Build,
                                ))
                        } else {
                            bcx.profiles.get_profile(
                                pkg.package_id(),
                                ws.is_member(pkg),
                                *unit_for,
                                *mode,
                            )
                        };
                        // Use unverified here since this is being more
                        // exhaustive than what is actually needed.
                        let features_for = unit_for.map_to_features_for();
                        let features =
                            features.activated_features_unverified(pkg.package_id(), features_for);
                        units.push(bcx.units.intern(
                            pkg, target, profile, *kind, *mode, features, /*is_std*/ false,
                        ));
                    }
                }
            }
        }
    }

    let unit_dependencies =
        unit_dependencies::build_unit_dependencies(&bcx, &resolve, &features, None, &units, &[])?;
    let mut cx = Context::new(config, &bcx, unit_dependencies, build_config.requested_kind)?;
    cx.prepare_units(None, &units)?;

    for unit in units.iter() {
        if unit.mode.is_doc() || unit.mode.is_doc_test() {
            // Cleaning individual rustdoc crates is currently not supported.
            // For example, the search index would need to be rebuilt to fully
            // remove it (otherwise you're left with lots of broken links).
            // Doc tests produce no output.
            continue;
        }
        rm_rf(&cx.files().fingerprint_dir(unit), config)?;
        if unit.target.is_custom_build() {
            if unit.mode.is_run_custom_build() {
                rm_rf(&cx.files().build_script_out_dir(unit), config)?;
            } else {
                rm_rf(&cx.files().build_script_dir(unit), config)?;
            }
            continue;
        }

        for output in cx.outputs(unit)?.iter() {
            rm_rf(&output.path, config)?;
            if let Some(ref dst) = output.hardlink {
                rm_rf(dst, config)?;
            }
        }
    }

    Ok(())
}

fn rm_rf(path: &Path, config: &Config) -> CargoResult<()> {
    let m = fs::metadata(path);
    if m.as_ref().map(|s| s.is_dir()).unwrap_or(false) {
        config
            .shell()
            .verbose(|shell| shell.status("Removing", path.display()))?;
        paths::remove_dir_all(path)
            .chain_err(|| anyhow::format_err!("could not remove build directory"))?;
    } else if m.is_ok() {
        config
            .shell()
            .verbose(|shell| shell.status("Removing", path.display()))?;
        paths::remove_file(path)
            .chain_err(|| anyhow::format_err!("failed to remove build artifact"))?;
    }
    Ok(())
}
