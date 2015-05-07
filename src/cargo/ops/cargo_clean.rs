use std::default::Default;
use std::fs;
use std::io::prelude::*;
use std::path::Path;

use core::{PackageSet, Profiles, Profile};
use core::source::{Source, SourceMap};
use sources::PathSource;
use util::{CargoResult, human, ChainError, Config};
use ops::{self, Layout, Context, BuildConfig, Kind};

pub struct CleanOptions<'a, 'b: 'a> {
    pub spec: Option<&'a str>,
    pub target: Option<&'a str>,
    pub config: &'a Config<'b>,
}

/// Cleans the project from build artifacts.
pub fn clean(manifest_path: &Path, opts: &CleanOptions) -> CargoResult<()> {
    let mut src = try!(PathSource::for_path(manifest_path.parent().unwrap(),
                                            opts.config));
    try!(src.update());
    let root = try!(src.root_package());
    let manifest = root.manifest();

    // If we have a spec, then we need to delete some package,s otherwise, just
    // remove the whole target directory and be done with it!
    let spec = match opts.spec {
        Some(spec) => spec,
        None => return rm_rf(manifest.target_dir()),
    };

    // Load the lockfile (if one's available), and resolve spec to a pkgid
    let lockfile = root.root().join("Cargo.lock");
    let source_id = root.package_id().source_id();
    let resolve = match try!(ops::load_lockfile(&lockfile, source_id)) {
        Some(resolve) => resolve,
        None => return Err(human("A Cargo.lock must exist before cleaning"))
    };
    let pkgid = try!(resolve.query(spec));

    // Translate the PackageId to a Package
    let pkg = {
        let mut source = pkgid.source_id().load(opts.config);
        try!(source.update());
        (try!(source.get(&[pkgid.clone()]))).into_iter().next().unwrap()
    };

    // Create a compilation context to have access to information like target
    // filenames and such
    let srcs = SourceMap::new();
    let pkgs = PackageSet::new(&[]);
    let profiles = Profiles::default();
    let cx = try!(Context::new(&resolve, &srcs, &pkgs, opts.config,
                               Layout::at(root.absolute_target_dir()),
                               None, &pkg, BuildConfig::default(),
                               &profiles));

    // And finally, clean everything out!
    for target in pkg.targets().iter() {
        // TODO: `cargo clean --release`
        let layout = Layout::new(&root, opts.target, "debug");
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
