use std::default::Default;
use std::fs;
use std::io::prelude::*;
use std::path::Path;

use core::{Package, PackageSet, Profiles, Profile};
use core::source::{Source, SourceMap};
use util::{CargoResult, human, ChainError, Config};
use ops::{self, Layout, Context, BuildConfig, Kind};

pub struct CleanOptions<'a> {
    pub spec: &'a [String],
    pub target: Option<&'a str>,
    pub config: &'a Config,
}

/// Cleans the project from build artifacts.
pub fn clean(manifest_path: &Path, opts: &CleanOptions) -> CargoResult<()> {
    let root = try!(Package::for_path(manifest_path, opts.config));
    let target_dir = opts.config.target_dir(&root);

    // If we have a spec, then we need to delete some packages, otherwise, just
    // remove the whole target directory and be done with it!
    if opts.spec.len() == 0 {
        return rm_rf(&target_dir);
    }

    // Load the lockfile (if one's available)
    let lockfile = root.root().join("Cargo.lock");
    let source_id = root.package_id().source_id();
    let resolve = match try!(ops::load_lockfile(&lockfile, source_id)) {
        Some(resolve) => resolve,
        None => return Err(human("A Cargo.lock must exist before cleaning"))
    };

     // Create a compilation context to have access to information like target
    // filenames and such
    let srcs = SourceMap::new();
    let pkgs = PackageSet::new(&[]);
    let profiles = Profiles::default();
    let cx = try!(Context::new(&resolve, &srcs, &pkgs, opts.config,
                               Layout::at(target_dir),
                               None, BuildConfig::default(),
                               &profiles));

    // resolve package specs and remove the corresponding packages
    for spec in opts.spec {
        let pkgid = try!(resolve.query(spec));

        // Translate the PackageId to a Package
        let pkg = {
            let mut source = pkgid.source_id().load(opts.config);
            try!(source.update());
            (try!(source.get(&[pkgid.clone()]))).into_iter().next().unwrap()
        };

        // And finally, clean everything out!
        for target in pkg.targets().iter() {
            // TODO: `cargo clean --release`
            let layout = Layout::new(opts.config, &root, opts.target, "debug");
            try!(rm_rf(&layout.fingerprint(&pkg)));
            let profiles = [Profile::default_dev(), Profile::default_test()];
            for profile in profiles.iter() {
                for filename in try!(cx.target_filenames(&pkg, target, profile,
                                                         Kind::Target)).iter() {
                    try!(rm_rf(&layout.dest().join(&filename)));
                    try!(rm_rf(&layout.deps().join(&filename)));
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
