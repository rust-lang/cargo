use std::default::Default;
use std::fs;
use std::path::Path;

use core::{Profiles, Workspace};
use util::Config;
use util::errors::{CargoResult, CargoResultExt};
use ops::{self, Context, BuildConfig, Kind, Unit};

pub struct CleanOptions<'a> {
    pub spec: &'a [String],
    pub target: Option<&'a str>,
    pub config: &'a Config,
    pub release: bool,
}

/// Cleans the project from build artifacts.
pub fn clean(ws: &Workspace, opts: &CleanOptions) -> CargoResult<()> {
    let target_dir = ws.target_dir();

    // If we have a spec, then we need to delete some packages, otherwise, just
    // remove the whole target directory and be done with it!
    //
    // Note that we don't bother grabbing a lock here as we're just going to
    // blow it all away anyway.
    if opts.spec.is_empty() {
        let target_dir = target_dir.into_path_unlocked();
        return rm_rf(&target_dir);
    }

    let (packages, resolve) = ops::resolve_ws(ws)?;

    let profiles = ws.profiles();
    let host_triple = opts.config.rustc()?.host.clone();
    let mut cx = Context::new(ws, &resolve, &packages, opts.config,
                                   BuildConfig {
                                       host_triple: host_triple,
                                       requested_target: opts.target.map(|s| s.to_owned()),
                                       release: opts.release,
                                       jobs: 1,
                                       ..BuildConfig::default()
                                   },
                                   profiles)?;
    let mut units = Vec::new();

    for spec in opts.spec {
        // Translate the spec to a Package
        let pkgid = resolve.query(spec)?;
        let pkg = packages.get(&pkgid)?;

        // Generate all relevant `Unit` targets for this package
        for target in pkg.targets() {
            for kind in [Kind::Host, Kind::Target].iter() {
                let Profiles {
                    ref release, ref dev, ref test, ref bench, ref doc,
                    ref custom_build, ref test_deps, ref bench_deps, ref check,
                    ref doctest,
                } = *profiles;
                let profiles = [release, dev, test, bench, doc, custom_build,
                                test_deps, bench_deps, check, doctest];
                for profile in profiles.iter() {
                    units.push(Unit {
                        pkg: &pkg,
                        target: target,
                        profile: profile,
                        kind: *kind,
                    });
                }
            }
        }
    }

    cx.probe_target_info(&units)?;

    for unit in units.iter() {
        rm_rf(&cx.fingerprint_dir(unit))?;
        if unit.target.is_custom_build() {
            if unit.profile.run_custom_build {
                rm_rf(&cx.build_script_out_dir(unit))?;
            } else {
                rm_rf(&cx.build_script_dir(unit))?;
            }
            continue
        }

        for &(ref src, ref link_dst, _) in cx.target_filenames(unit)?.iter() {
            rm_rf(src)?;
            if let Some(ref dst) = *link_dst {
                rm_rf(dst)?;
            }
        }
    }

    Ok(())
}

fn rm_rf(path: &Path) -> CargoResult<()> {
    let m = fs::metadata(path);
    if m.as_ref().map(|s| s.is_dir()).unwrap_or(false) {
        fs::remove_dir_all(path).chain_err(|| {
            "could not remove build directory"
        })?;
    } else if m.is_ok() {
        fs::remove_file(path).chain_err(|| {
            "failed to remove build artifact"
        })?;
    }
    Ok(())
}
