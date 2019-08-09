use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str;

use log::debug;

use crate::core::compiler::unit::UnitInterner;
use crate::core::compiler::{BuildConfig, BuildOutput, Kind, Unit};
use crate::core::profiles::Profiles;
use crate::core::{Dependency, Workspace};
use crate::core::{PackageId, PackageSet, Resolve};
use crate::util::errors::CargoResult;
use crate::util::{profile, Cfg, Config, Rustc};

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
    /// The dependency graph for our build.
    pub resolve: &'a Resolve,
    pub profiles: &'a Profiles,
    pub build_config: &'a BuildConfig,
    /// Extra compiler args for either `rustc` or `rustdoc`.
    pub extra_compiler_args: HashMap<Unit<'a>, Vec<String>>,
    pub packages: &'a PackageSet<'cfg>,

    /// Information about the compiler.
    pub rustc: Rustc,
    /// Build information for the host arch.
    pub host_config: TargetConfig,
    /// Build information for the target.
    pub target_config: TargetConfig,
    pub target_info: TargetInfo,
    pub host_info: TargetInfo,
    pub units: &'a UnitInterner<'a>,
}

impl<'a, 'cfg> BuildContext<'a, 'cfg> {
    pub fn new(
        ws: &'a Workspace<'cfg>,
        resolve: &'a Resolve,
        packages: &'a PackageSet<'cfg>,
        config: &'cfg Config,
        build_config: &'a BuildConfig,
        profiles: &'a Profiles,
        units: &'a UnitInterner<'a>,
        extra_compiler_args: HashMap<Unit<'a>, Vec<String>>,
    ) -> CargoResult<BuildContext<'a, 'cfg>> {
        let rustc = config.load_global_rustc(Some(ws))?;

        let host_config = TargetConfig::new(config, &rustc.host)?;
        let target_config = match build_config.requested_target.as_ref() {
            Some(triple) => TargetConfig::new(config, triple)?,
            None => host_config.clone(),
        };
        let (host_info, target_info) = {
            let _p = profile::start("BuildContext::probe_target_info");
            debug!("probe_target_info");
            let host_info =
                TargetInfo::new(config, &build_config.requested_target, &rustc, Kind::Host)?;
            let target_info =
                TargetInfo::new(config, &build_config.requested_target, &rustc, Kind::Target)?;
            (host_info, target_info)
        };

        Ok(BuildContext {
            ws,
            resolve,
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

    pub fn extern_crate_name(&self, unit: &Unit<'a>, dep: &Unit<'a>) -> CargoResult<String> {
        self.resolve
            .extern_crate_name(unit.pkg.package_id(), dep.pkg.package_id(), dep.target)
    }

    pub fn is_public_dependency(&self, unit: &Unit<'a>, dep: &Unit<'a>) -> bool {
        self.resolve
            .is_public_dep(unit.pkg.package_id(), dep.pkg.package_id())
    }

    /// Whether a dependency should be compiled for the host or target platform,
    /// specified by `Kind`.
    pub fn dep_platform_activated(&self, dep: &Dependency, kind: Kind) -> bool {
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
        platform.matches(name, info.cfg())
    }

    /// Gets the user-specified linker for a particular host or target.
    pub fn linker(&self, kind: Kind) -> Option<&Path> {
        self.target_config(kind).linker.as_ref().map(|s| s.as_ref())
    }

    /// Gets the user-specified `ar` program for a particular host or target.
    pub fn ar(&self, kind: Kind) -> Option<&Path> {
        self.target_config(kind).ar.as_ref().map(|s| s.as_ref())
    }

    /// Gets the list of `cfg`s printed out from the compiler for the specified kind.
    pub fn cfg(&self, kind: Kind) -> &[Cfg] {
        let info = match kind {
            Kind::Host => &self.host_info,
            Kind::Target => &self.target_info,
        };
        info.cfg()
    }

    /// Gets the host architecture triple.
    ///
    /// For example, x86_64-unknown-linux-gnu, would be
    /// - machine: x86_64,
    /// - hardware-platform: unknown,
    /// - operating system: linux-gnu.
    pub fn host_triple(&self) -> &str {
        &self.rustc.host
    }

    pub fn target_triple(&self) -> &str {
        self.build_config
            .requested_target
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or_else(|| self.host_triple())
    }

    /// Gets the target configuration for a particular host or target.
    fn target_config(&self, kind: Kind) -> &TargetConfig {
        match kind {
            Kind::Host => &self.host_config,
            Kind::Target => &self.target_config,
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

    fn info(&self, kind: Kind) -> &TargetInfo {
        match kind {
            Kind::Host => &self.host_info,
            Kind::Target => &self.target_info,
        }
    }

    pub fn extra_args_for(&self, unit: &Unit<'a>) -> Option<&Vec<String>> {
        self.extra_compiler_args.get(unit)
    }

    /// If a build script is overridden, this returns the `BuildOutput` to use.
    ///
    /// `lib_name` is the `links` library name and `kind` is whether it is for
    /// Host or Target.
    pub fn script_override(&self, lib_name: &str, kind: Kind) -> Option<&BuildOutput> {
        match kind {
            Kind::Host => self.host_config.overrides.get(lib_name),
            Kind::Target => self.target_config.overrides.get(lib_name),
        }
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
