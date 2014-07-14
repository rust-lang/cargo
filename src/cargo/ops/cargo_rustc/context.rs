use std::io::IoError;
use std::io;
use std::str;
use std::collections::{HashMap, HashSet};

use core::{Package, PackageId, PackageSet, Resolve, Target};
use util;
use util::{CargoResult, ChainError, internal, Config};

#[deriving(Show)]
pub enum PlatformRequirement {
    Target,
    Plugin,
    PluginAndTarget,
}

pub struct Context<'a, 'b> {
    pub primary: bool,
    pub rustc_version: String,
    pub config: &'b mut Config<'b>,
    pub resolve: &'a Resolve,

    dest: Path,
    host_dest: Path,
    deps_dir: Path,
    host_deps_dir: Path,
    host_dylib: (String, String),
    package_set: &'a PackageSet,
    target_dylib: (String, String),
    requirements: HashMap<(&'a PackageId, &'a str), PlatformRequirement>,
}

impl<'a, 'b> Context<'a, 'b> {
    pub fn new(resolve: &'a Resolve, deps: &'a PackageSet,
               config: &'b mut Config<'b>,
               dest: Path, deps_dir: Path,
               host_dest: Path, host_deps_dir: Path)
               -> CargoResult<Context<'a, 'b>> {
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
            requirements: HashMap::new(),
            host_dest: host_dest,
            host_deps_dir: host_deps_dir,
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
    pub fn prepare(&mut self, pkg: &'a Package) -> CargoResult<()> {
        debug!("creating target dir; path={}", self.dest.display());

        try!(self.mk_target(&self.dest).chain_error(||
            internal(format!("Couldn't create the target directory for {} at {}",
                     pkg.get_name(), self.dest.display()))));
        try!(self.mk_target(&self.host_dest).chain_error(||
            internal(format!("Couldn't create the host directory for {} at {}",
                     pkg.get_name(), self.dest.display()))));

        try!(self.mk_target(&self.deps_dir).chain_error(||
            internal(format!("Couldn't create the directory for dependencies for {} at {}",
                     pkg.get_name(), self.deps_dir.display()))));

        try!(self.mk_target(&self.host_deps_dir).chain_error(||
            internal(format!("Couldn't create the directory for dependencies for {} at {}",
                     pkg.get_name(), self.deps_dir.display()))));

        let targets = pkg.get_targets().iter();
        for target in targets.filter(|t| t.get_profile().is_compile()) {
            self.build_requirements(pkg, target, Target, &mut HashSet::new());
        }

        Ok(())
    }

    fn mk_target(&self, target: &Path) -> Result<(), IoError> {
        io::fs::mkdir_recursive(target, io::UserRWX)
    }

    fn build_requirements(&mut self, pkg: &'a Package, target: &'a Target,
                          req: PlatformRequirement,
                          visiting: &mut HashSet<&'a PackageId>) {
        if !visiting.insert(pkg.get_package_id()) { return }

        let key = (pkg.get_package_id(), target.get_name());
        let req = if target.get_profile().is_plugin() {Plugin} else {req};
        self.requirements.insert_or_update_with(key, req, |_, v| {
            *v = v.combine(req);
        });

        for &(pkg, dep) in self.dep_targets(pkg).iter() {
            self.build_requirements(pkg, dep, req, visiting);
        }

        visiting.remove(&pkg.get_package_id());
    }

    pub fn get_requirement(&self, pkg: &'a Package,
                           target: &'a Target) -> PlatformRequirement {
        self.requirements.find(&(pkg.get_package_id(), target.get_name()))
            .map(|a| *a).unwrap_or(Target)
    }

    /// Switch this context over to being the primary compilation unit,
    /// affecting the output of `dest()` and such.
    pub fn primary(&mut self) {
        self.primary = true;
    }

    /// Return the destination directory for output.
    pub fn dest<'a>(&'a self, plugin: bool) -> &'a Path {
        if self.primary {
            if plugin {&self.host_dest} else {&self.dest}
        } else {
            self.deps_dir(plugin)
        }
    }

    /// Return the destination directory for dependencies.
    pub fn deps_dir<'a>(&'a self, plugin: bool) -> &'a Path {
        if plugin {&self.host_deps_dir} else {&self.deps_dir}
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
    pub fn target_filenames(&self, target: &Target) -> Vec<String> {
        let stem = target.file_stem();

        let mut ret = Vec::new();
        if target.is_dylib() {
            let (prefix, suffix) = self.dylib(target.get_profile().is_plugin());
            ret.push(format!("{}{}{}", prefix, stem, suffix));
        }
        if target.is_rlib() {
            ret.push(format!("lib{}.rlib", stem));
        }
        assert!(ret.len() > 0);
        return ret;
    }

    /// For a package, return all targets which are registered as dependencies
    /// for that package.
    pub fn dep_targets(&self, pkg: &Package) -> Vec<(&'a Package, &'a Target)> {
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
            }).map(|t| (pkg, t))
        })
        .collect()
    }
}

impl PlatformRequirement {
    fn combine(self, other: PlatformRequirement) -> PlatformRequirement {
        match (self, other) {
            (Target, Target) => Target,
            (Plugin, Plugin) => Plugin,
            _ => PluginAndTarget,
        }
    }
}
