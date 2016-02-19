use std::collections::{HashSet, HashMap};
use std::path::{Path, PathBuf};
use std::str::{self, FromStr};
use std::sync::Arc;

use regex::Regex;

use core::{Package, PackageId, PackageSet, Resolve, Target, Profile};
use core::{TargetKind, LibKind, Profiles, Metadata, Dependency};
use core::dependency::Kind as DepKind;
use util::{self, CargoResult, ChainError, internal, Config, profile, Cfg, human};

use super::TargetConfig;
use super::custom_build::{BuildState, BuildScripts};
use super::fingerprint::Fingerprint;
use super::layout::{Layout, LayoutProxy};
use super::links::Links;
use super::{Kind, Compilation, BuildConfig};
use super::{ProcessEngine, ExecEngine};

#[derive(Clone, Copy, Eq, PartialEq, Hash)]
pub struct Unit<'a> {
    pub pkg: &'a Package,
    pub target: &'a Target,
    pub profile: &'a Profile,
    pub kind: Kind,
}

pub struct Context<'a, 'cfg: 'a> {
    pub config: &'cfg Config,
    pub resolve: &'a Resolve,
    pub compilation: Compilation<'cfg>,
    pub packages: &'a PackageSet<'cfg>,
    pub build_state: Arc<BuildState>,
    pub build_explicit_deps: HashMap<Unit<'a>, (PathBuf, Vec<String>)>,
    pub exec_engine: Arc<Box<ExecEngine>>,
    pub fingerprints: HashMap<Unit<'a>, Arc<Fingerprint>>,
    pub compiled: HashSet<Unit<'a>>,
    pub build_config: BuildConfig,
    pub build_scripts: HashMap<Unit<'a>, Arc<BuildScripts>>,
    pub links: Links<'a>,

    host: Layout,
    target: Option<Layout>,
    target_triple: String,
    target_info: TargetInfo,
    host_info: TargetInfo,
    profiles: &'a Profiles,
}

#[derive(Clone)]
struct TargetInfo {
    dylib: Option<(String, String)>,
    staticlib: Option<(String, String)>,
    exe: String,
    cfg: Option<Vec<Cfg>>,
}

impl<'a, 'cfg> Context<'a, 'cfg> {
    pub fn new(resolve: &'a Resolve,
               packages: &'a PackageSet<'cfg>,
               config: &'cfg Config,
               host: Layout,
               target_layout: Option<Layout>,
               build_config: BuildConfig,
               profiles: &'a Profiles) -> CargoResult<Context<'a, 'cfg>> {
        let target = build_config.requested_target.clone();
        let target = target.as_ref().map(|s| &s[..]);
        let target_info = try!(Context::target_info(target, config));
        let host_info = if build_config.requested_target.is_none() {
            target_info.clone()
        } else {
            try!(Context::target_info(None, config))
        };
        let target_triple = target.unwrap_or_else(|| {
            &config.rustc_info().host[..]
        }).to_string();
        let engine = build_config.exec_engine.as_ref().cloned().unwrap_or({
            Arc::new(Box::new(ProcessEngine))
        });
        Ok(Context {
            target_triple: target_triple,
            host: host,
            target: target_layout,
            resolve: resolve,
            packages: packages,
            config: config,
            target_info: target_info,
            host_info: host_info,
            compilation: Compilation::new(config),
            build_state: Arc::new(BuildState::new(&build_config)),
            build_config: build_config,
            exec_engine: engine,
            fingerprints: HashMap::new(),
            profiles: profiles,
            compiled: HashSet::new(),
            build_scripts: HashMap::new(),
            build_explicit_deps: HashMap::new(),
            links: Links::new(),
        })
    }

