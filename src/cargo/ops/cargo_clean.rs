use std::io::fs::{mod, PathExtensions};

use core::{MultiShell, PackageSet};
use core::source::{Source, SourceMap};
use sources::PathSource;
use util::{CargoResult, human, ChainError, Config};
use ops::{mod, Layout, Context};

pub struct CleanOptions<'a> {
    pub spec: Option<&'a str>,
    pub target: Option<&'a str>,
    pub shell: &'a mut MultiShell,
}

/// Cleans the project from build artifacts.
pub fn clean(manifest_path: &Path, opts: &mut CleanOptions) -> CargoResult<()> {
    let mut src = try!(PathSource::for_path(&manifest_path.dir_path()));
    try!(src.update());
    let root = try!(src.get_root_package());
    let manifest = root.get_manifest();

    // If we have a spec, then we need to delete some package,s otherwise, just
    // remove the whole target directory and be done with it!
    let spec = match opts.spec {
        Some(spec) => spec,
        None => return rm_rf(manifest.get_target_dir()),
    };

    // Load the lockfile (if one's available), and resolve spec to a pkgid
    let lockfile = root.get_root().join("Cargo.lock");
    let source_id = root.get_package_id().get_source_id();
    let resolve = match try!(ops::load_lockfile(&lockfile, source_id)) {
        Some(resolve) => resolve,
        None => return Err(human("A Cargo.lock must exist before cleaning"))
    };
    let pkgid = try!(resolve.query(spec));

    // Translate the PackageId to a Package
    let mut cfg = try!(Config::new(opts.shell, None, None));
    let pkg = {
        let mut source = pkgid.get_source_id().load(&mut cfg);
        try!(source.update());
        (try!(source.get([pkgid.clone()]))).into_iter().next().unwrap()
    };

    // Create a compilation context to have access to information like target
    // filenames and such
    let srcs = SourceMap::new();
    let pkgs = PackageSet::new([]);
    let cx = try!(Context::new("compile", &resolve, &srcs, &pkgs, &mut cfg,
                               Layout::at(root.get_absolute_target_dir()),
                               None, &pkg));

    // And finally, clean everything out!
    for target in pkg.get_targets().iter() {
        let layout = Layout::new(&root, opts.target,
                                 target.get_profile().get_dest());
        try!(rm_rf(&layout.native(&pkg)));
        try!(rm_rf(&layout.fingerprint(&pkg)));
        for filename in try!(cx.target_filenames(target)).iter() {
            let filename = filename.as_slice();
            try!(rm_rf(&layout.dest().join(filename)));
            try!(rm_rf(&layout.deps().join(filename)));
        }
    }

    Ok(())
}

fn rm_rf(path: &Path) -> CargoResult<()> {
    if path.is_dir() {
        try!(fs::rmdir_recursive(path).chain_error(|| {
            human("could not remove build directory")
        }));
    } else if path.exists() {
        try!(fs::unlink(path).chain_error(|| {
            human("failed to remove build artifact")
        }));
    }
    Ok(())
}
