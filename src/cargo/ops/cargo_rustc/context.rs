use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::hash_map::HashMap;
use std::str;
use std::sync::Arc;
use std::path::PathBuf;

use regex::Regex;

use core::{SourceMap, Package, PackageId, PackageSet, Resolve, Target, Profile};
use util::{self, CargoResult, ChainError, internal, Config, profile};
use util::human;

use super::TargetConfig;
use super::custom_build::BuildState;
use super::fingerprint::Fingerprint;
use super::layout::{Layout, LayoutProxy};
use super::{Kind, Compilation, BuildConfig};
use super::{ProcessEngine, ExecEngine};

#[derive(Debug, Copy)]
pub enum Platform {
    Target,
    Plugin,
    PluginAndTarget,
}

pub struct Context<'a, 'b: 'a> {
    pub config: &'a Config<'b>,
    pub resolve: &'a Resolve,
    pub sources: &'a SourceMap<'a>,
    pub compilation: Compilation,
    pub build_state: Arc<BuildState>,
    pub exec_engine: Arc<Box<ExecEngine>>,
    pub fingerprints: HashMap<(&'a PackageId, &'a Target, Kind), Fingerprint>,

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
    pub fn new(env: &'a str,
               resolve: &'a Resolve,
               sources: &'a SourceMap<'a>,
               deps: &'a PackageSet,
               config: &'a Config<'b>,
               host: Layout,
               target_layout: Option<Layout>,
               root_pkg: &Package,
               build_config: BuildConfig) -> CargoResult<Context<'a, 'b>> {
        let target = build_config.requested_target.clone();
        let target = target.as_ref().map(|s| &s[..]);
        let (target_dylib, target_exe) = try!(Context::filename_parts(target));
        let (host_dylib, host_exe) = if build_config.requested_target.is_none() {
            (target_dylib.clone(), target_exe.clone())
        } else {
            try!(Context::filename_parts(None))
        };
        let target_triple = target.unwrap_or(config.rustc_host()).to_string();
        Ok(Context {
            target_triple: target_triple,
            env: env,
            host: host,
            target: target_layout,
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
            exec_engine: Arc::new(Box::new(ProcessEngine) as Box<ExecEngine>),
            fingerprints: HashMap::new(),
        })
    }

    /// Run `rustc` to discover the dylib prefix/suffix for the target
    /// specified as well as the exe suffix
    fn filename_parts(target: Option<&str>)
                      -> CargoResult<(Option<(String, String)>, String)> {
        let mut process = try!(util::process("rustc"));
        process.arg("-")
               .arg("--crate-name").arg("_")
               .arg("--crate-type").arg("dylib")
               .arg("--crate-type").arg("bin")
               .arg("--print=file-names")
               .env_remove("RUST_LOG");
        if let Some(s) = target {
            process.arg("--target").arg(s);
        };
        let output = try!(process.exec_with_output());

        let error = str::from_utf8(&output.stderr).unwrap();
        let output = str::from_utf8(&output.stdout).unwrap();
        let mut lines = output.lines();
        let nodylib = Regex::new("unsupported crate type.*dylib").unwrap();
        let nobin = Regex::new("unsupported crate type.*bin").unwrap();
        let dylib = if nodylib.is_match(error) {
            None
        } else {
            let dylib_parts: Vec<&str> = lines.next().unwrap().trim()
                                              .split('_').collect();
            assert!(dylib_parts.len() == 2,
                    "rustc --print-file-name output has changed");
            Some((dylib_parts[0].to_string(), dylib_parts[1].to_string()))
        };

        let exe_suffix = if nobin.is_match(error) {
            String::new()
        } else {
            lines.next().unwrap().trim()
                 .split('_').skip(1).next().unwrap().to_string()
        };
        Ok((dylib, exe_suffix.to_string()))
    }

    /// Prepare this context, ensuring that all filesystem directories are in
    /// place.
    pub fn prepare(&mut self, pkg: &'a Package) -> CargoResult<()> {
        let _p = profile::start("preparing layout");

        try!(self.host.prepare().chain_error(|| {
            internal(format!("couldn't prepare build directories for `{}`",
                             pkg.name()))
        }));
        match self.target {
            Some(ref mut target) => {
                try!(target.prepare().chain_error(|| {
                    internal(format!("couldn't prepare build directories \
                                      for `{}`", pkg.name()))
                }));
            }
            None => {}
        }

        let targets = pkg.targets().iter();
        for target in targets.filter(|t| t.profile().is_compile()) {
            self.build_requirements(pkg, target, Platform::Target);
        }

        let jobs = self.jobs();
        self.compilation.extra_env.insert("NUM_JOBS".to_string(),
                                          jobs.to_string());
        self.compilation.root_output =
                self.layout(pkg, Kind::Target).proxy().dest().to_path_buf();
        self.compilation.deps_output =
                self.layout(pkg, Kind::Target).proxy().deps().to_path_buf();

        return Ok(());
    }

    fn build_requirements(&mut self, pkg: &'a Package, target: &'a Target,
                          req: Platform) {
        let req = if target.profile().is_for_host() {Platform::Plugin} else {req};
        match self.requirements.entry((pkg.package_id(), target.name())) {
            Occupied(mut entry) => match (*entry.get(), req) {
                (Platform::Plugin, Platform::Plugin) |
                (Platform::PluginAndTarget, Platform::Plugin) |
                (Platform::Target, Platform::Target) |
                (Platform::PluginAndTarget, Platform::Target) |
                (Platform::PluginAndTarget, Platform::PluginAndTarget) => return,
                _ => *entry.get_mut() = entry.get().combine(req),
            },
            Vacant(entry) => { entry.insert(req); }
        };

        for &(pkg, dep) in self.dep_targets(pkg, target).iter() {
            self.build_requirements(pkg, dep, req);
        }

        match pkg.targets().iter().find(|t| t.profile().is_custom_build()) {
            Some(custom_build) => {
                self.build_requirements(pkg, custom_build, Platform::Plugin);
            }
            None => {}
        }
    }

    pub fn get_requirement(&self, pkg: &'a Package,
                           target: &'a Target) -> Platform {
        let default = if target.profile().is_for_host() {
            Platform::Plugin
        } else {
            Platform::Target
        };
        self.requirements.get(&(pkg.package_id(), target.name()))
            .map(|a| *a).unwrap_or(default)
    }

    /// Returns the appropriate directory layout for either a plugin or not.
    pub fn layout(&self, pkg: &Package, kind: Kind) -> LayoutProxy {
        let primary = pkg.package_id() == self.resolve.root();
        match kind {
            Kind::Host => LayoutProxy::new(&self.host, primary),
            Kind::Target =>  LayoutProxy::new(self.target.as_ref()
                                                .unwrap_or(&self.host),
                                            primary),
        }
    }

    /// Returns the appropriate output directory for the specified package and
    /// target.
    pub fn out_dir(&self, pkg: &Package, kind: Kind, target: &Target) -> PathBuf {
        let out_dir = self.layout(pkg, kind);
        if target.profile().is_custom_build() {
            out_dir.build(pkg)
        } else if target.is_example() {
            out_dir.examples().to_path_buf()
        } else {
            out_dir.root().to_path_buf()
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
            Some((ref s1, ref s2)) => Ok((s1, s2)),
        }
    }

    /// Return the target triple which this context is targeting.
    pub fn target_triple(&self) -> &str {
        &self.target_triple
    }

    /// Return the exact filename of the target.
    pub fn target_filenames(&self, target: &Target) -> CargoResult<Vec<String>> {
        let stem = target.file_stem();

        let mut ret = Vec::new();
        if target.is_example() || target.is_bin() ||
           target.profile().is_test() {
            ret.push(format!("{}{}", stem,
                             if target.profile().is_for_host() {
                                 &self.host_exe
                             } else {
                                 &self.target_exe
                             }));
        } else {
            if target.is_dylib() {
                let plugin = target.profile().is_for_host();
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
        let deps = match self.resolve.deps(pkg.package_id()) {
            None => return vec!(),
            Some(deps) => deps,
        };
        let mut ret = deps.map(|id| self.get_package(id)).filter(|dep| {
            let pkg_dep = pkg.dependencies().iter().find(|d| {
                d.name() == dep.name()
            }).unwrap();

            // If this target is a build command, then we only want build
            // dependencies, otherwise we want everything *other than* build
            // dependencies.
            let is_correct_dep =
                target.profile().is_custom_build() == pkg_dep.is_build();

            // If this dependency is *not* a transitive dependency, then it
            // only applies to test/example targets
            let is_actual_dep = pkg_dep.is_transitive() ||
                                target.profile().is_test() ||
                                target.is_example();

            is_correct_dep && is_actual_dep
        }).filter_map(|pkg| {
            pkg.targets().iter().find(|&t| self.is_relevant_target(t))
               .map(|t| (pkg, t))
        }).collect::<Vec<_>>();

        // If this target is a binary, test, example, etc, then it depends on
        // the library of the same package. The call to `resolve.deps` above
        // didn't include `pkg` in the return values, so we need to special case
        // it here and see if we need to push `(pkg, pkg_lib_target)`.
        if !target.profile().is_custom_build() &&
           (target.is_bin() || target.is_example()) {
            let pkg = self.get_package(pkg.package_id());
            let target = pkg.targets().iter().filter(|t| {
                t.is_lib() && t.profile().is_compile() &&
                    (t.is_rlib() || t.is_dylib())
            }).next();
            if let Some(t) = target {
                ret.push((pkg, t));
            }
        }
        return ret;
    }

    /// Gets a package for the given package id.
    pub fn get_package(&self, id: &PackageId) -> &'a Package {
        self.package_set.iter()
            .find(|pkg| id == pkg.package_id())
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
            "doc" | "test" => target.profile().is_compile(),
            // doc-all == document everything, so look for doc targets and
            //            compile targets in dependencies
            "doc-all" => target.profile().is_compile() ||
                         (target.profile().env() == "doc" &&
                          target.profile().is_doc()),
            _ => target.profile().env() == self.env &&
                 !target.profile().is_test(),
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

    /// Number of jobs specified for this build
    pub fn jobs(&self) -> u32 { self.build_config.jobs }

    /// Requested (not actual) target for the build
    pub fn requested_target(&self) -> Option<&str> {
        self.build_config.requested_target.as_ref().map(|s| &s[..])
    }

    /// Calculate the actual profile to use for a target's compliation.
    ///
    /// This may involve overriding some options such as debug information,
    /// rpath, opt level, etc.
    pub fn profile(&self, target: &Target) -> Profile {
        let mut profile = target.profile().clone();
        let root_package = self.get_package(self.resolve.root());
        for target in root_package.manifest().targets().iter() {
            let root_profile = target.profile();
            if root_profile.env() != profile.env() { continue }
            profile = profile.set_opt_level(root_profile.opt_level())
                             .set_debug(root_profile.debug())
                             .set_rpath(root_profile.rpath())
        }
        profile
    }
}

impl Platform {
    pub fn combine(self, other: Platform) -> Platform {
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

    pub fn each_kind<F>(self, mut f: F) where F: FnMut(Kind) {
        match self {
            Platform::Target => f(Kind::Target),
            Platform::Plugin => f(Kind::Host),
            Platform::PluginAndTarget => { f(Kind::Target); f(Kind::Host); }
        }
    }
}