    /// Run `rustc` to discover the dylib prefix/suffix for the target
    /// specified as well as the exe suffix
    fn target_info(target: Option<&str>, cfg: &Config)
                   -> CargoResult<TargetInfo> {
        let mut process = util::process(cfg.rustc());
        process.arg("-")
               .arg("--crate-name").arg("_")
               .arg("--crate-type").arg("dylib")
               .arg("--crate-type").arg("staticlib")
               .arg("--crate-type").arg("bin")
               .arg("--print=file-names")
               .env_remove("RUST_LOG");
        if let Some(s) = target {
            process.arg("--target").arg(s);
        };

        let mut with_cfg = process.clone();
        with_cfg.arg("--print=cfg");

        let mut has_cfg = true;
        let output = try!(with_cfg.exec_with_output().or_else(|_| {
            has_cfg = false;
            process.exec_with_output()
        }).chain_error(|| {
            human(format!("failed to run `rustc` to learn about \
                           target-specific information"))
        }));

        let error = str::from_utf8(&output.stderr).unwrap();
        let output = str::from_utf8(&output.stdout).unwrap();
        let mut lines = output.lines();
        let nodylib = Regex::new("unsupported crate type.*dylib").unwrap();
        let nostaticlib = Regex::new("unsupported crate type.*staticlib").unwrap();
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
        let staticlib = if nostaticlib.is_match(error) {
            None
        } else {
            let staticlib_parts: Vec<&str> = lines.next().unwrap().trim()
                                              .split('_').collect();
            assert!(staticlib_parts.len() == 2,
                    "rustc --print-file-name output has changed");
            Some((staticlib_parts[0].to_string(), staticlib_parts[1].to_string()))
        };

        let exe = if nobin.is_match(error) {
            String::new()
        } else {
            lines.next().unwrap().trim()
                 .split('_').skip(1).next().unwrap().to_string()
        };

        let cfg = if has_cfg {
            Some(try!(lines.map(Cfg::from_str).collect()))
        } else {
            None
        };

        Ok(TargetInfo {
            dylib: dylib,
            staticlib: staticlib,
            exe: exe,
            cfg: cfg,
        })
    }

    /// Prepare this context, ensuring that all filesystem directories are in
    /// place.
    pub fn prepare(&mut self, root: &Package) -> CargoResult<()> {
        let _p = profile::start("preparing layout");

        try!(self.host.prepare().chain_error(|| {
            internal(format!("couldn't prepare build directories"))
        }));
        match self.target {
            Some(ref mut target) => {
                try!(target.prepare().chain_error(|| {
                    internal(format!("couldn't prepare build directories"))
                }));
            }
            None => {}
        }

        self.compilation.root_output =
                self.layout(root, Kind::Target).proxy().dest().to_path_buf();
        self.compilation.deps_output =
                self.layout(root, Kind::Target).proxy().deps().to_path_buf();

        Ok(())
    }

    /// Returns the appropriate directory layout for either a plugin or not.
    pub fn layout(&self, pkg: &Package, kind: Kind) -> LayoutProxy {
        let primary = pkg.package_id() == self.resolve.root();
        match kind {
            Kind::Host => LayoutProxy::new(&self.host, primary),
            Kind::Target => LayoutProxy::new(self.target.as_ref()
                                                 .unwrap_or(&self.host),
                                             primary),
        }
    }

    /// Returns the appropriate output directory for the specified package and
    /// target.
    pub fn out_dir(&self, unit: &Unit) -> PathBuf {
        self.layout(unit.pkg, unit.kind).out_dir(unit.pkg, unit.target)
    }

    /// Return the (prefix, suffix) pair for dynamic libraries.
    ///
    /// If `plugin` is true, the pair corresponds to the host platform,
    /// otherwise it corresponds to the target platform.
    fn dylib(&self, kind: Kind) -> CargoResult<(&str, &str)> {
        let (triple, pair) = if kind == Kind::Host {
            (&self.config.rustc_info().host, &self.host_info.dylib)
        } else {
            (&self.target_triple, &self.target_info.dylib)
        };
        match *pair {
            None => bail!("dylib outputs are not supported for {}", triple),
            Some((ref s1, ref s2)) => Ok((s1, s2)),
        }
    }

