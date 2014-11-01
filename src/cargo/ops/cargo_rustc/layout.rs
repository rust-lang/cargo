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
//!     # Root directory for all compiled examples
//!     examples/
//!
//!     # This is the location at which the output of all custom build
//!     # commands are rooted
//!     build/
//!
//!         # Each package gets its own directory where its build script and
//!         # script output are placed
//!         $pkg1/
//!         $pkg2/
//!         $pkg3/
//!
//!             # Each directory package has a `out` directory where output
//!             # is placed.
//!             out/
//!
//!     # This is the location at which the output of all old custom build
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
//!     old-build/
//!     old-fingerprint/
//!     old-examples/
//! ```

use std::io::{mod, fs, IoResult};
use std::io::fs::PathExtensions;

use core::Package;
use util::hex::short_hash;

pub struct Layout {
    root: Path,
    deps: Path,
    native: Path,
    build: Path,
    fingerprint: Path,
    examples: Path,

    old_deps: Path,
    old_root: Path,
    old_native: Path,
    old_build: Path,
    old_fingerprint: Path,
    old_examples: Path,
}

pub struct LayoutProxy<'a> {
    root: &'a Layout,
    primary: bool,
}

impl Layout {
    pub fn new(pkg: &Package, triple: Option<&str>, dest: Option<&str>) -> Layout {
        let mut path = pkg.get_absolute_target_dir();
        match triple {
            Some(s) => path.push(s),
            None => {}
        }
        match dest {
            Some(s) => path.push(s),
            None => {}
        }
        Layout::at(path)
    }

    pub fn at(root: Path) -> Layout {
        Layout {
            deps: root.join("deps"),
            native: root.join("native"),
            build: root.join("build"),
            fingerprint: root.join(".fingerprint"),
            examples: root.join("examples"),
            old_deps: root.join("old-deps"),
            old_root: root.join("old-root"),
            old_native: root.join("old-native"),
            old_build: root.join("old-build"),
            old_fingerprint: root.join("old-fingerprint"),
            old_examples: root.join("old-examples"),
            root: root,
        }
    }

    pub fn prepare(&mut self) -> IoResult<()> {
        if !self.root.exists() {
            try!(fs::mkdir_recursive(&self.root, io::USER_RWX));
        }

        try!(old(&[
            (&self.old_deps, &self.deps),
            (&self.old_native, &self.native),
            (&self.old_fingerprint, &self.fingerprint),
            (&self.old_examples, &self.examples),
        ]));

        if self.old_root.exists() {
            try!(fs::rmdir_recursive(&self.old_root));
        }
        try!(fs::mkdir(&self.old_root, io::USER_RWX));

        for file in try!(fs::readdir(&self.root)).iter() {
            if !file.is_file() { continue }

            try!(fs::rename(file, &self.old_root.join(file.filename().unwrap())));
        }

        return Ok(());

        fn old(dirs: &[(&Path, &Path)]) -> IoResult<()> {
            for &(old, new) in dirs.iter() {
                if old.exists() {
                    try!(fs::rmdir_recursive(old));
                }
                if new.exists() {
                    try!(fs::rename(new, old));
                }
                try!(fs::mkdir(new, io::USER_DIR));
            }
            Ok(())
        }
    }

    pub fn dest<'a>(&'a self) -> &'a Path { &self.root }
    pub fn deps<'a>(&'a self) -> &'a Path { &self.deps }
    pub fn examples<'a>(&'a self) -> &'a Path { &self.examples }

    // TODO: deprecated, remove
    pub fn native(&self, package: &Package) -> Path {
        self.native.join(self.pkg_dir(package))
    }
    pub fn fingerprint(&self, package: &Package) -> Path {
        self.fingerprint.join(self.pkg_dir(package))
    }

    pub fn build(&self, package: &Package) -> Path {
        self.build.join(self.pkg_dir(package))
    }

    pub fn build_out(&self, package: &Package) -> Path {
        self.build(package).join("out")
    }

    pub fn old_dest<'a>(&'a self) -> &'a Path { &self.old_root }
    pub fn old_deps<'a>(&'a self) -> &'a Path { &self.old_deps }
    pub fn old_examples<'a>(&'a self) -> &'a Path { &self.old_examples }

    // TODO: deprecated, remove
    pub fn old_native(&self, package: &Package) -> Path {
        self.old_native.join(self.pkg_dir(package))
    }
    pub fn old_fingerprint(&self, package: &Package) -> Path {
        self.old_fingerprint.join(self.pkg_dir(package))
    }

    pub fn old_build(&self, package: &Package) -> Path {
        self.old_build.join(self.pkg_dir(package))
    }

    fn pkg_dir(&self, pkg: &Package) -> String {
        format!("{}-{}", pkg.get_name(), short_hash(pkg.get_package_id()))
    }
}

impl Drop for Layout {
    fn drop(&mut self) {
        let _ = fs::rmdir_recursive(&self.old_deps);
        let _ = fs::rmdir_recursive(&self.old_root);
        let _ = fs::rmdir_recursive(&self.old_native);
        let _ = fs::rmdir_recursive(&self.old_fingerprint);
        let _ = fs::rmdir_recursive(&self.old_examples);
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

    pub fn examples(&self) -> &'a Path { self.root.examples() }

    // TODO: deprecated, remove
    pub fn native(&self, pkg: &Package) -> Path { self.root.native(pkg) }

    pub fn build(&self, pkg: &Package) -> Path { self.root.build(pkg) }

    pub fn build_out(&self, pkg: &Package) -> Path { self.root.build_out(pkg) }

    pub fn old_root(&self) -> &'a Path {
        if self.primary {self.root.old_dest()} else {self.root.old_deps()}
    }

    pub fn old_examples(&self) -> &'a Path { self.root.old_examples() }

    // TODO: deprecated, remove
    pub fn old_native(&self, pkg: &Package) -> Path {
        self.root.old_native(pkg)
    }

    pub fn old_build(&self, pkg: &Package) -> Path {
        self.root.old_build(pkg)
    }

    pub fn proxy(&self) -> &'a Layout { self.root }
}
