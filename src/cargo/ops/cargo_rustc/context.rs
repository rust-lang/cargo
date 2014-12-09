use std::collections::hash_map::{HashMap, Occupied, Vacant};
use std::str;
use std::sync::Arc;

use core::{SourceMap, Package, PackageId, PackageSet, Resolve, Target};
use util::{mod, CargoResult, ChainError, internal, Config, profile};
use util::human;

use super::{Kind, Compilation, BuildConfig};
use super::TargetConfig;
use super::layout::{Layout, LayoutProxy};
use super::custom_build::BuildState;

#[deriving(Show)]
pub enum Platform {
    Target,
    Plugin,
    PluginAndTarget,
}

pub struct Context<'a, 'b: 'a> {
    pub config: &'b Config<'b>,
    pub resolve: &'a Resolve,
    pub sources: &'a SourceMap<'b>,
    pub compilation: Compilation,
    pub build_state: Arc<BuildState>,

    env: &'a str,
    host: Layout,
    target: Option<Layout>,
    target_triple: String,
    host_dylib: Option<(String, String)>,
    host_exe: String,
    package_set: &'a PackageSet,
    target_dylib: Option<(String, String)>,
    target_exe: String,
    requirements: HashMap<(&'a PackageId, &'a str), Platform>,
    build_config: BuildConfig,
}

