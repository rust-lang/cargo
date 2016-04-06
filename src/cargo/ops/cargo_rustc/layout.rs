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

use std::fs;
use std::io;
use std::path::{PathBuf, Path};

use core::{Package, Target};
use util::{Config, FileLock, CargoResult, Filesystem};
use util::hex::short_hash;

pub struct Layout {
    root: PathBuf,
    deps: PathBuf,
    native: PathBuf,
    build: PathBuf,
    fingerprint: PathBuf,
    examples: PathBuf,
    _lock: FileLock,
}

pub struct LayoutProxy<'a> {
    root: &'a Layout,
    primary: bool,
}

impl Layout {
    pub fn new(config: &Config,
               pkg: &Package,
               triple: Option<&str>,
               dest: &str) -> CargoResult<Layout> {
        let mut path = config.target_dir(pkg);
        // Flexible target specifications often point at filenames, so interpret
        // the target triple as a Path and then just use the file stem as the
        // component for the directory name.
        if let Some(triple) = triple {
            path.push(Path::new(triple).file_stem().unwrap());
        }
        path.push(dest);
        Layout::at(config, path)
    }

    pub fn at(config: &Config, root: Filesystem) -> CargoResult<Layout> {
        // For now we don't do any more finer-grained locking on the artifact
        // directory, so just lock the entire thing for the duration of this
        // compile.
        let lock = try!(root.open_rw(".cargo-lock", config, "build directory"));
        let root = root.into_path_unlocked();

        Ok(Layout {
            deps: root.join("deps"),
            native: root.join("native"),
            build: root.join("build"),
            fingerprint: root.join(".fingerprint"),
            examples: root.join("examples"),
            root: root,
            _lock: lock,
        })
    }

    pub fn prepare(&mut self) -> io::Result<()> {
        if fs::metadata(&self.root).is_err() {
            try!(fs::create_dir_all(&self.root));
        }

        try!(mkdir(&self.deps));
        try!(mkdir(&self.native));
        try!(mkdir(&self.fingerprint));
        try!(mkdir(&self.examples));
        try!(mkdir(&self.build));

        return Ok(());

        fn mkdir(dir: &Path) -> io::Result<()> {
            if fs::metadata(&dir).is_err() {
                try!(fs::create_dir(dir));
            }
            Ok(())
        }
    }

    pub fn dest(&self) -> &Path { &self.root }
    pub fn deps(&self) -> &Path { &self.deps }
    pub fn examples(&self) -> &Path { &self.examples }
    pub fn root(&self) -> &Path { &self.root }

    pub fn fingerprint(&self, package: &Package) -> PathBuf {
        self.fingerprint.join(&self.pkg_dir(package))
    }

    pub fn build(&self, package: &Package) -> PathBuf {
        self.build.join(&self.pkg_dir(package))
    }

    pub fn build_out(&self, package: &Package) -> PathBuf {
        self.build(package).join("out")
    }

    fn pkg_dir(&self, pkg: &Package) -> String {
        format!("{}-{}", pkg.name(), short_hash(pkg))
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

    pub fn build(&self, pkg: &Package) -> PathBuf { self.root.build(pkg) }

    pub fn build_out(&self, pkg: &Package) -> PathBuf { self.root.build_out(pkg) }

    pub fn proxy(&self) -> &'a Layout { self.root }

    pub fn out_dir(&self, pkg: &Package, target: &Target) -> PathBuf {
        if target.is_custom_build() {
            self.build(pkg)
        } else if target.is_example() {
            self.examples().to_path_buf()
        } else {
            self.root().to_path_buf()
        }
    }

    pub fn doc_root(&self) -> PathBuf {
        // the "root" directory ends in 'debug' or 'release', and we want it to
        // end in 'doc' instead
        self.root.root().parent().unwrap().join("doc")
    }
}
