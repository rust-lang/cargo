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
//!     # Directory used to store incremental data for the compiler (when
//!     # incremental is enabled.
//!     incremental/
//!
//!     # Hidden directory that holds all of the fingerprint files for all
//!     # packages
//!     .fingerprint/
//! ```

use std::fs;
use std::io;
use std::path::{PathBuf, Path};

use core::Workspace;
use util::{Config, FileLock, CargoResult, Filesystem, human};

pub struct Layout {
    root: PathBuf,
    deps: PathBuf,
    native: PathBuf,
    build: PathBuf,
    incremental: PathBuf,
    fingerprint: PathBuf,
    examples: PathBuf,
    _lock: FileLock,
}

impl Layout {
    pub fn new(ws: &Workspace,
               triple: Option<&str>,
               dest: &str) -> CargoResult<Layout> {
        let mut path = ws.target_dir();
        // Flexible target specifications often point at filenames, so interpret
        // the target triple as a Path and then just use the file stem as the
        // component for the directory name.
        if let Some(triple) = triple {
            path.push(Path::new(triple).file_stem().ok_or(human(format!("target was empty")))?);
        }
        path.push(dest);
        Layout::at(ws.config(), path)
    }

    pub fn at(config: &Config, root: Filesystem) -> CargoResult<Layout> {
        // For now we don't do any more finer-grained locking on the artifact
        // directory, so just lock the entire thing for the duration of this
        // compile.
        let lock = root.open_rw(".cargo-lock", config, "build directory")?;
        let root = root.into_path_unlocked();

        Ok(Layout {
            deps: root.join("deps"),
            native: root.join("native"),
            build: root.join("build"),
            incremental: root.join("incremental"),
            fingerprint: root.join(".fingerprint"),
            examples: root.join("examples"),
            root: root,
            _lock: lock,
        })
    }

    pub fn prepare(&mut self) -> io::Result<()> {
        if fs::metadata(&self.root).is_err() {
            fs::create_dir_all(&self.root)?;
        }

        mkdir(&self.deps)?;
        mkdir(&self.native)?;
        mkdir(&self.incremental)?;
        mkdir(&self.fingerprint)?;
        mkdir(&self.examples)?;
        mkdir(&self.build)?;

        return Ok(());

        fn mkdir(dir: &Path) -> io::Result<()> {
            if fs::metadata(&dir).is_err() {
                fs::create_dir(dir)?;
            }
            Ok(())
        }
    }

    pub fn dest(&self) -> &Path { &self.root }
    pub fn deps(&self) -> &Path { &self.deps }
    pub fn examples(&self) -> &Path { &self.examples }
    pub fn root(&self) -> &Path { &self.root }
    pub fn incremental(&self) -> &Path { &self.incremental }
    pub fn fingerprint(&self) -> &Path { &self.fingerprint }
    pub fn build(&self) -> &Path { &self.build }
}
