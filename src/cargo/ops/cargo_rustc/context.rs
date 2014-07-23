use std::collections::{HashMap, HashSet};
use std::os;
use std::str;

use core::{Package, PackageId, PackageSet, Resolve, Target};
use util;
use util::{CargoResult, ChainError, internal, Config};

use super::layout::{Layout, LayoutProxy};

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

    env: &'a str,
    host: Layout,
    target: Option<Layout>,
    host_dylib: (String, String),
    package_set: &'a PackageSet,
    target_dylib: (String, String),
    target_exe: String,
    requirements: HashMap<(&'a PackageId, &'a str), PlatformRequirement>,
}

impl<'a, 'b> Context<'a, 'b> {
    pub fn new(env: &'a str, resolve: &'a Resolve, deps: &'a PackageSet,
               config: &'b mut Config<'b>,
               host: Layout, target: Option<Layout>)
               -> CargoResult<Context<'a, 'b>> {
        let (target_dylib, target_exe) =
                try!(Context::filename_parts(config.target()));
        let host_dylib = if config.target().is_none() {
            target_dylib.clone()
        } else {
            let (dylib, _) = try!(Context::filename_parts(None));
            dylib
        };
        Ok(Context {
            rustc_version: try!(Context::rustc_version()),
            env: env,
            host: host,
            target: target,
            primary: false,
            resolve: resolve,
            package_set: deps,
            config: config,
            target_dylib: target_dylib,
            target_exe: target_exe,
            host_dylib: host_dylib,
            requirements: HashMap::new(),
        })
    }

    /// Run `rustc` to figure out what its current version string is
    fn rustc_version() -> CargoResult<String> {
        let output = try!(util::process("rustc").arg("-v").arg("verbose")
                               .exec_with_output());
        Ok(String::from_utf8(output.output).unwrap())
    }

    /// Run `rustc` to discover the dylib prefix/suffix for the target
    /// specified as well as the exe suffix
    fn filename_parts(target: Option<&str>)
                      -> CargoResult<((String, String), String)> {
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
        let dylib_parts: Vec<&str> = output.trim().split('-').collect();
        assert!(dylib_parts.len() == 2,
                "rustc --print-file-name output has changed");
        let exe_suffix = match target {
            None => os::consts::EXE_SUFFIX,
            Some(s) if s.contains("win32") || s.contains("windows") => ".exe",
            Some(_) => "",
        };

        Ok(((dylib_parts[0].to_string(), dylib_parts[1].to_string()),
            exe_suffix.to_string()))
    }

    /// Prepare this context, ensuring that all filesystem directories are in
    /// place.
    pub fn prepare(&mut self, pkg: &'a Package) -> CargoResult<()> {
        try!(self.host.prepare().chain_error(|| {
            internal(format!("couldn't prepare build directories for `{}`",
                             pkg.get_name()))
        }));
        match self.target {
            Some(ref mut target) => {
                try!(target.prepare().chain_error(|| {
                    internal(format!("couldn't prepare build directories \
                                      for `{}`", pkg.get_name()))
                }));
            }
            None => {}
        }

        let targets = pkg.get_targets().iter();
        for target in targets.filter(|t| t.get_profile().is_compile()) {
            self.build_requirements(pkg, target, Target, &mut HashSet::new());
        }

        Ok(())
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

    /// Returns the appropriate directory layout for either a plugin or not.
    pub fn layout(&self, plugin: bool) -> LayoutProxy {
        if plugin {
            LayoutProxy::new(&self.host, self.primary)
        } else {
            LayoutProxy::new(self.target.as_ref().unwrap_or(&self.host),
                             self.primary)
        }
    }

    /// Return the (prefix, suffix) pair for dynamic libraries.
    ///
    /// If `plugin` is true, the pair corresponds to the host platform,
    /// otherwise it corresponds to the target platform.
    fn dylib(&self, plugin: bool) -> (&str, &str) {
        let pair = if plugin {&self.host_dylib} else {&self.target_dylib};
        (pair.ref0().as_slice(), pair.ref1().as_slice())
    }

    /// Return the exact filename of the target.
    pub fn target_filenames(&self, target: &Target) -> Vec<String> {
        let stem = target.file_stem();

        let mut ret = Vec::new();
        if target.is_bin() || target.get_profile().is_test() {
            ret.push(format!("{}{}", stem, self.target_exe));
        } else {
            if target.is_dylib() {
                let (prefix, suffix) = self.dylib(target.get_profile().is_plugin());
                ret.push(format!("{}{}{}", prefix, stem, suffix));
            }
            if target.is_rlib() {
                ret.push(format!("lib{}.rlib", stem));
            }
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
        deps.map(|pkg_id| self.get_package(pkg_id))
        .filter_map(|pkg| {
            pkg.get_targets().iter().find(|&t| self.is_relevant_target(t))
               .map(|t| (pkg, t))
        })
        .collect()
    }

    /// Gets a package for the given package id.
    pub fn get_package(&self, id: &PackageId) -> &'a Package {
        self.package_set.iter()
            .find(|pkg| id == pkg.get_package_id())
            .expect("Should have found package")
    }

    pub fn is_relevant_target(&self, target: &Target) -> bool {
        target.is_lib() && match self.env {
            "test" => target.get_profile().is_compile(),
            _ => target.get_profile().get_env() == self.env,
        }
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
