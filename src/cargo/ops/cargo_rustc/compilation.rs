use std::collections::HashMap;
use std::dynamic_lib::DynamicLibrary;
use std::os;
use semver::Version;

use core::{PackageId, Package};
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

    /// Extra environment variables that were passed to compilations and should
    /// be passed to future invocations of programs.
    pub extra_env: HashMap<String, Option<String>>,

    /// Top-level package that was compiled
    pub package: Package,
}

impl Compilation {
    pub fn new(pkg: &Package) -> Compilation {
        Compilation {
            libraries: HashMap::new(),
            native_dirs: HashMap::new(),
            root_output: Path::new("/"),
            deps_output: Path::new("/"),
            tests: Vec::new(),
            binaries: Vec::new(),
            extra_env: HashMap::new(),
            package: pkg.clone(),
        }
    }

    /// Prepares a new process with an appropriate environment to run against
    /// the artifacts produced by the build process.
    ///
    /// The package argument is also used to configure environment variables as
    /// well as the working directory of the child process.
    pub fn process<T: ToCStr>(&self, cmd: T, pkg: &Package)
                              -> util::ProcessBuilder {
        let mut search_path = DynamicLibrary::search_path();
        for dir in self.native_dirs.values() {
            search_path.push(dir.clone());
        }
        search_path.push(self.root_output.clone());
        search_path.push(self.deps_output.clone());
        let search_path = os::join_paths(search_path.as_slice()).unwrap();
        let mut cmd = util::process(cmd).env(DynamicLibrary::envvar(),
                                             Some(search_path.as_slice()));
        for (k, v) in self.extra_env.iter() {
            cmd = cmd.env(k.as_slice(), v.as_ref().map(|s| s.as_slice()));
        }

        cmd.env("CARGO_MANIFEST_DIR", Some(pkg.get_manifest_path().dir_path()))
           .env("CARGO_PKG_VERSION_MAJOR",
                Some(pkg.get_version().major.to_string()))
           .env("CARGO_PKG_VERSION_MINOR",
                Some(pkg.get_version().minor.to_string()))
           .env("CARGO_PKG_VERSION_PATCH",
                Some(pkg.get_version().patch.to_string()))
           .env("CARGO_PKG_VERSION_PRE",
                pre_version_component(pkg.get_version()))
           .cwd(pkg.get_root())
    }
}

fn pre_version_component(v: &Version) -> Option<String> {
    if v.pre.is_empty() {
        return None;
    }

    let mut ret = String::new();

    for (i, x) in v.pre.iter().enumerate() {
        if i != 0 { ret.push('.') };
        ret.push_str(x.to_string().as_slice());
    }

    Some(ret)
}
