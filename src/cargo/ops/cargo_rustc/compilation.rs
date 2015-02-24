use std::collections::{HashMap, HashSet};
use std::dynamic_lib::DynamicLibrary;
use std::ffi::CString;
use std::old_path::BytesContainer;
use semver::Version;

use core::{PackageId, Package};
use util::{self, CargoResult};

use super::{CommandType, CommandPrototype};

/// A structure returning the result of a compilation.
pub struct Compilation {
    /// All libraries which were built for a package.
    ///
    /// This is currently used for passing --extern flags to rustdoc tests later
    /// on.
    pub libraries: HashMap<PackageId, Vec<Path>>,

    /// An array of all tests created during this compilation.
    pub tests: Vec<(String, Path)>,

    /// An array of all binaries created.
    pub binaries: Vec<Path>,

    /// All directires for the output of native build commands.
    ///
    /// This is currently used to drive some entries which are added to the
    /// LD_LIBRARY_PATH as appropriate.
    // TODO: deprecated, remove
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

    /// Features enabled during this compilation.
    pub features: HashSet<String>,
}

impl Compilation {
    pub fn new(pkg: &Package) -> Compilation {
        Compilation {
            libraries: HashMap::new(),
            native_dirs: HashMap::new(),  // TODO: deprecated, remove
            root_output: Path::new("/"),
            deps_output: Path::new("/"),
            tests: Vec::new(),
            binaries: Vec::new(),
            extra_env: HashMap::new(),
            package: pkg.clone(),
            features: HashSet::new(),
        }
    }

    /// See `process`.
    pub fn rustc_process(&self, pkg: &Package) -> CargoResult<CommandPrototype> {
        self.process(CommandType::Rustc, pkg)
    }

    /// See `process`.
    pub fn rustdoc_process(&self, pkg: &Package) -> CargoResult<CommandPrototype> {
        self.process(CommandType::Rustdoc, pkg)
    }

    /// See `process`.
    pub fn target_process<T: BytesContainer>(&self, cmd: T, pkg: &Package)
                                     -> CargoResult<CommandPrototype> {
        let cmd = try!(CString::new(cmd.container_as_bytes()));
        self.process(CommandType::Target(cmd), pkg)
    }

    /// See `process`.
    pub fn host_process<T: BytesContainer>(&self, cmd: T, pkg: &Package)
                                   -> CargoResult<CommandPrototype> {
        let cmd = try!(CString::new(cmd.container_as_bytes()));
        self.process(CommandType::Host(cmd), pkg)
    }

    /// Prepares a new process with an appropriate environment to run against
    /// the artifacts produced by the build process.
    ///
    /// The package argument is also used to configure environment variables as
    /// well as the working directory of the child process.
    pub fn process(&self, cmd: CommandType, pkg: &Package)
                   -> CargoResult<CommandPrototype> {
        let mut search_path = DynamicLibrary::search_path();
        for dir in self.native_dirs.values() {
            search_path.push(dir.clone());
        }
        search_path.push(self.root_output.clone());
        search_path.push(self.deps_output.clone());
        let search_path = try!(util::join_paths(&search_path,
                                                DynamicLibrary::envvar()));
        let mut cmd = try!(CommandPrototype::new(cmd)).env(
            DynamicLibrary::envvar(), Some(&search_path));
        for (k, v) in self.extra_env.iter() {
            cmd = cmd.env(k, v.as_ref());
        }

        Ok(cmd.env("CARGO_MANIFEST_DIR", Some(pkg.manifest_path().dir_path()))
              .env("CARGO_PKG_VERSION_MAJOR",
                   Some(pkg.version().major.to_string()))
              .env("CARGO_PKG_VERSION_MINOR",
                  Some(pkg.version().minor.to_string()))
              .env("CARGO_PKG_VERSION_PATCH",
                   Some(pkg.version().patch.to_string()))
              .env("CARGO_PKG_VERSION_PRE",
                   pre_version_component(pkg.version()))
              .env("CARGO_PKG_VERSION",
                   Some(pkg.version().to_string()))
              .cwd(pkg.root()))
    }
}

fn pre_version_component(v: &Version) -> Option<String> {
    if v.pre.is_empty() {
        return None;
    }

    let mut ret = String::new();

    for (i, x) in v.pre.iter().enumerate() {
        if i != 0 { ret.push('.') };
        ret.push_str(&x.to_string());
    }

    Some(ret)
}
