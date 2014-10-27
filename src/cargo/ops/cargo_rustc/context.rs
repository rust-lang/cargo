use std::collections::{HashMap, HashSet};
use std::collections::hashmap::{Occupied, Vacant};
use std::str;

use core::{SourceMap, Package, PackageId, PackageSet, Resolve, Target};
use util::{mod, CargoResult, ChainError, internal, Config, profile, Require};
use util::human;

use super::{Kind, KindPlugin, KindTarget, Compilation};
use super::layout::{Layout, LayoutProxy};

#[deriving(Show)]
pub enum PlatformRequirement {
    PlatformTarget,
    PlatformPlugin,
    PlatformPluginAndTarget,
}

pub struct Context<'a, 'b> {
    pub rustc_version: String,
    pub config: &'b mut Config<'b>,
    pub resolve: &'a Resolve,
    pub sources: &'a SourceMap<'b>,
    pub compilation: Compilation,

    env: &'a str,
    host: Layout,
    target: Option<Layout>,
    target_triple: String,
    host_triple: String,
    host_dylib: Option<(String, String)>,
    package_set: &'a PackageSet,
    target_dylib: Option<(String, String)>,
    target_exe: String,
    requirements: HashMap<(&'a PackageId, &'a str), PlatformRequirement>,
}

impl<'a, 'b> Context<'a, 'b> {
    pub fn new(env: &'a str, resolve: &'a Resolve, sources: &'a SourceMap,
               deps: &'a PackageSet, config: &'b mut Config<'b>,
               host: Layout, target: Option<Layout>,
               root_pkg: &Package)
               -> CargoResult<Context<'a, 'b>> {
        let (target_dylib, target_exe) =
                try!(Context::filename_parts(config.target()));
        let host_dylib = if config.target().is_none() {
            target_dylib.clone()
        } else {
            let (dylib, _) = try!(Context::filename_parts(None));
            dylib
        };
        let (rustc_version, rustc_host) = try!(Context::rustc_version());
        let target_triple = config.target().map(|s| s.to_string());
        let target_triple = target_triple.unwrap_or(rustc_host.clone());
        Ok(Context {
            rustc_version: rustc_version,
            target_triple: target_triple,
            host_triple: rustc_host,
            env: env,
            host: host,
            target: target,
            resolve: resolve,
            sources: sources,
            package_set: deps,
            config: config,
            target_dylib: target_dylib,
            target_exe: target_exe,
            host_dylib: host_dylib,
            requirements: HashMap::new(),
            compilation: Compilation::new(root_pkg),
        })
    }

    /// Run `rustc` to figure out what its current version string is.
    ///
    /// The second element of the tuple returned is the target triple that rustc
    /// is a host for.
    fn rustc_version() -> CargoResult<(String, String)> {
        let output = try!(util::process("rustc").arg("-v").arg("verbose")
                               .exec_with_output());
        let output = try!(String::from_utf8(output.output).map_err(|_| {
            internal("rustc -v didn't return utf8 output")
        }));
        let triple = {
            let triple = output.as_slice().lines().filter(|l| {
                l.starts_with("host: ")
            }).map(|l| l.slice_from(6)).next();
            let triple = try!(triple.require(|| {
                internal("rustc -v didn't have a line for `host:`")
            }));
            triple.to_string()
        };
        Ok((output, triple))
    }

    /// Run `rustc` to discover the dylib prefix/suffix for the target
    /// specified as well as the exe suffix
    fn filename_parts(target: Option<&str>)
                      -> CargoResult<(Option<(String, String)>, String)> {
        let process = util::process("rustc")
                           .arg("-")
                           .arg("--crate-name").arg("-")
                           .arg("--crate-type").arg("dylib")
                           .arg("--crate-type").arg("bin")
                           .arg("--print-file-name");
        let process = match target {
            Some(s) => process.arg("--target").arg(s),
            None => process,
        };
        let output = try!(process.exec_with_output());

        let error = str::from_utf8(output.error.as_slice()).unwrap();
        let output = str::from_utf8(output.output.as_slice()).unwrap();
        let mut lines = output.lines();
        let dylib = if error.contains("dropping unsupported crate type `dylib`") {
            None
        } else {
            let dylib_parts: Vec<&str> = lines.next().unwrap().trim()
                                              .split('-').collect();
            assert!(dylib_parts.len() == 2,
                    "rustc --print-file-name output has changed");
            Some((dylib_parts[0].to_string(), dylib_parts[1].to_string()))
        };

        let exe_suffix = lines.next().unwrap().trim()
                              .split('-').skip(1).next().unwrap().to_string();
        Ok((dylib, exe_suffix.to_string()))
    }

    /// Prepare this context, ensuring that all filesystem directories are in
    /// place.
    pub fn prepare(&mut self, pkg: &'a Package) -> CargoResult<()> {
        let _p = profile::start("preparing layout");

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
            self.build_requirements(pkg, target, PlatformTarget,
                                    &mut HashSet::new());
        }

