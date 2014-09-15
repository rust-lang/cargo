//! Management of the directory layout of a build
//!
//! The directory layout is a little tricky at times, hence a separate file to
//! house this logic. The current layout looks like this:
//!
//! ```ignore
//! # This is the root directory for all output, the top-level package
//! # places all of its output here.
//! target/
//!
//!     # This is the root directory for all output of *dependencies*
//!     deps/
//!
//!     # This is the location at which the output of all custom build
//!     # commands are rooted
//!     native/
//!
//!         # Each package gets its own directory for where its output is
//!         # placed. We can't track exactly what's getting put in here, so
//!         # we just assume that all relevant output is in these
//!         # directories.
//!         $pkg1/
//!         $pkg2/
//!         $pkg3/
//!
//!     # Hidden directory that holds all of the fingerprint files for all
//!     # packages
//!     .fingerprint/
//!
//!     # This is a temporary directory as part of the build process. When a
//!     # build starts, it initially moves the old `deps` directory to this
//!     # location. This is done to ensure that there are no stale artifacts
//!     # lying around in the build directory which may cause a build to
//!     # succeed where it would fail elsewhere.
//!     #
//!     # If a package is determined to be fresh, its files are moved out of
//!     # this directory and back into `deps`.
//!     old-deps/
//!
//!     # Similar to old-deps, this is where all of the output under
//!     # `target/` is moved at the start of a build.
//!     old-root/
//!
//!     # Same as the two above old directories
//!     old-native/
//!     old-fingerprint/
//! ```

use std::io::{mod, fs, IoResult};
use std::io::fs::PathExtensions;

use core::Package;
use util::hex::short_hash;

pub struct Layout {
    root: Path,
    deps: Path,
    native: Path,
    fingerprint: Path,

    old_deps: Path,
    old_root: Path,
    old_native: Path,
    old_fingerprint: Path,
}

pub struct LayoutProxy<'a> {
    root: &'a Layout,
    primary: bool,
}

impl Layout {
    pub fn new(root: Path) -> Layout {
        Layout {
            deps: root.join("deps"),
            native: root.join("native"),
            fingerprint: root.join(".fingerprint"),
            old_deps: root.join("old-deps"),
            old_root: root.join("old-root"),
            old_native: root.join("old-native"),
            old_fingerprint: root.join("old-fingerprint"),
            root: root,
        }
    }

    pub fn prepare(&mut self) -> IoResult<()> {
        if !self.root.exists() {
            try!(fs::mkdir_recursive(&self.root, io::UserRWX));
        }

        if self.old_deps.exists() {
            try!(fs::rmdir_recursive(&self.old_deps));
        }
        if self.old_root.exists() {
            try!(fs::rmdir_recursive(&self.old_root));
        }
        if self.old_native.exists() {
            try!(fs::rmdir_recursive(&self.old_native));
        }
        if self.old_fingerprint.exists() {
            try!(fs::rmdir_recursive(&self.old_fingerprint));
        }
        if self.deps.exists() {
            try!(fs::rename(&self.deps, &self.old_deps));
        }
        if self.native.exists() {
            try!(fs::rename(&self.native, &self.old_native));
        }
        if self.fingerprint.exists() {
            try!(fs::rename(&self.fingerprint, &self.old_fingerprint));
        }

        try!(fs::mkdir(&self.deps, io::UserRWX));
        try!(fs::mkdir(&self.native, io::UserRWX));
        try!(fs::mkdir(&self.fingerprint, io::UserRWX));
        try!(fs::mkdir(&self.old_root, io::UserRWX));

        for file in try!(fs::readdir(&self.root)).iter() {
            if !file.is_file() { continue }

            try!(fs::rename(file, &self.old_root.join(file.filename().unwrap())));
        }

        Ok(())
    }

    pub fn dest<'a>(&'a self) -> &'a Path { &self.root }
    pub fn deps<'a>(&'a self) -> &'a Path { &self.deps }
    pub fn native(&self, package: &Package) -> Path {
        self.native.join(self.native_name(package))
    }
    pub fn fingerprint(&self) -> &Path { &self.fingerprint }

    pub fn old_dest<'a>(&'a self) -> &'a Path { &self.old_root }
    pub fn old_deps<'a>(&'a self) -> &'a Path { &self.old_deps }
    pub fn old_native(&self, package: &Package) -> Path {
        self.old_native.join(self.native_name(package))
    }
    pub fn old_fingerprint(&self) -> &Path { &self.old_fingerprint }

    fn native_name(&self, pkg: &Package) -> String {
        format!("{}-{}", pkg.get_name(), short_hash(pkg.get_package_id()))
    }
}

impl Drop for Layout {
    fn drop(&mut self) {
        let _ = fs::rmdir_recursive(&self.old_deps);
        let _ = fs::rmdir_recursive(&self.old_root);
        let _ = fs::rmdir_recursive(&self.old_native);
        let _ = fs::rmdir_recursive(&self.old_fingerprint);
    }
}

impl<'a> LayoutProxy<'a> {
    pub fn new(root: &'a Layout, primary: bool) -> LayoutProxy<'a> {
        LayoutProxy {
            root: root,
            primary: primary,
        }
    }

    pub fn root(&self) -> &'a Path {
        if self.primary {self.root.dest()} else {self.root.deps()}
    }
    pub fn deps(&self) -> &'a Path { self.root.deps() }

    pub fn native(&self, pkg: &Package) -> Path { self.root.native(pkg) }

    pub fn old_root(&self) -> &'a Path {
        if self.primary {self.root.old_dest()} else {self.root.old_deps()}
    }

    pub fn old_native(&self, pkg: &Package) -> Path {
        self.root.old_native(pkg)
    }

    pub fn proxy(&self) -> &'a Layout { self.root }
}
