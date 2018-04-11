use std::fs;
use std::path::Path;

use core::{Profiles, Workspace};
use util::Config;
use util::errors::{CargoResult, CargoResultExt};
use util::paths;
use ops::{self, BuildConfig, Context, Kind, Unit};

pub struct CleanOptions<'a> {
    pub config: &'a Config,
    /// A list of packages to clean. If empty, everything is cleaned.
    pub spec: Vec<String>,
    /// The target arch triple to clean, or None for the host arch
    pub target: Option<String>,
    /// Whether to clean the release directory
    pub release: bool,
}

/// Cleans the project from build artifacts.
pub fn clean(ws: &Workspace, opts: &CleanOptions) -> CargoResult<()> {
    let target_dir = ws.target_dir();
    let config = ws.config();

    // If we have a spec, then we need to delete some packages, otherwise, just
    // remove the whole target directory and be done with it!
    //
    // Note that we don't bother grabbing a lock here as we're just going to
    // blow it all away anyway.
    if opts.spec.is_empty() {
        let target_dir = target_dir.into_path_unlocked();
        return rm_rf(&target_dir, config);
    }

    let (packages, resolve) = ops::resolve_ws(ws)?;

    let profiles = ws.profiles();
    let mut units = Vec::new();

    for spec in opts.spec.iter() {
        // Translate the spec to a Package
        let pkgid = resolve.query(spec)?;
        let pkg = packages.get(pkgid)?;

        // Generate all relevant `Unit` targets for this package
        for target in pkg.targets() {
            for kind in [Kind::Host, Kind::Target].iter() {
                let Profiles {
                    ref release,
                    ref dev,
                    ref test,
                    ref bench,
                    ref doc,
                    ref custom_build,
                    ref test_deps,
                    ref bench_deps,
                    ref check,
                    ref check_test,
                    ref doctest,
                } = *profiles;
                let profiles = [
                    release,
                    dev,
                    test,
                    bench,
                    doc,
                    custom_build,
                    test_deps,
                    bench_deps,
                    check,
                    check_test,
                    doctest,
                ];
                for profile in profiles.iter() {
                    units.push(Unit {
                        pkg,
                        target,
                        profile,
                        kind: *kind,
                    });
                }
            }
        }
    }

    let mut build_config = BuildConfig::new(&opts.config.rustc()?.host, &opts.target)?;
    build_config.release = opts.release;
    let mut cx = Context::new(ws, &resolve, &packages, opts.config, build_config, profiles)?;
    cx.prepare_units(None, &units)?;

    for unit in units.iter() {
        rm_rf(&cx.files().fingerprint_dir(unit), config)?;
        if unit.target.is_custom_build() {
            if unit.profile.run_custom_build {
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
        paths::remove_dir_all(path).chain_err(|| format_err!("could not remove build directory"))?;
    } else if m.is_ok() {
        config
            .shell()
            .verbose(|shell| shell.status("Removing", path.display()))?;
        paths::remove_file(path).chain_err(|| format_err!("failed to remove build artifact"))?;
    }
    Ok(())
}