        self.compilation.extra_env.insert("NUM_JOBS".to_string(),
                                          Some(self.config.jobs().to_string()));
        self.compilation.root_output =
                self.layout(pkg, KindTarget).proxy().dest().clone();
        self.compilation.deps_output =
                self.layout(pkg, KindTarget).proxy().deps().clone();

        return Ok(());
    }

    fn build_requirements(&mut self, pkg: &'a Package, target: &'a Target,
                          req: PlatformRequirement,
                          visiting: &mut HashSet<&'a PackageId>) {
        if !visiting.insert(pkg.get_package_id()) { return }

        let key = (pkg.get_package_id(), target.get_name());
        let req = if target.get_profile().is_plugin() {PlatformPlugin} else {req};
        match self.requirements.entry(key) {
            Occupied(mut entry) => { *entry.get_mut() = entry.get().combine(req); }
            Vacant(entry) => { entry.set(req); }
        };

        for &(pkg, dep) in self.dep_targets(pkg).iter() {
            self.build_requirements(pkg, dep, req, visiting);
        }

        visiting.remove(&pkg.get_package_id());
    }

    pub fn get_requirement(&self, pkg: &'a Package,
                           target: &'a Target) -> PlatformRequirement {
        self.requirements.find(&(pkg.get_package_id(), target.get_name()))
            .map(|a| *a).unwrap_or(PlatformTarget)
    }

    /// Returns the appropriate directory layout for either a plugin or not.
    pub fn layout(&self, pkg: &Package, kind: Kind) -> LayoutProxy {
        let primary = pkg.get_package_id() == self.resolve.root();
        match kind {
            KindPlugin => LayoutProxy::new(&self.host, primary),
            KindTarget =>  LayoutProxy::new(self.target.as_ref()
                                                .unwrap_or(&self.host),
                                            primary)
        }
    }

    /// Return the (prefix, suffix) pair for dynamic libraries.
    ///
    /// If `plugin` is true, the pair corresponds to the host platform,
    /// otherwise it corresponds to the target platform.
    fn dylib(&self, kind: Kind) -> CargoResult<(&str, &str)> {
        let (triple, pair) = if kind == KindPlugin {
            (&self.host_triple, &self.host_dylib)
        } else {
            (&self.target_triple, &self.target_dylib)
        };
        match *pair {
            None => return Err(human(format!("dylib outputs are not supported \
                                              for {}", triple))),
            Some((ref s1, ref s2)) => Ok((s1.as_slice(), s2.as_slice())),
        }
    }

    /// Return the target triple which this context is targeting.
    pub fn target_triple(&self) -> &str {
        self.target_triple.as_slice()
    }

    /// Return the exact filename of the target.
    pub fn target_filenames(&self, target: &Target) -> CargoResult<Vec<String>> {
        let stem = target.file_stem();

        let mut ret = Vec::new();
        if target.is_example() {
            ret.push(format!("examples/{}{}", stem, self.target_exe));
        } else if target.is_bin() || target.get_profile().is_test() {
            ret.push(format!("{}{}", stem, self.target_exe));
        } else {
            if target.is_dylib() {
                let plugin = target.get_profile().is_plugin();
                let kind = if plugin {KindPlugin} else {KindTarget};
                let (prefix, suffix) = try!(self.dylib(kind));
                ret.push(format!("{}{}{}", prefix, stem, suffix));
            }
            if target.is_rlib() {
                ret.push(format!("lib{}.rlib", stem));
            }
            if target.is_staticlib() {
                ret.push(format!("lib{}.a", stem));
            }
        }
        assert!(ret.len() > 0);
        return Ok(ret);
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

    pub fn env(&self) -> &str {
        // The "doc-all" environment just means to document everything (see
        // below), but we want to canonicalize that the the "doc" profile
        // environment, so do that here.
        if self.env == "doc-all" {"doc"} else {self.env}
    }

    pub fn is_relevant_target(&self, target: &Target) -> bool {
        target.is_lib() && match self.env {
            "doc" | "test" => target.get_profile().is_compile(),
            // doc-all == document everything, so look for doc targets and
            //            compile targets in dependencies
            "doc-all" => target.get_profile().is_compile() ||
                         (target.get_profile().get_env() == "doc" &&
                          target.get_profile().is_doc()),
            _ => target.get_profile().get_env() == self.env &&
                 !target.get_profile().is_test(),
        }
    }
}

impl PlatformRequirement {
    fn combine(self, other: PlatformRequirement) -> PlatformRequirement {
        match (self, other) {
            (PlatformTarget, PlatformTarget) => PlatformTarget,
            (PlatformPlugin, PlatformPlugin) => PlatformPlugin,
            _ => PlatformPluginAndTarget,
        }
    }
}