impl<'a, 'b: 'a> Context<'a, 'b> {
    pub fn new(env: &'a str, resolve: &'a Resolve, sources: &'a SourceMap<'b>,
               deps: &'a PackageSet, config: &'b Config<'b>,
               host: Layout, target: Option<Layout>,
               root_pkg: &Package,
               build_config: BuildConfig)
               -> CargoResult<Context<'a, 'b>> {
        let (target_dylib, target_exe) =
                try!(Context::filename_parts(config.target()));
        let (host_dylib, host_exe) = if config.target().is_none() {
            (target_dylib.clone(),
             target_exe.clone())
        } else {
            try!(Context::filename_parts(None))
        };
        let target_triple = config.target().map(|s| s.to_string());
        let target_triple = target_triple.unwrap_or(config.rustc_host().to_string());
        Ok(Context {
            target_triple: target_triple,
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
            host_exe: host_exe,
            requirements: HashMap::new(),
            compilation: Compilation::new(root_pkg),
            build_state: Arc::new(BuildState::new(build_config.clone(), deps)),
            build_config: build_config,
        })
    }

    /// Run `rustc` to discover the dylib prefix/suffix for the target
    /// specified as well as the exe suffix
    fn filename_parts(target: Option<&str>)
                      -> CargoResult<(Option<(String, String)>, String)> {
        let process = try!(util::process("rustc"))
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

        let exe_suffix = if error.contains("dropping unsupported crate type `bin`") {
            String::new()
        } else {
            lines.next().unwrap().trim()
                 .split('-').skip(1).next().unwrap().to_string()
        };
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
            self.build_requirements(pkg, target, Platform::Target);
        }

        self.compilation.extra_env.insert("NUM_JOBS".to_string(),
                                          Some(self.config.jobs().to_string()));
        self.compilation.root_output =
                self.layout(pkg, Kind::Target).proxy().dest().clone();
        self.compilation.deps_output =
                self.layout(pkg, Kind::Target).proxy().deps().clone();

        return Ok(());
    }

    fn build_requirements(&mut self, pkg: &'a Package, target: &'a Target,
                          req: Platform) {

        let req = if target.get_profile().is_for_host() {Platform::Plugin} else {req};
        match self.requirements.entry((pkg.get_package_id(), target.get_name())) {
            Occupied(mut entry) => match (*entry.get(), req) {
                (Platform::Plugin, Platform::Plugin) |
                (Platform::PluginAndTarget, Platform::Plugin) |
                (Platform::Target, Platform::Target) |
                (Platform::PluginAndTarget, Platform::Target) |
                (Platform::PluginAndTarget, Platform::PluginAndTarget) => return,
                _ => *entry.get_mut() = entry.get().combine(req),
            },
            Vacant(entry) => { entry.set(req); }
        };

        for &(pkg, dep) in self.dep_targets(pkg, target).iter() {
            self.build_requirements(pkg, dep, req);
        }

        match pkg.get_targets().iter().find(|t| t.get_profile().is_custom_build()) {
            Some(custom_build) => {
                self.build_requirements(pkg, custom_build, Platform::Plugin);
            }
            None => {}
        }
    }

    pub fn get_requirement(&self, pkg: &'a Package,
                           target: &'a Target) -> Platform {
        let default = if target.get_profile().is_for_host() {
            Platform::Plugin
        } else {
            Platform::Target
        };
        self.requirements.get(&(pkg.get_package_id(), target.get_name()))
            .map(|a| *a).unwrap_or(default)
    }

    /// Returns the appropriate directory layout for either a plugin or not.
    pub fn layout(&self, pkg: &Package, kind: Kind) -> LayoutProxy {
        let primary = pkg.get_package_id() == self.resolve.root();
        match kind {
            Kind::Host => LayoutProxy::new(&self.host, primary),
            Kind::Target =>  LayoutProxy::new(self.target.as_ref()
                                                .unwrap_or(&self.host),
                                            primary),
        }
    }

    /// Returns the appropriate output directory for the specified package and
    /// target.
    pub fn out_dir(&self, pkg: &Package, kind: Kind, target: &Target) -> Path {
        let out_dir = self.layout(pkg, kind);
        if target.get_profile().is_custom_build() {
            out_dir.build(pkg)
        } else if target.is_example() {
            out_dir.examples().clone()
        } else {
            out_dir.root().clone()
        }
    }

    /// Return the (prefix, suffix) pair for dynamic libraries.
    ///
    /// If `plugin` is true, the pair corresponds to the host platform,
    /// otherwise it corresponds to the target platform.
    fn dylib(&self, kind: Kind) -> CargoResult<(&str, &str)> {
        let (triple, pair) = if kind == Kind::Host {
            (self.config.rustc_host(), &self.host_dylib)
        } else {
            (self.target_triple.as_slice(), &self.target_dylib)
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
        if target.is_example() || target.is_bin() ||
           target.get_profile().is_test() {
            ret.push(format!("{}{}", stem,
                             if target.get_profile().is_for_host() {
                                 self.host_exe.as_slice()
                             } else {
                                 self.target_exe.as_slice()
                             }));
        } else {
            if target.is_dylib() {
                let plugin = target.get_profile().is_for_host();
                let kind = if plugin {Kind::Host} else {Kind::Target};
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
    pub fn dep_targets(&self, pkg: &Package, target: &Target)
                       -> Vec<(&'a Package, &'a Target)> {
        let deps = match self.resolve.deps(pkg.get_package_id()) {
            None => return vec!(),
            Some(deps) => deps,
        };
        deps.map(|id| self.get_package(id)).filter(|dep| {
            let pkg_dep = pkg.get_dependencies().iter().find(|d| {
                d.get_name() == dep.get_name()
            }).unwrap();

            // If this target is a build command, then we only want build
            // dependencies, otherwise we want everything *other than* build
            // dependencies.
            let is_correct_dep =
                target.get_profile().is_custom_build() == pkg_dep.is_build();

            // If this dependency is *not* a transitive dependency, then it
            // only applies to test targets
            let is_actual_dep = pkg_dep.is_transitive() ||
                                target.get_profile().is_test();

            is_correct_dep && is_actual_dep
        }).filter_map(|pkg| {
            pkg.get_targets().iter().find(|&t| self.is_relevant_target(t))
               .map(|t| (pkg, t))
        }).collect()
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

    /// Get the user-specified linker for a particular host or target
    pub fn linker(&self, kind: Kind) -> Option<&str> {
        self.target_config(kind).linker.as_ref().map(|s| s.as_slice())
    }

    /// Get the user-specified `ar` program for a particular host or target
    pub fn ar(&self, kind: Kind) -> Option<&str> {
        self.target_config(kind).ar.as_ref().map(|s| s.as_slice())
    }

    /// Get the target configuration for a particular host or target
    fn target_config(&self, kind: Kind) -> &TargetConfig {
        match kind {
            Kind::Host => &self.build_config.host,
            Kind::Target => &self.build_config.target,
        }
    }
}

impl Platform {
    fn combine(self, other: Platform) -> Platform {
        match (self, other) {
            (Platform::Target, Platform::Target) => Platform::Target,
            (Platform::Plugin, Platform::Plugin) => Platform::Plugin,
            _ => Platform::PluginAndTarget,
        }
    }

    pub fn includes(self, kind: Kind) -> bool {
        match (self, kind) {
            (Platform::PluginAndTarget, _) |
            (Platform::Target, Kind::Target) |
            (Platform::Plugin, Kind::Host) => true,
            _ => false,
        }
    }

    pub fn each_kind(self, f: |Kind|) {
        match self {
            Platform::Target => f(Kind::Target),
            Platform::Plugin => f(Kind::Host),
            Platform::PluginAndTarget => { f(Kind::Target); f(Kind::Host); }
        }
    }
}
