use std::collections::{HashSet, HashMap, BTreeSet};
use std::env;
use std::path::{Path, PathBuf};
use std::str::{self, FromStr};
use std::sync::Arc;


use core::{Package, PackageId, PackageSet, Resolve, Target, Profile};
use core::{TargetKind, Profiles, Metadata, Dependency, Workspace};
use core::dependency::Kind as DepKind;
use util::{CargoResult, ChainError, internal, Config, profile, Cfg, human};

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
    target_info: TargetInfo,
    host_info: TargetInfo,
    profiles: &'a Profiles,
}

#[derive(Clone, Default)]
struct TargetInfo {
    crate_types: HashMap<String, Option<(String, String)>>,
    cfg: Option<Vec<Cfg>>,
}

impl<'a, 'cfg> Context<'a, 'cfg> {
    pub fn new(ws: &Workspace<'cfg>,
               resolve: &'a Resolve,
               packages: &'a PackageSet<'cfg>,
               config: &'cfg Config,
               build_config: BuildConfig,
               profiles: &'a Profiles) -> CargoResult<Context<'a, 'cfg>> {

        let dest = if build_config.release { "release" } else { "debug" };
        let host_layout = try!(Layout::new(ws, None, &dest));
        let target_layout = match build_config.requested_target.as_ref() {
            Some(target) => {
                Some(try!(Layout::new(ws, Some(&target), &dest)))
            }
            None => None,
        };

        let engine = build_config.exec_engine.as_ref().cloned().unwrap_or({
            Arc::new(Box::new(ProcessEngine))
        });
        Ok(Context {
            host: host_layout,
            target: target_layout,
            resolve: resolve,
            packages: packages,
            config: config,
            target_info: TargetInfo::default(),
            host_info: TargetInfo::default(),
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

    /// Ensure that we've collected all target-specific information to compile
    /// all the units mentioned in `units`.
    pub fn probe_target_info(&mut self, units: &[Unit<'a>]) -> CargoResult<()> {
        let mut crate_types = BTreeSet::new();
        // pre-fill with `bin` for learning about tests (nothing may be
        // explicitly `bin`) as well as `rlib` as it's the coalesced version of
        // `lib` in the compiler and we're not sure which we'll see.
        crate_types.insert("bin".to_string());
        crate_types.insert("rlib".to_string());
        for unit in units {
            try!(self.visit_crate_type(unit, &mut crate_types));
        }
        try!(self.probe_target_info_kind(&crate_types, Kind::Target));
        if self.requested_target().is_none() {
            self.host_info = self.target_info.clone();
        } else {
            try!(self.probe_target_info_kind(&crate_types, Kind::Host));
        }
        Ok(())
    }

    fn visit_crate_type(&self,
                        unit: &Unit<'a>,
                        crate_types: &mut BTreeSet<String>)
                        -> CargoResult<()> {
        for target in unit.pkg.manifest().targets() {
            crate_types.extend(target.rustc_crate_types().iter().map(|s| {
                if *s == "lib" {
                    "rlib".to_string()
                } else {
                    s.to_string()
                }
            }));
        }
        for dep in try!(self.dep_targets(&unit)) {
            try!(self.visit_crate_type(&dep, crate_types));
        }
        Ok(())
    }

    fn probe_target_info_kind(&mut self,
                              crate_types: &BTreeSet<String>,
                              kind: Kind)
                              -> CargoResult<()> {
        let rustflags = try!(env_args(self.config,
                                      &self.build_config,
                                      kind,
                                      "RUSTFLAGS"));
        let mut process = try!(self.config.rustc()).process();
        process.arg("-")
               .arg("--crate-name").arg("_")
               .arg("--print=file-names")
               .args(&rustflags)
               .env_remove("RUST_LOG");

        for crate_type in crate_types {
            process.arg("--crate-type").arg(crate_type);
        }
        if kind == Kind::Target {
            process.arg("--target").arg(&self.target_triple());
        }

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
        let mut map = HashMap::new();
        for crate_type in crate_types {
            let not_supported = error.lines().any(|line| {
                line.contains("unsupported crate type") &&
                    line.contains(crate_type)
            });
            if not_supported {
                map.insert(crate_type.to_string(), None);
                continue
            }
            let line = match lines.next() {
                Some(line) => line,
                None => bail!("malformed output when learning about \
                               target-specific information from rustc"),
            };
            let mut parts = line.trim().split('_');
            let prefix = parts.next().unwrap();
            let suffix = match parts.next() {
                Some(part) => part,
                None => bail!("output of --print=file-names has changed in \
                               the compiler, cannot parse"),
            };
            map.insert(crate_type.to_string(),
                       Some((prefix.to_string(), suffix.to_string())));
        }

        let cfg = if has_cfg {
            Some(try!(lines.map(Cfg::from_str).collect()))
        } else {
            None
        };

        let info = match kind {
            Kind::Target => &mut self.target_info,
            Kind::Host => &mut self.host_info,
        };
        info.crate_types = map;
        info.cfg = cfg;
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
        if unit.profile.doc {
            self.layout(unit.pkg, unit.kind).doc_root()
        } else {
            self.layout(unit.pkg, unit.kind).out_dir(unit.pkg, unit.target)
        }
    }

    /// Return the host triple for this context
    pub fn host_triple(&self) -> &str {
        &self.build_config.host_triple
    }

    /// Return the target triple which this context is targeting.
    pub fn target_triple(&self) -> &str {
        self.requested_target().unwrap_or(self.host_triple())
    }

    /// Requested (not actual) target for the build
    pub fn requested_target(&self) -> Option<&str> {
        self.build_config.requested_target.as_ref().map(|s| &s[..])
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
    /// generate, along with whether you can link against that file (e.g. it's a
    /// library).
    pub fn target_filenames(&self, unit: &Unit)
                            -> CargoResult<Vec<(String, bool)>> {
        let stem = self.file_stem(unit);
        let info = if unit.target.for_host() {
            &self.host_info
        } else {
            &self.target_info
        };

        let mut ret = Vec::new();
        let mut unsupported = Vec::new();
        {
            let mut add = |crate_type: &str, linkable: bool| -> CargoResult<()> {
                let crate_type = if crate_type == "lib" {"rlib"} else {crate_type};
                match info.crate_types.get(crate_type) {
                    Some(&Some((ref prefix, ref suffix))) => {
                        ret.push((format!("{}{}{}", prefix, stem, suffix),
                                  linkable));
                        Ok(())
                    }
                    // not supported, don't worry about it
                    Some(&None) => {
                        unsupported.push(crate_type.to_string());
                        Ok(())
                    }
                    None => {
                        bail!("failed to learn about crate-type `{}` early on",
                              crate_type)
                    }
                }
            };
            match *unit.target.kind() {
                TargetKind::Example |
                TargetKind::Bin |
                TargetKind::CustomBuild |
                TargetKind::Bench |
                TargetKind::Test => {
                    try!(add("bin", false));
                }
                TargetKind::Lib(..) if unit.profile.test => {
                    try!(add("bin", false));
                }
                TargetKind::Lib(ref libs) => {
                    for lib in libs {
                        try!(add(lib.crate_type(), lib.linkable()));
                    }
                }
            }
        }
        if ret.is_empty() {
            if unsupported.len() > 0 {
                bail!("cannot produce {} for `{}` as the target `{}` \
                       does not support these crate types",
                      unsupported.join(", "), unit.pkg, self.target_triple())
            }
            bail!("cannot compile `{}` as the target `{}` does not \
                   support any of the output crate types",
                  unit.pkg, self.target_triple());
        }
        Ok(ret)
    }

    /// For a package, return all targets which are registered as dependencies
    /// for that package.
    pub fn dep_targets(&self, unit: &Unit<'a>) -> CargoResult<Vec<Unit<'a>>> {
        if unit.profile.run_custom_build {
            return self.dep_run_custom_build(unit)
        } else if unit.profile.doc {
            return self.doc_deps(unit);
        }

        let id = unit.pkg.package_id();
        let deps = self.resolve.deps(id);
        let mut ret = try!(deps.filter(|dep| {
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
        }).filter_map(|id| {
            match self.get_package(id) {
                Ok(pkg) => {
                    pkg.targets().iter().find(|t| t.is_lib()).map(|t| {
                        Ok(Unit {
                            pkg: pkg,
                            target: t,
                            profile: self.lib_profile(id),
                            kind: unit.kind.for_target(t),
                        })
                    })
                }
                Err(e) => Some(Err(e))
            }
        }).collect::<CargoResult<Vec<_>>>());

        // If this target is a build script, then what we've collected so far is
        // all we need. If this isn't a build script, then it depends on the
        // build script if there is one.
        if unit.target.is_custom_build() {
            return Ok(ret)
        }
        ret.extend(self.dep_build_script(unit));

        // If this target is a binary, test, example, etc, then it depends on
        // the library of the same package. The call to `resolve.deps` above
        // didn't include `pkg` in the return values, so we need to special case
        // it here and see if we need to push `(pkg, pkg_lib_target)`.
        if unit.target.is_lib() {
            return Ok(ret)
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
        Ok(ret)
    }

    /// Returns the dependencies needed to run a build script.
    ///
    /// The `unit` provided must represent an execution of a build script, and
    /// the returned set of units must all be run before `unit` is run.
    pub fn dep_run_custom_build(&self, unit: &Unit<'a>)
                                -> CargoResult<Vec<Unit<'a>>> {
        // If this build script's execution has been overridden then we don't
        // actually depend on anything, we've reached the end of the dependency
        // chain as we've got all the info we're gonna get.
        let key = (unit.pkg.package_id().clone(), unit.kind);
        if self.build_state.outputs.lock().unwrap().contains_key(&key) {
            return Ok(Vec::new())
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
        let deps = try!(self.dep_targets(&tmp));
        Ok(deps.iter().filter_map(|unit| {
            if !unit.target.linkable() || unit.pkg.manifest().links().is_none() {
                return None
            }
            self.dep_build_script(unit)
        }).chain(Some(Unit {
            profile: self.build_script_profile(unit.pkg.package_id()),
            kind: Kind::Host, // build scripts always compiled for the host
            ..*unit
        })).collect())
    }

    /// Returns the dependencies necessary to document a package
    fn doc_deps(&self, unit: &Unit<'a>) -> CargoResult<Vec<Unit<'a>>> {
        let deps = self.resolve.deps(unit.pkg.package_id()).filter(|dep| {
            unit.pkg.dependencies().iter().filter(|d| {
                d.name() == dep.name()
            }).any(|dep| {
                match dep.kind() {
                    DepKind::Normal => self.dep_platform_activated(dep,
                                                                   unit.kind),
                    _ => false,
                }
            })
        }).map(|dep| {
            self.get_package(dep)
        });

        // To document a library, we depend on dependencies actually being
        // built. If we're documenting *all* libraries, then we also depend on
        // the documentation of the library being built.
        let mut ret = Vec::new();
        for dep in deps {
            let dep = try!(dep);
            let lib = match dep.targets().iter().find(|t| t.is_lib()) {
                Some(lib) => lib,
                None => continue,
            };
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
        Ok(ret)
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
            Kind::Host => (self.host_triple(), &self.host_info),
            Kind::Target => (self.target_triple(), &self.target_info),
        };
        platform.matches(name, info.cfg.as_ref().map(|cfg| &cfg[..]))
    }

    /// Gets a package for the given package id.
    pub fn get_package(&self, id: &PackageId) -> CargoResult<&'a Package> {
        self.packages.get(id)
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

    pub fn lib_profile(&self, _pkg: &PackageId) -> &'a Profile {
        let (normal, test) = if self.build_config.release {
            (&self.profiles.release, &self.profiles.bench_deps)
        } else {
            (&self.profiles.dev, &self.profiles.test_deps)
        };
        if self.build_config.test {
            test
        } else {
            normal
        }
    }

    pub fn build_script_profile(&self, pkg: &PackageId) -> &'a Profile {
        // TODO: should build scripts always be built with the same library
        //       profile? How is this controlled at the CLI layer?
        self.lib_profile(pkg)
    }

    pub fn rustflags_args(&self, unit: &Unit) -> CargoResult<Vec<String>> {
        env_args(self.config, &self.build_config, unit.kind, "RUSTFLAGS")
    }

    pub fn rustdocflags_args(&self, unit: &Unit) -> CargoResult<Vec<String>> {
        env_args(self.config, &self.build_config, unit.kind, "RUSTDOCFLAGS")
    }

    pub fn show_warnings(&self, pkg: &PackageId) -> bool {
        pkg == self.resolve.root() || pkg.source_id().is_path() ||
            self.config.extra_verbose()
    }
}

// Acquire extra flags to pass to the compiler from the
// RUSTFLAGS environment variable and similar config values
fn env_args(config: &Config,
            build_config: &BuildConfig,
            kind: Kind,
            name: &str) -> CargoResult<Vec<String>> {
    // We *want* to apply RUSTFLAGS only to builds for the
    // requested target architecture, and not to things like build
    // scripts and plugins, which may be for an entirely different
    // architecture. Cargo's present architecture makes it quite
    // hard to only apply flags to things that are not build
    // scripts and plugins though, so we do something more hacky
    // instead to avoid applying the same RUSTFLAGS to multiple targets
    // arches:
    //
    // 1) If --target is not specified we just apply RUSTFLAGS to
    // all builds; they are all going to have the same target.
    //
    // 2) If --target *is* specified then we only apply RUSTFLAGS
    // to compilation units with the Target kind, which indicates
    // it was chosen by the --target flag.
    //
    // This means that, e.g. even if the specified --target is the
    // same as the host, build scripts in plugins won't get
    // RUSTFLAGS.
    let compiling_with_target = build_config.requested_target.is_some();
    let is_target_kind = kind == Kind::Target;

    if compiling_with_target && !is_target_kind {
        // This is probably a build script or plugin and we're
        // compiling with --target. In this scenario there are
        // no rustflags we can apply.
        return Ok(Vec::new());
    }

    // First try RUSTFLAGS from the environment
    if let Some(a) = env::var(name).ok() {
        let args = a.split(" ")
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        return Ok(args.collect());
    }

    // Then the build.rustflags value
    let name = name.chars().flat_map(|c| c.to_lowercase()).collect::<String>();
    let key = format!("build.{}", name);
    if let Some(args) = try!(config.get_list(&key)) {
        let args = args.val.into_iter().map(|a| a.0);
        return Ok(args.collect());
    }

    Ok(Vec::new())
}
