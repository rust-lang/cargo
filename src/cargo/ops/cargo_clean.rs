use std::default::Default;
use std::fs;
use std::path::Path;

use core::{Profiles, Workspace};
use util::Config;
use util::errors::{CargoResult, CargoResultExt};
use util::paths;
use ops::{self, BuildConfig, Context, Kind, Unit};

pub struct CleanOptions<'a> {
    pub config: &'a Config,
    pub spec: Vec<String>,
    pub target: Option<String>,
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
    let host_triple = opts.config.rustc()?.host.clone();
    let mut cx = Context::new(
        ws,
        &resolve,
        &packages,
        opts.config,
        BuildConfig {
            host_triple,
            requested_target: opts.target.clone(),
            release: opts.release,
            jobs: 1,
            ..BuildConfig::default()
        },
        profiles,
    )?;
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

    cx.probe_target_info(&units)?;

    for unit in units.iter() {
        rm_rf(&cx.fingerprint_dir(unit), config)?;
        if unit.target.is_custom_build() {
            if unit.profile.run_custom_build {
                rm_rf(&cx.build_script_out_dir(unit), config)?;
            } else {
                rm_rf(&cx.build_script_dir(unit), config)?;
            }
            continue;
        }

        for &(ref src, ref link_dst, _) in cx.target_filenames(unit)?.iter() {
            rm_rf(src, config)?;
            if let Some(ref dst) = *link_dst {
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
