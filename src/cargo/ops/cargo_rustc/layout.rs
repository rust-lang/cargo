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
//! ```

use std::cell::RefCell;
use std::collections::HashSet;
use std::old_io::fs::PathExtensions;
use std::old_io::{self, fs, IoResult};
use std::mem;

use core::Package;
use util::hex::short_hash;

pub struct Layout {
    root: Path,
    deps: Path,
    native: Path,
    build: Path,
    fingerprint: Path,
    examples: Path,
    to_delete: RefCell<HashSet<Path>>,
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
            root: root,
            to_delete: RefCell::new(HashSet::new()),
        }
    }

    pub fn prepare(&mut self) -> IoResult<()> {
        if !self.root.exists() {
            try!(fs::mkdir_recursive(&self.root, old_io::USER_RWX));
        }

        try!(mkdir(self, &self.deps, false));
        try!(mkdir(self, &self.native, false));
        try!(mkdir(self, &self.fingerprint, true));
        try!(mkdir(self, &self.examples, false));
        try!(mkdir(self, &self.build, false));

        for file in try!(fs::readdir(&self.root)).into_iter() {
            if !file.is_file() { continue }

            self.to_delete.borrow_mut().insert(file);
        }

        return Ok(());

        fn mkdir(layout: &Layout, dir: &Path, deep: bool) -> IoResult<()> {
            if dir.exists() {
                if deep {
                    for file in try!(fs::walk_dir(dir)) {
                        if !file.is_dir() {
                            layout.to_delete.borrow_mut().insert(file);
                        }
                    }
                } else {
                    for file in try!(fs::readdir(dir)).into_iter() {
                        layout.to_delete.borrow_mut().insert(file);
                    }
                }
            } else {
                try!(fs::mkdir(dir, old_io::USER_DIR));
            }
            Ok(())
        }
    }

    pub fn dest<'a>(&'a self) -> &'a Path { &self.root }
    pub fn deps<'a>(&'a self) -> &'a Path { &self.deps }
    pub fn examples<'a>(&'a self) -> &'a Path { &self.examples }

    // TODO: deprecated, remove
    pub fn native(&self, package: &Package) -> Path {
        let ret = self.native.join(self.pkg_dir(package));
        self.whitelist(&ret);
        ret
    }
    pub fn fingerprint(&self, package: &Package) -> Path {
        let ret = self.fingerprint.join(self.pkg_dir(package));
        self.whitelist(&ret);
        ret
    }

    pub fn build(&self, package: &Package) -> Path {
        let ret = self.build.join(self.pkg_dir(package));
        self.whitelist(&ret);
        ret
    }

    pub fn build_out(&self, package: &Package) -> Path {
        self.build(package).join("out")
    }

    pub fn clean(&self) -> IoResult<()> {
        let set = mem::replace(&mut *self.to_delete.borrow_mut(),
                               HashSet::new());
        for file in set.iter() {
            info!("dirty: {}", file.display());
            if file.is_dir() {
                try!(fs::rmdir_recursive(file));
            } else {
                try!(fs::unlink(file));
            }
        }
        Ok(())
    }

    pub fn whitelist(&self, p: &Path) {
        self.to_delete.borrow_mut().remove(p);
    }

    fn pkg_dir(&self, pkg: &Package) -> String {
        format!("{}-{}", pkg.get_name(), short_hash(pkg.get_package_id()))
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

    pub fn proxy(&self) -> &'a Layout { self.root }
}
