#![allow(deprecated)]

use std::collections::{HashSet, HashMap, BTreeSet};
use std::collections::hash_map::Entry;
use std::env;
use std::fmt;
use std::hash::{Hasher, Hash, SipHasher};
use std::path::{Path, PathBuf};
use std::str::{self, FromStr};
use std::sync::Arc;
use std::cell::RefCell;

use jobserver::Client;

use core::{Package, PackageId, PackageSet, Resolve, Target, Profile};
use core::{TargetKind, Profiles, Dependency, Workspace};
use core::dependency::Kind as DepKind;
use util::{self, ProcessBuilder, internal, Config, profile, Cfg, CfgExpr};
use util::errors::{CargoResult, CargoResultExt};

use super::TargetConfig;
use super::custom_build::{BuildState, BuildScripts, BuildDeps};
use super::fingerprint::Fingerprint;
use super::layout::Layout;
use super::links::Links;
use super::{Kind, Compilation, BuildConfig};

#[derive(Clone, Copy, Eq, PartialEq, Hash)]
pub struct Unit<'a> {
    pub pkg: &'a Package,
    pub target: &'a Target,
    pub profile: &'a Profile,
    pub kind: Kind,
}

pub struct Context<'a, 'cfg: 'a> {
    pub ws: &'a Workspace<'cfg>,
    pub config: &'cfg Config,
    pub resolve: &'a Resolve,
    pub compilation: Compilation<'cfg>,
    pub packages: &'a PackageSet<'cfg>,
    pub build_state: Arc<BuildState>,
    pub build_script_overridden: HashSet<(PackageId, Kind)>,
    pub build_explicit_deps: HashMap<Unit<'a>, BuildDeps>,
    pub fingerprints: HashMap<Unit<'a>, Arc<Fingerprint>>,
    pub compiled: HashSet<Unit<'a>>,
    pub build_config: BuildConfig,
    pub build_scripts: HashMap<Unit<'a>, Arc<BuildScripts>>,
    pub links: Links<'a>,
    pub used_in_plugin: HashSet<Unit<'a>>,
    pub jobserver: Client,

    host: Layout,
    target: Option<Layout>,
    target_info: TargetInfo,
    host_info: TargetInfo,
    profiles: &'a Profiles,
    incremental_enabled: bool,

    target_filenames: HashMap<Unit<'a>, Arc<Vec<(PathBuf, Option<PathBuf>, bool)>>>,
    target_metadatas: HashMap<Unit<'a>, Option<Metadata>>,
}

#[derive(Clone, Default)]
struct TargetInfo {
    crate_type_process: Option<ProcessBuilder>,
    crate_types: RefCell<HashMap<String, Option<(String, String)>>>,
    cfg: Option<Vec<Cfg>>,
}

impl TargetInfo {
    fn discover_crate_type(&self, crate_type: &str) -> CargoResult<Option<(String, String)>> {
        let mut process = self.crate_type_process.clone().unwrap();

        process.arg("--crate-type").arg(crate_type);

        let output = process.exec_with_output().chain_err(|| {
            format!("failed to run `rustc` to learn about \
                     crate-type {} information", crate_type)
        })?;

        let error = str::from_utf8(&output.stderr).unwrap();
        let output = str::from_utf8(&output.stdout).unwrap();
        Ok(parse_crate_type(crate_type, error, &mut output.lines())?)
    }
}

#[derive(Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct Metadata(u64);