    /// Return the (prefix, suffix) pair for static libraries.
    ///
    /// If `plugin` is true, the pair corresponds to the host platform,
    /// otherwise it corresponds to the target platform.
    pub fn staticlib(&self, kind: Kind) -> CargoResult<(&str, &str)> {
        let (triple, pair) = if kind == Kind::Host {
            (&self.config.rustc_info().host, &self.host_info.staticlib)
        } else {
            (&self.target_triple, &self.target_info.staticlib)
        };
        match *pair {
            None => bail!("staticlib outputs are not supported for {}", triple),
            Some((ref s1, ref s2)) => Ok((s1, s2)),
        }
    }

    /// Return the target triple which this context is targeting.
    pub fn target_triple(&self) -> &str {
        &self.target_triple
    }

    /// Get the metadata for a target in a specific profile
    pub fn target_metadata(&self, unit: &Unit) -> Option<Metadata> {
        let metadata = unit.target.metadata();
        if unit.target.is_lib() && unit.profile.test {
            // Libs and their tests are built in parallel, so we need to make
            // sure that their metadata is different.
            metadata.cloned().map(|mut m| {
                m.mix(&"test");
                m
            })
        } else if unit.target.is_bin() && unit.profile.test {
            // Make sure that the name of this test executable doesn't
            // conflict with a library that has the same name and is
            // being tested
            let mut metadata = unit.pkg.generate_metadata();
            metadata.mix(&format!("bin-{}", unit.target.name()));
            Some(metadata)
        } else if unit.pkg.package_id() == self.resolve.root() &&
                  !unit.profile.test {
            // If we're not building a unit test then the root package never
            // needs any metadata as it's guaranteed to not conflict with any
            // other output filenames. This means that we'll have predictable
            // file names like `target/debug/libfoo.{a,so,rlib}` and such.
            None
        } else {
            metadata.cloned()
        }
    }

    /// Returns the file stem for a given target/profile combo
    pub fn file_stem(&self, unit: &Unit) -> String {
        match self.target_metadata(unit) {
            Some(ref metadata) => format!("{}{}", unit.target.crate_name(),
                                          metadata.extra_filename),
            None if unit.target.allows_underscores() => {
                unit.target.name().to_string()
            }
            None => unit.target.crate_name(),
        }
    }

    /// Return the filenames that the given target for the given profile will
    /// generate.
    pub fn target_filenames(&self, unit: &Unit) -> CargoResult<Vec<String>> {
        let stem = self.file_stem(unit);
        let suffix = if unit.target.for_host() {
            &self.host_info.exe
        } else {
            &self.target_info.exe
        };

        let mut ret = Vec::new();
        match *unit.target.kind() {
            TargetKind::Example | TargetKind::Bin | TargetKind::CustomBuild |
            TargetKind::Bench | TargetKind::Test => {
                ret.push(format!("{}{}", stem, suffix));
            }
            TargetKind::Lib(..) if unit.profile.test => {
                ret.push(format!("{}{}", stem, suffix));
            }
            TargetKind::Lib(ref libs) => {
                for lib in libs.iter() {
                    match *lib {
                        LibKind::Dylib => {
                            if let Ok((prefix, suffix)) = self.dylib(unit.kind) {
                                ret.push(format!("{}{}{}", prefix, stem, suffix));
                            }
                        }
                        LibKind::Lib |
                        LibKind::Rlib => ret.push(format!("lib{}.rlib", stem)),
                        LibKind::StaticLib => {
                            if let Ok((prefix, suffix)) = self.staticlib(unit.kind) {
                                ret.push(format!("{}{}{}", prefix, stem, suffix));
                            }
                        }
                    }
                }
            }
        }
        assert!(!ret.is_empty());
        Ok(ret)
    }

