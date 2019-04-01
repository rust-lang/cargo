use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::str;

use log::debug;

use crate::core::profiles::Profiles;
use crate::core::{Dependency, Workspace};
use crate::core::{PackageId, PackageSet, Resolve};
use crate::util::errors::CargoResult;
use crate::util::{profile, Cfg, CfgExpr, Config, Rustc};

use super::{BuildConfig, BuildOutput, Kind, Unit};

mod target_info;
pub use self::target_info::{FileFlavor, TargetInfo};

/// The build context, containing all information about a build task.
pub struct BuildContext<'a, 'cfg: 'a> {
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
}

impl<'a, 'cfg> BuildContext<'a, 'cfg> {
    pub fn new(
        ws: &'a Workspace<'cfg>,
        resolve: &'a Resolve,
        packages: &'a PackageSet<'cfg>,
        config: &'cfg Config,
        build_config: &'a BuildConfig,
        profiles: &'a Profiles,
        extra_compiler_args: HashMap<Unit<'a>, Vec<String>>,
    ) -> CargoResult<BuildContext<'a, 'cfg>> {
        let mut rustc = config.load_global_rustc(Some(ws))?;
        if let Some(wrapper) = &build_config.rustc_wrapper {
            rustc.set_wrapper(wrapper.clone());
        }

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
        })
    }

    pub fn extern_crate_name(&self, unit: &Unit<'a>, dep: &Unit<'a>) -> CargoResult<String> {
        self.resolve
            .extern_crate_name(unit.pkg.package_id(), dep.pkg.package_id(), dep.target)
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
        info.cfg().unwrap_or(&[])
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

    pub fn rustflags_args(&self, unit: &Unit<'_>) -> CargoResult<Vec<String>> {
        env_args(
            self.config,
            &self.build_config.requested_target,
            self.host_triple(),
            self.info(unit.kind).cfg(),
            unit.kind,
            "RUSTFLAGS",
        )
    }

    pub fn rustdocflags_args(&self, unit: &Unit<'_>) -> CargoResult<Vec<String>> {
        env_args(
            self.config,
            &self.build_config.requested_target,
            self.host_triple(),
            self.info(unit.kind).cfg(),
            unit.kind,
            "RUSTDOCFLAGS",
        )
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
}

/// Information required to build for a target.
#[derive(Clone, Default)]
pub struct TargetConfig {
    /// The path of archiver (lib builder) for this target.
    pub ar: Option<PathBuf>,
    /// The path of the linker for this target.
    pub linker: Option<PathBuf>,
    /// Special build options for any necessary input files (filename -> options).
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
                        output
                            .linker_args
                            .extend(args.iter().map(|v| v.0.clone()));
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

/// Acquire extra flags to pass to the compiler from various locations.
///
/// The locations are:
///
///  - the `RUSTFLAGS` environment variable
///
/// then if this was not found
///
///  - `target.*.rustflags` from the manifest (Cargo.toml)
///  - `target.cfg(..).rustflags` from the manifest
///
/// then if neither of these were found
///
///  - `build.rustflags` from the manifest
///
/// Note that if a `target` is specified, no args will be passed to host code (plugins, build
/// scripts, ...), even if it is the same as the target.
fn env_args(
    config: &Config,
    requested_target: &Option<String>,
    host_triple: &str,
    target_cfg: Option<&[Cfg]>,
    kind: Kind,
    name: &str,
) -> CargoResult<Vec<String>> {
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
    // This means that, e.g., even if the specified --target is the
    // same as the host, build scripts in plugins won't get
    // RUSTFLAGS.
    let compiling_with_target = requested_target.is_some();
    let is_target_kind = kind == Kind::Target;

    if compiling_with_target && !is_target_kind {
        // This is probably a build script or plugin and we're
        // compiling with --target. In this scenario there are
        // no rustflags we can apply.
        return Ok(Vec::new());
    }

    // First try RUSTFLAGS from the environment
    if let Ok(a) = env::var(name) {
        let args = a
            .split(' ')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        return Ok(args.collect());
    }

    let mut rustflags = Vec::new();

    let name = name
        .chars()
        .flat_map(|c| c.to_lowercase())
        .collect::<String>();
    // Then the target.*.rustflags value...
    let target = requested_target
        .as_ref()
        .map(|s| s.as_str())
        .unwrap_or(host_triple);
    let key = format!("target.{}.{}", target, name);
    if let Some(args) = config.get_list_or_split_string(&key)? {
        let args = args.val.into_iter();
        rustflags.extend(args);
    }
    // ...including target.'cfg(...)'.rustflags
    if let Some(target_cfg) = target_cfg {
        if let Some(table) = config.get_table("target")? {
            let cfgs = table
                .val
                .keys()
                .filter(|key| CfgExpr::matches_key(key, target_cfg));

            // Note that we may have multiple matching `[target]` sections and
            // because we're passing flags to the compiler this can affect
            // cargo's caching and whether it rebuilds. Ensure a deterministic
            // ordering through sorting for now. We may perhaps one day wish to
            // ensure a deterministic ordering via the order keys were defined
            // in files perhaps.
            let mut cfgs = cfgs.collect::<Vec<_>>();
            cfgs.sort();

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

    // Then the `build.rustflags` value.
    let key = format!("build.{}", name);
    if let Some(args) = config.get_list_or_split_string(&key)? {
        let args = args.val.into_iter();
        return Ok(args.collect());
    }

    Ok(Vec::new())
}
