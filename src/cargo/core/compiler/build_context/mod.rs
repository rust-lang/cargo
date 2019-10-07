use crate::core::compiler::unit::UnitInterner;
use crate::core::compiler::CompileTarget;
use crate::core::compiler::{BuildConfig, BuildOutput, CompileKind, Unit};
use crate::core::profiles::Profiles;
use crate::core::{Dependency, InternedString, Workspace};
use crate::core::{PackageId, PackageSet};
use crate::util::errors::CargoResult;
use crate::util::{Config, Rustc};
use cargo_platform::Cfg;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str;

mod target_info;
pub use self::target_info::{FileFlavor, TargetInfo};

/// The build context, containing all information about a build task.
///
/// It is intended that this is mostly static information. Stuff that mutates
/// during the build can be found in the parent `Context`. (I say mostly,
/// because this has internal caching, but nothing that should be observable
/// or require &mut.)
pub struct BuildContext<'a, 'cfg> {
    /// The workspace the build is for.
    pub ws: &'a Workspace<'cfg>,
    /// The cargo configuration.
    pub config: &'cfg Config,
    pub profiles: &'a Profiles,
    pub build_config: &'a BuildConfig,
    /// Extra compiler args for either `rustc` or `rustdoc`.
    pub extra_compiler_args: HashMap<Unit<'a>, Vec<String>>,
    pub packages: &'a PackageSet<'cfg>,

    /// Source of interning new units as they're created.
    pub units: &'a UnitInterner<'a>,

    /// Information about the compiler that we've detected on the local system.
    pub rustc: Rustc,

    /// Build information for the "host", which is information about when
    /// `rustc` is invoked without a `--target` flag. This is used for
    /// procedural macros, build scripts, etc.
    host_config: TargetConfig,
    host_info: TargetInfo,

    /// Build information for targets that we're building for. This will be
    /// empty if the `--target` flag is not passed, and currently also only ever
    /// has at most one entry, but eventually we'd like to support multi-target
    /// builds with Cargo.
    target_config: HashMap<CompileTarget, TargetConfig>,
    target_info: HashMap<CompileTarget, TargetInfo>,
}

impl<'a, 'cfg> BuildContext<'a, 'cfg> {
    pub fn new(
        ws: &'a Workspace<'cfg>,
        packages: &'a PackageSet<'cfg>,
        config: &'cfg Config,
        build_config: &'a BuildConfig,
        profiles: &'a Profiles,
        units: &'a UnitInterner<'a>,
        extra_compiler_args: HashMap<Unit<'a>, Vec<String>>,
    ) -> CargoResult<BuildContext<'a, 'cfg>> {
        let rustc = config.load_global_rustc(Some(ws))?;

        let host_config = TargetConfig::new(config, &rustc.host)?;
        let host_info = TargetInfo::new(
            config,
            build_config.requested_kind,
            &rustc,
            CompileKind::Host,
        )?;
        let mut target_config = HashMap::new();
        let mut target_info = HashMap::new();
        if let CompileKind::Target(target) = build_config.requested_kind {
            target_config.insert(target, TargetConfig::new(config, target.short_name())?);
            target_info.insert(
                target,
                TargetInfo::new(
                    config,
                    build_config.requested_kind,
                    &rustc,
                    CompileKind::Target(target),
                )?,
            );
        }

        Ok(BuildContext {
            ws,
            packages,
            config,
            rustc,
            target_config,
            target_info,
            host_config,
            host_info,
            build_config,
            profiles,
            extra_compiler_args,
            units,
        })
    }

    /// Whether a dependency should be compiled for the host or target platform,
    /// specified by `CompileKind`.
    pub fn dep_platform_activated(&self, dep: &Dependency, kind: CompileKind) -> bool {
        // If this dependency is only available for certain platforms,
        // make sure we're only enabling it for that platform.
        let platform = match dep.platform() {
            Some(p) => p,
            None => return true,
        };
        let name = kind.short_name(self);
        platform.matches(name, self.cfg(kind))
    }

    /// Gets the user-specified linker for a particular host or target.
    pub fn linker(&self, kind: CompileKind) -> Option<&Path> {
        self.target_config(kind).linker.as_ref().map(|s| s.as_ref())
    }

    /// Gets the user-specified `ar` program for a particular host or target.
    pub fn ar(&self, kind: CompileKind) -> Option<&Path> {
        self.target_config(kind).ar.as_ref().map(|s| s.as_ref())
    }

    /// Gets the list of `cfg`s printed out from the compiler for the specified kind.
    pub fn cfg(&self, kind: CompileKind) -> &[Cfg] {
        self.info(kind).cfg()
    }

    /// Gets the host architecture triple.
    ///
    /// For example, x86_64-unknown-linux-gnu, would be
    /// - machine: x86_64,
    /// - hardware-platform: unknown,
    /// - operating system: linux-gnu.
    pub fn host_triple(&self) -> InternedString {
        self.rustc.host
    }

