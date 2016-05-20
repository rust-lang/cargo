use std::default::Default;
use std::fs;
use std::path::Path;

use core::{Package, Profiles};
use core::registry::PackageRegistry;
use util::{CargoResult, human, ChainError, Config};
use ops::{self, Layout, Context, BuildConfig, Kind, Unit};

pub struct CleanOptions<'a> {
    pub spec: &'a [String],
    pub target: Option<&'a str>,
    pub config: &'a Config,
    pub release: bool,
}

/// Cleans the project from build artifacts.
pub fn clean(manifest_path: &Path, opts: &CleanOptions) -> CargoResult<()> {
    let root = try!(Package::for_path(manifest_path, opts.config));
    let target_dir = opts.config.target_dir(&root);

    // If we have a spec, then we need to delete some packages, otherwise, just
    // remove the whole target directory and be done with it!
    //
    // Note that we don't bother grabbing a lock here as we're just going to
    // blow it all away anyway.
    if opts.spec.is_empty() {
        let target_dir = target_dir.into_path_unlocked();
        return rm_rf(&target_dir);
    }

    let mut registry = PackageRegistry::new(opts.config);
    let resolve = try!(ops::resolve_pkg(&mut registry, &root, opts.config));
    let packages = ops::get_resolved_packages(&resolve, registry);

    let dest = if opts.release {"release"} else {"debug"};
    let host_layout = try!(Layout::new(opts.config, &root, None, dest));
    let target_layout = match opts.target {
        Some(target) => {
            Some(try!(Layout::new(opts.config, &root, Some(target), dest)))
        }
        None => None,
    };

    let mut cx = try!(Context::new(&resolve, &packages, opts.config,
                                   host_layout, target_layout,
                                   BuildConfig::default(),
                                   root.manifest().profiles()));
    let mut units = Vec::new();

    for spec in opts.spec {
        // Translate the spec to a Package
        let pkgid = try!(resolve.query(spec));
        let pkg = try!(packages.get(&pkgid));

        // Generate all relevant `Unit` targets for this package
        for target in pkg.targets() {
            for kind in [Kind::Host, Kind::Target].iter() {
                let Profiles {
                    ref release, ref dev, ref test, ref bench, ref doc,
                    ref custom_build, ref test_deps, ref bench_deps,
                } = *root.manifest().profiles();
                let profiles = [release, dev, test, bench, doc, custom_build,
                                test_deps, bench_deps];
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

    try!(cx.probe_target_info(&units));

    for unit in units.iter() {
        let layout = cx.layout(&unit.pkg, unit.kind);
        try!(rm_rf(&layout.proxy().fingerprint(&unit.pkg)));
        try!(rm_rf(&layout.build(&unit.pkg)));

        let root = cx.out_dir(&unit);
        for (filename, _) in try!(cx.target_filenames(&unit)) {
            try!(rm_rf(&root.join(&filename)));
        }
    }

    Ok(())
}

fn rm_rf(path: &Path) -> CargoResult<()> {
    let m = fs::metadata(path);
    if m.as_ref().map(|s| s.is_dir()).unwrap_or(false) {
        try!(fs::remove_dir_all(path).chain_error(|| {
            human("could not remove build directory")
        }));
    } else if m.is_ok() {
        try!(fs::remove_file(path).chain_error(|| {
            human("failed to remove build artifact")
        }));
    }
    Ok(())
}
