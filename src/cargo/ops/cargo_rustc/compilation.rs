use std::collections::HashMap;
use std::dynamic_lib::DynamicLibrary;
use std::os;

use core::PackageId;
use util;

/// A structure returning the result of a compilation.
pub struct Compilation {
    /// All libraries which were built for a package.
    ///
    /// This is currently used for passing --extern flags to rustdoc tests later
    /// on.
    pub libraries: HashMap<PackageId, Vec<Path>>,

    /// An array of all tests created during this compilation.
    pub tests: Vec<Path>,

    /// An array of all binaries created.
    pub binaries: Vec<Path>,

    /// All directires for the output of native build commands.
    ///
    /// This is currently used to drive some entries which are added to the
    /// LD_LIBRARY_PATH as appropriate.
    pub native_dirs: HashMap<PackageId, Path>,

    /// Root output directory (for the local package's artifacts)
    pub root_output: Path,

    /// Output directory for rust dependencies
    pub deps_output: Path,
}

impl Compilation {
    pub fn new() -> Compilation {
        Compilation {
            libraries: HashMap::new(),
            native_dirs: HashMap::new(),
            root_output: Path::new("/"),
            deps_output: Path::new("/"),
            tests: Vec::new(),
            binaries: Vec::new(),
        }
    }

    /// Prepares a new process with an appropriate environment to run against
    /// the artifacts produced by the build process.
    pub fn process<T: ToCStr>(&self, cmd: T) -> util::ProcessBuilder {
        let mut search_path = DynamicLibrary::search_path();
        for dir in self.native_dirs.values() {
            search_path.push(dir.clone());
        }
        search_path.push(self.root_output.clone());
        search_path.push(self.deps_output.clone());
        let search_path = os::join_paths(search_path.as_slice()).unwrap();
        util::process(cmd).env(DynamicLibrary::envvar(),
                               Some(search_path.as_slice()))
    }
}
