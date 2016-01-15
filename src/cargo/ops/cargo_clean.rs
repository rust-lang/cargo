use std::default::Default;
use std::fs;
use std::path::Path;

use core::{Package, PackageSet, Profiles};
use core::source::{Source, SourceMap};
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
    if opts.spec.is_empty() {
        return rm_rf(&target_dir);
    }

    // Load the lockfile (if one's available)
    let lockfile = root.root().join("Cargo.lock");
    let source_id = root.package_id().source_id();
    let resolve = match try!(ops::load_lockfile(&lockfile, source_id)) {
        Some(resolve) => resolve,
        None => bail!("a Cargo.lock must exist before cleaning")
    };

    // Create a compilation context to have access to information like target
    // filenames and such
    let srcs = SourceMap::new();
    let pkgs = PackageSet::new(&[]);

    let dest = if opts.release {"release"} else {"debug"};
    let host_layout = Layout::new(opts.config, &root, None, dest);
    let target_layout = opts.target.map(|target| {
        Layout::new(opts.config, &root, Some(target), dest)
    });

    let cx = try!(Context::new(&resolve, &srcs, &pkgs, opts.config,
                               host_layout, target_layout,
                               BuildConfig::default(),
                               root.manifest().profiles()));

    let mut registry = PackageRegistry::new(opts.config);

    // resolve package specs and remove the corresponding packages
    for spec in opts.spec {
        let pkgid = try!(resolve.query(spec));

        // Translate the PackageId to a Package
        let pkg = {
            try!(registry.add_sources(&[pkgid.source_id().clone()]));
            (try!(registry.get(&[pkgid.clone()]))).into_iter().next().unwrap()
        };

        // And finally, clean everything out!
        for target in pkg.targets() {
            for kind in [Kind::Host, Kind::Target].iter() {
                let layout = cx.layout(&pkg, *kind);
                try!(rm_rf(&layout.proxy().fingerprint(&pkg)));
                try!(rm_rf(&layout.build(&pkg)));
                let Profiles {
                    ref release, ref dev, ref test, ref bench, ref doc,
                    ref custom_build,
                } = *root.manifest().profiles();
                for profile in [release, dev, test, bench, doc, custom_build].iter() {
                    let unit = Unit {
                        pkg: &pkg,
                        target: target,
                        profile: profile,
                        kind: *kind,
                    };
                    let root = cx.out_dir(&unit);
                    for filename in try!(cx.target_filenames(&unit)).iter() {
                        try!(rm_rf(&root.join(&filename)));
                    }
                }
            }
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