    /// For a package, return all targets which are registered as dependencies
    /// for that package.
    pub fn dep_targets(&self, unit: &Unit<'a>) -> Vec<Unit<'a>> {
        if unit.profile.run_custom_build {
            return self.dep_run_custom_build(unit)
        } else if unit.profile.doc {
            return self.doc_deps(unit);
        }

        let id = unit.pkg.package_id();
        let deps = self.resolve.deps(id).into_iter().flat_map(|a| a);
        let mut ret = deps.map(|id| self.get_package(id)).filter(|dep| {
            unit.pkg.dependencies().iter().filter(|d| {
                d.name() == dep.name()
            }).any(|d| {
                // If this target is a build command, then we only want build
                // dependencies, otherwise we want everything *other than* build
                // dependencies.
                if unit.target.is_custom_build() != d.is_build() {
                    return false
                }

                // If this dependency is *not* a transitive dependency, then it
                // only applies to test/example targets
                if !d.is_transitive() && !unit.target.is_test() &&
                   !unit.target.is_example() && !unit.profile.test {
                    return false
                }

                // If this dependency is only available for certain platforms,
                // make sure we're only enabling it for that platform.
                if !self.dep_platform_activated(d, unit.kind) {
                    return false
                }

                // If the dependency is optional, then we're only activating it
                // if the corresponding feature was activated
                if d.is_optional() {
                    match self.resolve.features(id) {
                        Some(f) if f.contains(d.name()) => {}
                        _ => return false,
                    }
                }

                // If we've gotten past all that, then this dependency is
                // actually used!
                true
            })
        }).filter_map(|pkg| {
            pkg.targets().iter().find(|t| t.is_lib()).map(|t| {
                Unit {
                    pkg: pkg,
                    target: t,
                    profile: self.lib_profile(id),
                    kind: unit.kind.for_target(t),
                }
            })
        }).collect::<Vec<_>>();

        // If this target is a build script, then what we've collected so far is
        // all we need. If this isn't a build script, then it depends on the
        // build script if there is one.
        if unit.target.is_custom_build() {
            return ret
        }
        ret.extend(self.dep_build_script(unit));

        // If this target is a binary, test, example, etc, then it depends on
        // the library of the same package. The call to `resolve.deps` above
        // didn't include `pkg` in the return values, so we need to special case
        // it here and see if we need to push `(pkg, pkg_lib_target)`.
        if unit.target.is_lib() {
            return ret
        }
        ret.extend(self.maybe_lib(unit));

        // Integration tests/benchmarks require binaries to be built
        if unit.profile.test &&
           (unit.target.is_test() || unit.target.is_bench()) {
            ret.extend(unit.pkg.targets().iter().filter(|t| t.is_bin()).map(|t| {
                Unit {
                    pkg: unit.pkg,
                    target: t,
                    profile: self.lib_profile(id),
                    kind: unit.kind.for_target(t),
                }
            }));
        }
        ret
    }

    /// Returns the dependencies needed to run a build script.
    ///
    /// The `unit` provided must represent an execution of a build script, and
    /// the returned set of units must all be run before `unit` is run.
    pub fn dep_run_custom_build(&self, unit: &Unit<'a>) -> Vec<Unit<'a>> {
        // If this build script's execution has been overridden then we don't
        // actually depend on anything, we've reached the end of the dependency
        // chain as we've got all the info we're gonna get.
        let key = (unit.pkg.package_id().clone(), unit.kind);
        if self.build_state.outputs.lock().unwrap().contains_key(&key) {
            return Vec::new()
        }

        // When not overridden, then the dependencies to run a build script are:
        //
        // 1. Compiling the build script itself
        // 2. For each immediate dependency of our package which has a `links`
        //    key, the execution of that build script.
        let not_custom_build = unit.pkg.targets().iter().find(|t| {
            !t.is_custom_build()
        }).unwrap();
        let tmp = Unit {
            target: not_custom_build,
            profile: &self.profiles.dev,
            ..*unit
        };
        self.dep_targets(&tmp).iter().filter_map(|unit| {
            if !unit.target.linkable() || unit.pkg.manifest().links().is_none() {
                return None
            }
            self.dep_build_script(unit)
        }).chain(Some(Unit {
            profile: self.build_script_profile(unit.pkg.package_id()),
            kind: Kind::Host, // build scripts always compiled for the host
            ..*unit
        })).collect()
    }

    /// Returns the dependencies necessary to document a package
    fn doc_deps(&self, unit: &Unit<'a>) -> Vec<Unit<'a>> {
        let deps = self.resolve.deps(unit.pkg.package_id()).into_iter();
        let deps = deps.flat_map(|a| a).map(|id| {
            self.get_package(id)
        }).filter(|dep| {
            unit.pkg.dependencies().iter().filter(|d| {
                d.name() == dep.name()
            }).any(|dep| {
                match dep.kind() {
                    DepKind::Normal => self.dep_platform_activated(dep,
                                                                   unit.kind),
                    _ => false,
                }
            })
        }).filter_map(|dep| {
            dep.targets().iter().find(|t| t.is_lib()).map(|t| (dep, t))
        });

        // To document a library, we depend on dependencies actually being
        // built. If we're documenting *all* libraries, then we also depend on
        // the documentation of the library being built.
        let mut ret = Vec::new();
        for (dep, lib) in deps {
            ret.push(Unit {
                pkg: dep,
                target: lib,
                profile: self.lib_profile(dep.package_id()),
                kind: unit.kind.for_target(lib),
            });
            if self.build_config.doc_all {
                ret.push(Unit {
                    pkg: dep,
                    target: lib,
                    profile: &self.profiles.doc,
                    kind: unit.kind.for_target(lib),
                });
            }
        }

        // Be sure to build/run the build script for documented libraries as
        ret.extend(self.dep_build_script(unit));

        // If we document a binary, we need the library available
        if unit.target.is_bin() {
            ret.extend(self.maybe_lib(unit));
        }
        ret
    }

    /// If a build script is scheduled to be run for the package specified by
    /// `unit`, this function will return the unit to run that build script.
    ///
    /// Overriding a build script simply means that the running of the build
    /// script itself doesn't have any dependencies, so even in that case a unit
    /// of work is still returned. `None` is only returned if the package has no
    /// build script.
    fn dep_build_script(&self, unit: &Unit<'a>) -> Option<Unit<'a>> {
        unit.pkg.targets().iter().find(|t| t.is_custom_build()).map(|t| {
            Unit {
                pkg: unit.pkg,
                target: t,
                profile: &self.profiles.custom_build,
                kind: unit.kind,
            }
        })
    }

    fn maybe_lib(&self, unit: &Unit<'a>) -> Option<Unit<'a>> {
        unit.pkg.targets().iter().find(|t| t.linkable()).map(|t| {
            Unit {
                pkg: unit.pkg,
                target: t,
                profile: self.lib_profile(unit.pkg.package_id()),
                kind: unit.kind.for_target(t),
            }
        })
    }

    fn dep_platform_activated(&self, dep: &Dependency, kind: Kind) -> bool {
        // If this dependency is only available for certain platforms,
        // make sure we're only enabling it for that platform.
        let platform = match dep.platform() {
            Some(p) => p,
            None => return true,
        };
        let (name, info) = match kind {
            Kind::Host => (&self.config.rustc_info().host, &self.host_info),
            Kind::Target => (&self.target_triple, &self.target_info),
        };
        platform.matches(name, info.cfg.as_ref().map(|cfg| &cfg[..]))
    }

    /// Gets a package for the given package id.
    pub fn get_package(&self, id: &PackageId) -> &'a Package {
        self.packages.packages()
            .find(|pkg| id == pkg.package_id())
            .expect("Should have found package")
    }

    /// Get the user-specified linker for a particular host or target
    pub fn linker(&self, kind: Kind) -> Option<&Path> {
        self.target_config(kind).linker.as_ref().map(|s| s.as_ref())
    }

    /// Get the user-specified `ar` program for a particular host or target
    pub fn ar(&self, kind: Kind) -> Option<&Path> {
        self.target_config(kind).ar.as_ref().map(|s| s.as_ref())
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

    pub fn lib_profile(&self, _pkg: &PackageId) -> &'a Profile {
        if self.build_config.release {
            &self.profiles.release
        } else {
            &self.profiles.dev
        }
    }

    pub fn build_script_profile(&self, _pkg: &PackageId) -> &'a Profile {
        // TODO: should build scripts always be built with a dev
        //       profile? How is this controlled at the CLI layer?
        &self.profiles.dev
    }
}