impl<'a, 'cfg> Context<'a, 'cfg> {
    pub fn new(ws: &'a Workspace<'cfg>,
               resolve: &'a Resolve,
               packages: &'a PackageSet<'cfg>,
               config: &'cfg Config,
               build_config: BuildConfig,
               profiles: &'a Profiles) -> CargoResult<Context<'a, 'cfg>> {

        let dest = if build_config.release { "release" } else { "debug" };
        let host_layout = Layout::new(ws, None, dest)?;
        let target_layout = match build_config.requested_target.as_ref() {
            Some(target) => Some(Layout::new(ws, Some(target), dest)?),
            None => None,
        };

        // Enable incremental builds if the user opts in. For now,
        // this is an environment variable until things stabilize a
        // bit more.
        let incremental_enabled = match env::var("CARGO_INCREMENTAL") {
            Ok(v) => v == "1",
            Err(_) => false,
        };

        // -Z can only be used on nightly builds; other builds complain loudly.
        // Since incremental builds only work on nightly anyway, we silently
        // ignore CARGO_INCREMENTAL on anything but nightly. This allows users
        // to always have CARGO_INCREMENTAL set without getting unexpected
        // errors on stable/beta builds.
        let is_nightly =
            config.rustc()?.verbose_version.contains("-nightly") ||
            config.rustc()?.verbose_version.contains("-dev");
        let incremental_enabled = incremental_enabled && is_nightly;

        // Load up the jobserver that we'll use to manage our parallelism. This
        // is the same as the GNU make implementation of a jobserver, and
        // intentionally so! It's hoped that we can interact with GNU make and
        // all share the same jobserver.
        //
        // Note that if we don't have a jobserver in our environment then we
        // create our own, and we create it with `n-1` tokens because one token
        // is ourself, a running process.
        let jobserver = match config.jobserver_from_env() {
            Some(c) => c.clone(),
            None => Client::new(build_config.jobs as usize - 1).chain_err(|| {
                "failed to create jobserver"
            })?,
        };

        Ok(Context {
            ws: ws,
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
            fingerprints: HashMap::new(),
            profiles: profiles,
            compiled: HashSet::new(),
            build_scripts: HashMap::new(),
            build_explicit_deps: HashMap::new(),
            links: Links::new(),
            used_in_plugin: HashSet::new(),
            incremental_enabled: incremental_enabled,
            jobserver: jobserver,
            build_script_overridden: HashSet::new(),

            // TODO: Pre-Calculate these with a topo-sort, rather than lazy-calculating
            target_filenames: HashMap::new(),
            target_metadatas: HashMap::new(),
        })
    }

    /// Prepare this context, ensuring that all filesystem directories are in
    /// place.
    pub fn prepare(&mut self) -> CargoResult<()> {
        let _p = profile::start("preparing layout");

        self.host.prepare().chain_err(|| {
            internal("couldn't prepare build directories")
        })?;
        if let Some(ref mut target) = self.target {
            target.prepare().chain_err(|| {
                internal("couldn't prepare build directories")
            })?;
        }

        self.compilation.host_deps_output = self.host.deps().to_path_buf();

        let layout = self.target.as_ref().unwrap_or(&self.host);
        self.compilation.root_output = layout.dest().to_path_buf();
        self.compilation.deps_output = layout.deps().to_path_buf();
        Ok(())
    }