    /// Gets the target configuration for a particular host or target.
    pub fn target_config(&self, kind: CompileKind) -> &TargetConfig {
        match kind {
            CompileKind::Host => &self.host_config,
            CompileKind::Target(s) => &self.target_config[&s],
        }
    }

    /// Gets the number of jobs specified for this build.
    pub fn jobs(&self) -> u32 {
        self.build_config.jobs
    }

    pub fn rustflags_args(&self, unit: &Unit<'_>) -> &[String] {
        &self.info(unit.kind).rustflags
    }

    pub fn rustdocflags_args(&self, unit: &Unit<'_>) -> &[String] {
        &self.info(unit.kind).rustdocflags
    }

    pub fn show_warnings(&self, pkg: PackageId) -> bool {
        pkg.source_id().is_path() || self.config.extra_verbose()
    }

    pub fn info(&self, kind: CompileKind) -> &TargetInfo {
        match kind {
            CompileKind::Host => &self.host_info,
            CompileKind::Target(s) => &self.target_info[&s],
        }
    }

    pub fn extra_args_for(&self, unit: &Unit<'a>) -> Option<&Vec<String>> {
        self.extra_compiler_args.get(unit)
    }

    /// If a build script is overridden, this returns the `BuildOutput` to use.
    ///
    /// `lib_name` is the `links` library name and `kind` is whether it is for
    /// Host or Target.
    pub fn script_override(&self, lib_name: &str, kind: CompileKind) -> Option<&BuildOutput> {
        self.target_config(kind).overrides.get(lib_name)
    }
}

/// Information required to build for a target.
#[derive(Clone, Default)]
pub struct TargetConfig {
    /// The path of archiver (lib builder) for this target.
    pub ar: Option<PathBuf>,
    /// The path of the linker for this target.
    pub linker: Option<PathBuf>,
    /// Build script override for the given library name.
    ///
    /// Any package with a `links` value for the given library name will skip
    /// running its build script and instead use the given output from the
    /// config file.
    pub overrides: HashMap<String, BuildOutput>,
}

impl TargetConfig {
    pub fn new(config: &Config, triple: &str) -> CargoResult<TargetConfig> {
        let key = format!("target.{}", triple);
        let mut ret = TargetConfig {
            ar: config.get_path(&format!("{}.ar", key))?.map(|v| v.val),
            linker: config.get_path(&format!("{}.linker", key))?.map(|v| v.val),
            overrides: HashMap::new(),
        };
        let table = match config.get_table(&key)? {
            Some(table) => table.val,
            None => return Ok(ret),
        };
        for (lib_name, value) in table {
            match lib_name.as_str() {
                "ar" | "linker" | "runner" | "rustflags" => continue,
                _ => {}
            }

            let mut output = BuildOutput {
                library_paths: Vec::new(),
                library_links: Vec::new(),
                linker_args: Vec::new(),
                cfgs: Vec::new(),
                env: Vec::new(),
                metadata: Vec::new(),
                rerun_if_changed: Vec::new(),
                rerun_if_env_changed: Vec::new(),
                warnings: Vec::new(),
            };
            // We require deterministic order of evaluation, so we must sort the pairs by key first.
            let mut pairs = Vec::new();
            for (k, value) in value.table(&lib_name)?.0 {
                pairs.push((k, value));
            }
            pairs.sort_by_key(|p| p.0);
            for (k, value) in pairs {
                let key = format!("{}.{}", key, k);
                match &k[..] {
                    "rustc-flags" => {
                        let (flags, definition) = value.string(k)?;
                        let whence = format!("in `{}` (in {})", key, definition.display());
                        let (paths, links) = BuildOutput::parse_rustc_flags(flags, &whence)?;
                        output.library_paths.extend(paths);
                        output.library_links.extend(links);
                    }
                    "rustc-link-lib" => {
                        let list = value.list(k)?;
                        output
                            .library_links
                            .extend(list.iter().map(|v| v.0.clone()));
                    }
                    "rustc-link-search" => {
                        let list = value.list(k)?;
                        output
                            .library_paths
                            .extend(list.iter().map(|v| PathBuf::from(&v.0)));
                    }
                    "rustc-cdylib-link-arg" => {
                        let args = value.list(k)?;
                        output.linker_args.extend(args.iter().map(|v| v.0.clone()));
                    }
                    "rustc-cfg" => {
                        let list = value.list(k)?;
                        output.cfgs.extend(list.iter().map(|v| v.0.clone()));
                    }
                    "rustc-env" => {
                        for (name, val) in value.table(k)?.0 {
                            let val = val.string(name)?.0;
                            output.env.push((name.clone(), val.to_string()));
                        }
                    }
                    "warning" | "rerun-if-changed" | "rerun-if-env-changed" => {
                        failure::bail!("`{}` is not supported in build script overrides", k);
                    }
                    _ => {
                        let val = value.string(k)?.0;
                        output.metadata.push((k.clone(), val.to_string()));
                    }
                }
            }
            ret.overrides.insert(lib_name, output);
        }

        Ok(ret)
    }
}
