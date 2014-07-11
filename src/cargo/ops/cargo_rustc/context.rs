use std::io::IoError;
use std::io;
use std::str;

use core::{Package, PackageSet, Resolve, Target};
use util;
use util::{CargoResult, ChainError, internal, Config};

pub struct Context<'a, 'b> {
    pub deps_dir: Path,
    pub primary: bool,
    pub rustc_version: String,
    pub config: &'b mut Config<'b>,

    dest: Path,
    host_dylib: (String, String),
    package_set: &'a PackageSet,
    resolve: &'a Resolve,
    target_dylib: (String, String),
}

impl<'a, 'b> Context<'a, 'b> {
    pub fn new(resolve: &'a Resolve, deps: &'a PackageSet,
               config: &'b mut Config<'b>,
               dest: Path, deps_dir: Path) -> CargoResult<Context<'a, 'b>> {
        let target_dylib = try!(Context::dylib_parts(config.target()));
        let host_dylib = if config.target().is_none() {
            target_dylib.clone()
        } else {
            try!(Context::dylib_parts(None))
        };
        Ok(Context {
            rustc_version: try!(Context::rustc_version()),
            dest: dest,
            deps_dir: deps_dir,
            primary: false,
            resolve: resolve,
            package_set: deps,
            config: config,
            target_dylib: target_dylib,
            host_dylib: host_dylib,
        })
    }

    /// Run `rustc` to figure out what its current version string is
    fn rustc_version() -> CargoResult<String> {
        let output = try!(util::process("rustc").arg("-v").arg("verbose")
                               .exec_with_output());
        Ok(String::from_utf8(output.output).unwrap())
    }

    /// Run `rustc` to discover the dylib prefix/suffix for the target
    /// specified.
    fn dylib_parts(target: Option<&str>) -> CargoResult<(String, String)> {
        let process = util::process("rustc")
                           .arg("-")
                           .arg("--crate-name").arg("-")
                           .arg("--crate-type").arg("dylib")
                           .arg("--print-file-name");
        let process = match target {
            Some(s) => process.arg("--target").arg(s),
            None => process,
        };
        let output = try!(process.exec_with_output());

        let output = str::from_utf8(output.output.as_slice()).unwrap();
        let parts: Vec<&str> = output.trim().split('-').collect();
        assert!(parts.len() == 2, "rustc --print-file-name output has changed");

        Ok((parts.get(0).to_string(), parts.get(1).to_string()))
    }

    /// Prepare this context, ensuring that all filesystem directories are in
    /// place.
    pub fn prepare(&self, pkg: &Package) -> CargoResult<()> {
        debug!("creating target dir; path={}", self.dest.display());

        try!(self.mk_target(&self.dest).chain_error(||
            internal(format!("Couldn't create the target directory for {} at {}",
                     pkg.get_name(), self.dest.display()))));

        try!(self.mk_target(&self.deps_dir).chain_error(||
            internal(format!("Couldn't create the directory for dependencies for {} at {}",
                     pkg.get_name(), self.deps_dir.display()))));

        Ok(())
    }

    fn mk_target(&self, target: &Path) -> Result<(), IoError> {
        io::fs::mkdir_recursive(target, io::UserRWX)
    }

    /// Switch this context over to being the primary compilation unit,
    /// affecting the output of `dest()` and such.
    pub fn primary(&mut self) {
        self.primary = true;
    }

    /// Return the destination directory for output.
    pub fn dest<'a>(&'a self) -> &'a Path {
        if self.primary {&self.dest} else {&self.deps_dir}
    }

    /// Return the (prefix, suffix) pair for dynamic libraries.
    ///
    /// If `plugin` is true, the pair corresponds to the host platform,
    /// otherwise it corresponds to the target platform.
    fn dylib<'a>(&'a self, plugin: bool) -> (&'a str, &'a str) {
        let pair = if plugin {&self.host_dylib} else {&self.target_dylib};
        (pair.ref0().as_slice(), pair.ref1().as_slice())
    }

    /// Return the exact filename of the target.
    pub fn target_filename(&self, target: &Target) -> String {
        let stem = target.file_stem();

        if target.is_dylib() {
            let (prefix, suffix) = self.dylib(target.get_profile().is_plugin());
            format!("{}{}{}", prefix, stem, suffix)
        } else if target.is_rlib() {
            format!("lib{}.rlib", stem)
        } else {
            unreachable!()
        }
    }

    /// For a package, return all targets which are registered as dependencies
    /// for that package.
    pub fn dep_targets(&self, pkg: &Package) -> Vec<Target> {
        let deps = match self.resolve.deps(pkg.get_package_id()) {
            None => return vec!(),
            Some(deps) => deps,
        };
        deps.map(|pkg_id| {
            self.package_set.iter()
                .find(|pkg| pkg_id == pkg.get_package_id())
                .expect("Should have found package")
        })
        .filter_map(|pkg| {
            pkg.get_targets().iter().find(|&t| {
                t.is_lib() && t.get_profile().is_compile()
            })
        })
        .map(|t| t.clone())
        .collect()
    }
}