    /// Ensure that we've collected all target-specific information to compile
    /// all the units mentioned in `units`.
    pub fn probe_target_info(&mut self, units: &[Unit<'a>]) -> CargoResult<()> {
        let mut crate_types = BTreeSet::new();
        let mut visited_units = HashSet::new();
        // pre-fill with `bin` for learning about tests (nothing may be
        // explicitly `bin`) as well as `rlib` as it's the coalesced version of
        // `lib` in the compiler and we're not sure which we'll see.
        crate_types.insert("bin".to_string());
        crate_types.insert("rlib".to_string());
        for unit in units {
            self.visit_crate_type(unit, &mut crate_types, &mut visited_units)?;
        }
        debug!("probe_target_info: crate_types={:?}", crate_types);
        self.probe_target_info_kind(&crate_types, Kind::Target)?;
        if self.requested_target().is_none() {
            self.host_info = self.target_info.clone();
        } else {
            self.probe_target_info_kind(&crate_types, Kind::Host)?;
        }
        Ok(())
    }

    fn visit_crate_type(&self,
                        unit: &Unit<'a>,
                        crate_types: &mut BTreeSet<String>,
                        visited_units: &mut HashSet<Unit<'a>>)
                        -> CargoResult<()> {
        if !visited_units.insert(*unit) {
            return Ok(());
        }
        for target in unit.pkg.manifest().targets() {
            crate_types.extend(target.rustc_crate_types().iter().map(|s| {
                if *s == "lib" {
                    "rlib".to_string()
                } else {
                    s.to_string()
                }
            }));
        }
        for dep in self.dep_targets(unit)? {
            self.visit_crate_type(&dep, crate_types, visited_units)?;
        }
        Ok(())
    }

    fn probe_target_info_kind(&mut self,
                              crate_types: &BTreeSet<String>,
                              kind: Kind)
                              -> CargoResult<()> {
        let rustflags = env_args(self.config,
                                 &self.build_config,
                                 self.info(&kind),
                                 kind,
                                 "RUSTFLAGS")?;
        let mut process = self.config.rustc()?.process();
        process.arg("-")
               .arg("--crate-name").arg("___")
               .arg("--print=file-names")
               .args(&rustflags)
               .env_remove("RUST_LOG");

        if kind == Kind::Target {
            process.arg("--target").arg(&self.target_triple());
        }

        let crate_type_process = process.clone();

        for crate_type in crate_types {
            process.arg("--crate-type").arg(crate_type);
        }

        let mut with_cfg = process.clone();
        with_cfg.arg("--print=sysroot");
        with_cfg.arg("--print=cfg");

        let mut has_cfg_and_sysroot = true;
        let output = with_cfg.exec_with_output().or_else(|_| {
            has_cfg_and_sysroot = false;
            process.exec_with_output()
        }).chain_err(|| {
            "failed to run `rustc` to learn about target-specific information"
        })?;

        let error = str::from_utf8(&output.stderr).unwrap();
        let output = str::from_utf8(&output.stdout).unwrap();
        let mut lines = output.lines();
        let mut map = HashMap::new();
        for crate_type in crate_types {
            let out = parse_crate_type(crate_type, error, &mut lines)?;
            map.insert(crate_type.to_string(), out);
        }

        if has_cfg_and_sysroot {
            let line = match lines.next() {
                Some(line) => line,
                None => bail!("output of --print=sysroot missing when learning about \
                               target-specific information from rustc"),
            };
            let mut rustlib = PathBuf::from(line);
            if kind == Kind::Host {
                if cfg!(windows) {
                    rustlib.push("bin");
                } else {
                    rustlib.push("lib");
                }
                self.compilation.host_dylib_path = Some(rustlib);
            } else {
                rustlib.push("lib");
                rustlib.push("rustlib");
                rustlib.push(self.target_triple());
                rustlib.push("lib");
                self.compilation.target_dylib_path = Some(rustlib);
            }
        }

        let cfg = if has_cfg_and_sysroot {
            Some(try!(lines.map(Cfg::from_str).collect()))
        } else {
            None
        };

        let info = match kind {
            Kind::Target => &mut self.target_info,
            Kind::Host => &mut self.host_info,
        };
        info.crate_type_process = Some(crate_type_process);
        info.crate_types = RefCell::new(map);
        info.cfg = cfg;
        Ok(())
    }

    /// Builds up the `used_in_plugin` internal to this context from the list of
    /// top-level units.
    ///
    /// This will recursively walk `units` and all of their dependencies to
    /// determine which crate are going to be used in plugins or not.
    pub fn build_used_in_plugin_map(&mut self, units: &[Unit<'a>])
                                    -> CargoResult<()> {
        let mut visited = HashSet::new();
        for unit in units {
            self.walk_used_in_plugin_map(unit,
                                         unit.target.for_host(),
                                         &mut visited)?;
        }
        Ok(())
    }

    fn walk_used_in_plugin_map(&mut self,
                               unit: &Unit<'a>,
                               is_plugin: bool,
                               visited: &mut HashSet<(Unit<'a>, bool)>)
                               -> CargoResult<()> {
        if !visited.insert((*unit, is_plugin)) {
            return Ok(())
        }
        if is_plugin {
            self.used_in_plugin.insert(*unit);
        }
        for unit in self.dep_targets(unit)? {
            self.walk_used_in_plugin_map(&unit,
                                         is_plugin || unit.target.for_host(),
                                         visited)?;
        }
        Ok(())
    }

    /// Returns the appropriate directory layout for either a plugin or not.
    fn layout(&self, kind: Kind) -> &Layout {
        match kind {
            Kind::Host => &self.host,
            Kind::Target => self.target.as_ref().unwrap_or(&self.host)
        }
    }

    /// Returns the directories where Rust crate dependencies are found for the
    /// specified unit.
    pub fn deps_dir(&self, unit: &Unit) -> &Path {
        self.layout(unit.kind).deps()
    }

    /// Returns the directory for the specified unit where fingerprint
    /// information is stored.
    pub fn fingerprint_dir(&mut self, unit: &Unit<'a>) -> PathBuf {
        let dir = self.pkg_dir(unit);
        self.layout(unit.kind).fingerprint().join(dir)
    }

    /// Returns the appropriate directory layout for either a plugin or not.
    pub fn build_script_dir(&mut self, unit: &Unit<'a>) -> PathBuf {
        assert!(unit.target.is_custom_build());
        assert!(!unit.profile.run_custom_build);
        let dir = self.pkg_dir(unit);
        self.layout(Kind::Host).build().join(dir)
    }

    /// Returns the appropriate directory layout for either a plugin or not.
    pub fn build_script_out_dir(&mut self, unit: &Unit<'a>) -> PathBuf {
        assert!(unit.target.is_custom_build());
        assert!(unit.profile.run_custom_build);
        let dir = self.pkg_dir(unit);
        self.layout(unit.kind).build().join(dir).join("out")
    }

    pub fn host_deps(&self) -> &Path {
        self.host.deps()
    }

    /// Return the root of the build output tree
    pub fn target_root(&self) -> &Path {
        self.host.dest()
    }

    /// Returns the appropriate output directory for the specified package and
    /// target.
    pub fn out_dir(&mut self, unit: &Unit<'a>) -> PathBuf {
        if unit.profile.doc {
            self.layout(unit.kind).root().parent().unwrap().join("doc")
        } else if unit.target.is_custom_build() {
            self.build_script_dir(unit)
        } else if unit.target.is_example() {
            self.layout(unit.kind).examples().to_path_buf()
        } else {
            self.deps_dir(unit).to_path_buf()
        }
    }

    fn pkg_dir(&mut self, unit: &Unit<'a>) -> String {
        let name = unit.pkg.package_id().name();
        match self.target_metadata(unit) {
            Some(meta) => format!("{}-{}", name, meta),
            None => format!("{}-{}", name, self.target_short_hash(unit)),
        }
    }

    /// Return the host triple for this context
    pub fn host_triple(&self) -> &str {
        &self.build_config.host_triple
    }

    /// Return the target triple which this context is targeting.
    pub fn target_triple(&self) -> &str {
        self.requested_target().unwrap_or_else(|| self.host_triple())
    }

    /// Requested (not actual) target for the build
    pub fn requested_target(&self) -> Option<&str> {
        self.build_config.requested_target.as_ref().map(|s| &s[..])
    }

    /// Get the short hash based only on the PackageId
    /// Used for the metadata when target_metadata returns None
    pub fn target_short_hash(&self, unit: &Unit) -> String {
        let hashable = unit.pkg.package_id().stable_hash(self.ws.root());
        util::short_hash(&hashable)
    }

    /// Get the metadata for a target in a specific profile
    /// We build to the path: "{filename}-{target_metadata}"
    /// We use a linking step to link/copy to a predictable filename
    /// like `target/debug/libfoo.{a,so,rlib}` and such.
    pub fn target_metadata(&mut self, unit: &Unit<'a>) -> Option<Metadata> {
        if let Some(cache) = self.target_metadatas.get(unit) {
            return cache.clone()
        }

        let metadata = self.calc_target_metadata(unit);
        self.target_metadatas.insert(*unit, metadata.clone());
        metadata
    }

    fn calc_target_metadata(&mut self, unit: &Unit<'a>) -> Option<Metadata> {
        // No metadata for dylibs because of a couple issues
        // - OSX encodes the dylib name in the executable
        // - Windows rustc multiple files of which we can't easily link all of them
        //
        // Two exceptions
        // 1) Upstream dependencies (we aren't exporting + need to resolve name conflict)
        // 2) __CARGO_DEFAULT_LIB_METADATA env var
        //
        // Note, though, that the compiler's build system at least wants
        // path dependencies (eg libstd) to have hashes in filenames. To account for
        // that we have an extra hack here which reads the
        // `__CARGO_DEFAULT_LIB_METADATA` environment variable and creates a
        // hash in the filename if that's present.
        //
        // This environment variable should not be relied on! It's
        // just here for rustbuild. We need a more principled method
        // doing this eventually.
        let __cargo_default_lib_metadata = env::var("__CARGO_DEFAULT_LIB_METADATA");
        if !unit.profile.test &&
            (unit.target.is_dylib() || unit.target.is_cdylib()) &&
            unit.pkg.package_id().source_id().is_path() &&
            !__cargo_default_lib_metadata.is_ok() {
            return None;
        }

        let mut hasher = SipHasher::new_with_keys(0, 0);

        // Unique metadata per (name, source, version) triple. This'll allow us
        // to pull crates from anywhere w/o worrying about conflicts
        unit.pkg.package_id().stable_hash(self.ws.root()).hash(&mut hasher);

        // Add package properties which map to environment variables
        // exposed by Cargo
        let manifest_metadata = unit.pkg.manifest().metadata();
        manifest_metadata.authors.hash(&mut hasher);
        manifest_metadata.description.hash(&mut hasher);
        manifest_metadata.homepage.hash(&mut hasher);

        // Also mix in enabled features to our metadata. This'll ensure that
        // when changing feature sets each lib is separately cached.
        self.resolve.features_sorted(unit.pkg.package_id()).hash(&mut hasher);

        // Mix in the target-metadata of all the dependencies of this target
        if let Ok(deps) = self.dep_targets(unit) {
            let mut deps_metadata = deps.into_iter().map(|dep_unit| {
                self.target_metadata(&dep_unit)
            }).collect::<Vec<_>>();
            deps_metadata.sort();
            deps_metadata.hash(&mut hasher);
        }

        // Throw in the profile we're compiling with. This helps caching
        // panic=abort and panic=unwind artifacts, additionally with various
        // settings like debuginfo and whatnot.
        unit.profile.hash(&mut hasher);

        // Artifacts compiled for the host should have a different metadata
        // piece than those compiled for the target, so make sure we throw in
        // the unit's `kind` as well
        unit.kind.hash(&mut hasher);

        // Finally throw in the target name/kind. This ensures that concurrent
        // compiles of targets in the same crate don't collide.
        unit.target.name().hash(&mut hasher);
        unit.target.kind().hash(&mut hasher);

        if let Ok(rustc) = self.config.rustc() {
            rustc.verbose_version.hash(&mut hasher);
        }

        // Seed the contents of __CARGO_DEFAULT_LIB_METADATA to the hasher if present.
        // This should be the release channel, to get a different hash for each channel.
        if let Ok(ref channel) = __cargo_default_lib_metadata {
            channel.hash(&mut hasher);
        }

        Some(Metadata(hasher.finish()))
    }

    /// Returns the file stem for a given target/profile combo (with metadata)
    pub fn file_stem(&mut self, unit: &Unit<'a>) -> String {
        match self.target_metadata(unit) {
            Some(ref metadata) => format!("{}-{}", unit.target.crate_name(),
                                          metadata),
            None => self.bin_stem(unit),
        }
    }

    /// Returns the bin stem for a given target (without metadata)
    fn bin_stem(&self, unit: &Unit) -> String {
        if unit.target.allows_underscores() {
            unit.target.name().to_string()
        } else {
            unit.target.crate_name()
        }
    }

    /// Returns a tuple with the directory and name of the hard link we expect
    /// our target to be copied to. Eg, file_stem may be out_dir/deps/foo-abcdef
    /// and link_stem would be out_dir/foo
    /// This function returns it in two parts so the caller can add prefix/suffix
    /// to filename separately

    /// Returns an Option because in some cases we don't want to link
    /// (eg a dependent lib)
    pub fn link_stem(&mut self, unit: &Unit<'a>) -> Option<(PathBuf, String)> {
        let src_dir = self.out_dir(unit);
        let bin_stem = self.bin_stem(unit);
        let file_stem = self.file_stem(unit);

        // We currently only lift files up from the `deps` directory. If
        // it was compiled into something like `example/` or `doc/` then
        // we don't want to link it up.
        if src_dir.ends_with("deps") {
            // Don't lift up library dependencies
            if self.ws.members().find(|&p| p == unit.pkg).is_none() &&
               !unit.target.is_bin() {
                None
            } else {
                Some((
                    src_dir.parent().unwrap().to_owned(),
                    if unit.profile.test {file_stem} else {bin_stem},
                ))
            }
        } else if bin_stem == file_stem {
            None
        } else if src_dir.ends_with("examples")
               || src_dir.parent().unwrap().ends_with("build") {
            Some((src_dir, bin_stem))
        } else {
            None
        }
    }

    /// Return the filenames that the given target for the given profile will
    /// generate as a list of 3-tuples (filename, link_dst, linkable)
    /// filename: filename rustc compiles to. (Often has metadata suffix).
    /// link_dst: Optional file to link/copy the result to (without metadata suffix)
    /// linkable: Whether possible to link against file (eg it's a library)
    pub fn target_filenames(&mut self, unit: &Unit<'a>)
                            -> CargoResult<Arc<Vec<(PathBuf, Option<PathBuf>, bool)>>> {
        if let Some(cache) = self.target_filenames.get(unit) {
            return Ok(Arc::clone(cache))
        }

        let result = self.calc_target_filenames(unit);
        if let Ok(ref ret) = result {
            self.target_filenames.insert(*unit, Arc::clone(ret));
        }
        result
    }

    fn calc_target_filenames(&mut self, unit: &Unit<'a>)
                            -> CargoResult<Arc<Vec<(PathBuf, Option<PathBuf>, bool)>>> {
        let out_dir = self.out_dir(unit);
        let stem = self.file_stem(unit);
        let link_stem = self.link_stem(unit);
        let info = if unit.target.for_host() {
            &self.host_info
        } else {
            &self.target_info
        };

        let mut ret = Vec::new();
        let mut unsupported = Vec::new();
        {
            if unit.profile.check {
                let filename = out_dir.join(format!("lib{}.rmeta", stem));
                let link_dst = link_stem.clone().map(|(ld, ls)| {
                    ld.join(format!("lib{}.rmeta", ls))
                });
                ret.push((filename, link_dst, true));
            } else {
                let mut add = |crate_type: &str, linkable: bool| -> CargoResult<()> {
                    let crate_type = if crate_type == "lib" {"rlib"} else {crate_type};
                    let mut crate_types = info.crate_types.borrow_mut();
                    let entry = crate_types.entry(crate_type.to_string());
                    let crate_type_info = match entry {
                        Entry::Occupied(o) => &*o.into_mut(),
                        Entry::Vacant(v) => {
                            let value = info.discover_crate_type(v.key())?;
                            &*v.insert(value)
                        }
                    };
                    match *crate_type_info {
                        Some((ref prefix, ref suffix)) => {
                            let filename = out_dir.join(format!("{}{}{}", prefix, stem, suffix));
                            let link_dst = link_stem.clone().map(|(ld, ls)| {
                                ld.join(format!("{}{}{}", prefix, ls, suffix))
                            });
                            ret.push((filename, link_dst, linkable));
                            Ok(())
                        }
                        // not supported, don't worry about it
                        None => {
                            unsupported.push(crate_type.to_string());
                            Ok(())
                        }
                    }
                };

                match *unit.target.kind() {
                    TargetKind::Bin |
                    TargetKind::CustomBuild |
                    TargetKind::ExampleBin |
                    TargetKind::Bench |
                    TargetKind::Test => {
                        add("bin", false)?;
                    }
                    TargetKind::Lib(..) |
                    TargetKind::ExampleLib(..)
                    if unit.profile.test => {
                        add("bin", false)?;
                    }
                    TargetKind::ExampleLib(ref kinds) |
                    TargetKind::Lib(ref kinds) => {
                        for kind in kinds {
                            add(kind.crate_type(), kind.linkable())?;
                        }
                    }
                }
            }
        }
        if ret.is_empty() {
            if !unsupported.is_empty() {
                bail!("cannot produce {} for `{}` as the target `{}` \
                       does not support these crate types",
                      unsupported.join(", "), unit.pkg, self.target_triple())
            }
            bail!("cannot compile `{}` as the target `{}` does not \
                   support any of the output crate types",
                  unit.pkg, self.target_triple());
        }
        info!("Target filenames: {:?}", ret);

        Ok(Arc::new(ret))
    }

    /// For a package, return all targets which are registered as dependencies
    /// for that package.
    pub fn dep_targets(&self, unit: &Unit<'a>) -> CargoResult<Vec<Unit<'a>>> {
        if unit.profile.run_custom_build {
            return self.dep_run_custom_build(unit)
        } else if unit.profile.doc && !unit.profile.test {
            return self.doc_deps(unit);
        }

        let id = unit.pkg.package_id();
        let deps = self.resolve.deps(id);
        let mut ret = deps.filter(|dep| {
            unit.pkg.dependencies().iter().filter(|d| {
                d.name() == dep.name() && d.version_req().matches(dep.version())
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
                if d.is_optional() && !self.resolve.features(id).contains(d.name()) {
                    return false;
                }

                // If we've gotten past all that, then this dependency is
                // actually used!
                true
            })
        }).filter_map(|id| {
            match self.get_package(id) {
                Ok(pkg) => {
                    pkg.targets().iter().find(|t| t.is_lib()).map(|t| {
                        let unit = Unit {
                            pkg: pkg,
                            target: t,
                            profile: self.lib_or_check_profile(unit, t),
                            kind: unit.kind.for_target(t),
                        };
                        Ok(unit)
                    })
                }
                Err(e) => Some(Err(e))
            }
        }).collect::<CargoResult<Vec<_>>>()?;

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
        if unit.target.is_lib() && !unit.profile.doc {
            return Ok(ret)
        }
        ret.extend(self.maybe_lib(unit));

        // Integration tests/benchmarks require binaries to be built
        if unit.profile.test &&
           (unit.target.is_test() || unit.target.is_bench()) {
            ret.extend(unit.pkg.targets().iter().filter(|t| {
                let no_required_features = Vec::new();

                t.is_bin() &&
                // Skip binaries with required features that have not been selected.
                t.required_features().unwrap_or(&no_required_features).iter().all(|f| {
                    self.resolve.features(id).contains(f)
                })
            }).map(|t| {
                Unit {
                    pkg: unit.pkg,
                    target: t,
                    profile: self.lib_profile(),
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
        if self.build_script_overridden.contains(&key) {
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
        let deps = self.dep_targets(&tmp)?;
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
            let dep = dep?;
            let lib = match dep.targets().iter().find(|t| t.is_lib()) {
                Some(lib) => lib,
                None => continue,
            };
            ret.push(Unit {
                pkg: dep,
                target: lib,
                profile: self.lib_profile(),
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
                profile: self.lib_or_check_profile(unit, t),
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

    /// Get the list of cfg printed out from the compiler for the specified kind
    pub fn cfg(&self, kind: Kind) -> &[Cfg] {
        let info = match kind {
            Kind::Host => &self.host_info,
            Kind::Target => &self.target_info,
        };
        info.cfg.as_ref().map(|s| &s[..]).unwrap_or(&[])
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

    pub fn lib_profile(&self) -> &'a Profile {
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

    pub fn lib_or_check_profile(&self, unit: &Unit, target: &Target) -> &'a Profile {
        if unit.profile.check && !target.is_custom_build() && !target.for_host() {
            &self.profiles.check
        } else {
            self.lib_profile()
        }
    }

    pub fn build_script_profile(&self, _pkg: &PackageId) -> &'a Profile {
        // TODO: should build scripts always be built with the same library
        //       profile? How is this controlled at the CLI layer?
        self.lib_profile()
    }

    pub fn incremental_args(&self, unit: &Unit) -> CargoResult<Vec<String>> {
        if self.incremental_enabled {
            if unit.pkg.package_id().source_id().is_path() {
                // Only enable incremental compilation for sources the user can modify.
                // For things that change infrequently, non-incremental builds yield
                // better performance.
                // (see also https://github.com/rust-lang/cargo/issues/3972)
                return Ok(vec![format!("-Zincremental={}",
                                       self.layout(unit.kind).incremental().display())]);
            } else if unit.profile.codegen_units.is_none() {
                // For non-incremental builds we set a higher number of
                // codegen units so we get faster compiles. It's OK to do
                // so because the user has already opted into slower
                // runtime code by setting CARGO_INCREMENTAL.
                return Ok(vec![format!("-Ccodegen-units={}", ::num_cpus::get())]);
            }
        }

        Ok(vec![])
    }

    pub fn rustflags_args(&self, unit: &Unit) -> CargoResult<Vec<String>> {
        env_args(self.config, &self.build_config, self.info(&unit.kind), unit.kind, "RUSTFLAGS")
    }

    pub fn rustdocflags_args(&self, unit: &Unit) -> CargoResult<Vec<String>> {
        env_args(self.config, &self.build_config, self.info(&unit.kind), unit.kind, "RUSTDOCFLAGS")
    }

    pub fn show_warnings(&self, pkg: &PackageId) -> bool {
        pkg.source_id().is_path() || self.config.extra_verbose()
    }

    fn info(&self, kind: &Kind) -> &TargetInfo {
        match *kind {
            Kind::Host => &self.host_info,
            Kind::Target => &self.target_info,
        }
    }
}

// Acquire extra flags to pass to the compiler from the
// RUSTFLAGS environment variable and similar config values
fn env_args(config: &Config,
            build_config: &BuildConfig,
            target_info: &TargetInfo,
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
    if let Ok(a) = env::var(name) {
        let args = a.split(' ')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        return Ok(args.collect());
    }

    let mut rustflags = Vec::new();

    let name = name.chars().flat_map(|c| c.to_lowercase()).collect::<String>();
    // Then the target.*.rustflags value...
    let target = build_config.requested_target.as_ref().unwrap_or(&build_config.host_triple);
    let key = format!("target.{}.{}", target, name);
    if let Some(args) = config.get_list_or_split_string(&key)? {
        let args = args.val.into_iter();
        rustflags.extend(args);
    }
    // ...including target.'cfg(...)'.rustflags
    if let Some(ref target_cfg) = target_info.cfg {
        if let Some(table) = config.get_table("target")? {
            let cfgs = table.val.keys().filter_map(|t| {
                if t.starts_with("cfg(") && t.ends_with(')') {
                    let cfg = &t[4..t.len() - 1];
                    CfgExpr::from_str(cfg)
                        .ok()
                        .and_then(|c| if c.matches(target_cfg) { Some(t) } else { None })
                } else {
                    None
                }
            });
            for n in cfgs {
                let key = format!("target.{}.{}", n, name);
                if let Some(args) = config.get_list_or_split_string(&key)? {
                    let args = args.val.into_iter();
                    rustflags.extend(args);
                }
            }
        }
    }

    if !rustflags.is_empty() {
        return Ok(rustflags);
    }

    // Then the build.rustflags value
    let key = format!("build.{}", name);
    if let Some(args) = config.get_list_or_split_string(&key)? {
        let args = args.val.into_iter();
        return Ok(args.collect());
    }

    Ok(Vec::new())
}

impl fmt::Display for Metadata {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

fn parse_crate_type(
    crate_type: &str,
    error: &str,
    lines: &mut str::Lines,
) -> CargoResult<Option<(String, String)>> {
    let not_supported = error.lines().any(|line| {
        (line.contains("unsupported crate type") ||
         line.contains("unknown crate type")) &&
        line.contains(crate_type)
    });
    if not_supported {
        return Ok(None);
    }
    let line = match lines.next() {
        Some(line) => line,
        None => bail!("malformed output when learning about \
            crate-type {} information", crate_type),
    };
    let mut parts = line.trim().split("___");
    let prefix = parts.next().unwrap();
    let suffix = match parts.next() {
        Some(part) => part,
        None => bail!("output of --print=file-names has changed in \
            the compiler, cannot parse"),
    };

    Ok(Some((prefix.to_string(), suffix.to_string())))
}
