use std::env;
use std::path::Path;
use std::str::{self, FromStr};

use core::profiles::Profiles;
use core::{Dependency, Workspace};
use core::{Package, PackageId, PackageSet, Resolve};
use util::errors::CargoResult;
use util::{profile, Cfg, CfgExpr, Config};

use super::{BuildConfig, Kind, TargetConfig, Unit};

mod target_info;
pub use self::target_info::{FileFlavor, TargetInfo};

/// The build context, containing all information about a build task
pub struct BuildContext<'a, 'cfg: 'a> {
    /// The workspace the build is for
    pub ws: &'a Workspace<'cfg>,
    /// The cargo configuration
    pub config: &'cfg Config,
    /// The dependency graph for our build
    pub resolve: &'a Resolve,
    pub profiles: &'a Profiles,
    pub build_config: &'a BuildConfig,
    /// This is a workaround to carry the extra compiler args for either
    /// `rustc` or `rustdoc` given on the command-line for the commands `cargo
    /// rustc` and `cargo rustdoc`.  These commands only support one target,
    /// but we don't want the args passed to any dependencies, so we include
    /// the `Unit` corresponding to the top-level target.
    pub extra_compiler_args: Option<(Unit<'a>, Vec<String>)>,
    pub packages: &'a PackageSet<'cfg>,

    pub target_info: TargetInfo,
    pub host_info: TargetInfo,
    pub incremental_env: Option<bool>,
}

impl<'a, 'cfg> BuildContext<'a, 'cfg> {
    pub fn new(
        ws: &'a Workspace<'cfg>,
        resolve: &'a Resolve,
        packages: &'a PackageSet<'cfg>,
        config: &'cfg Config,
        build_config: &'a BuildConfig,
        profiles: &'a Profiles,
        extra_compiler_args: Option<(Unit<'a>, Vec<String>)>,
    ) -> CargoResult<BuildContext<'a, 'cfg>> {
        let incremental_env = match env::var("CARGO_INCREMENTAL") {
            Ok(v) => Some(v == "1"),
            Err(_) => None,
        };

        let (host_info, target_info) = {
            let _p = profile::start("BuildContext::probe_target_info");
            debug!("probe_target_info");
            let host_info = TargetInfo::new(config, &build_config, Kind::Host)?;
            let target_info = TargetInfo::new(config, &build_config, Kind::Target)?;
            (host_info, target_info)
        };

        Ok(BuildContext {
            ws,
            resolve,
            packages,
            config,
            target_info,
            host_info,
            build_config,
            profiles,
            incremental_env,
            extra_compiler_args,
        })
    }

    pub fn extern_crate_name(&self, unit: &Unit<'a>, dep: &Unit<'a>) -> CargoResult<String> {
        let deps = {
            let a = unit.pkg.package_id();
            let b = dep.pkg.package_id();
            if a == b {
                &[]
            } else {
                self.resolve.dependencies_listed(a, b)
            }
        };

        let crate_name = dep.target.crate_name();
        let mut names = deps.iter()
            .map(|d| d.rename().unwrap_or(&crate_name));
        let name = names.next().unwrap_or(&crate_name);
        for n in names {
            if n == name {
                continue
            }
            bail!("multiple dependencies listed for the same crate must \
                   all have the same name, but the dependency on `{}` \
                   is listed as having different names", dep.pkg.package_id());
        }
        Ok(name.to_string())
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
            Kind::Host => (self.build_config.host_triple(), &self.host_info),
            Kind::Target => (self.build_config.target_triple(), &self.target_info),
        };
        platform.matches(name, info.cfg())
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
        info.cfg().unwrap_or(&[])
    }

    /// Get the target configuration for a particular host or target
    fn target_config(&self, kind: Kind) -> &TargetConfig {
        match kind {
            Kind::Host => &self.build_config.host,
            Kind::Target => &self.build_config.target,
        }
    }

    /// Number of jobs specified for this build
    pub fn jobs(&self) -> u32 {
        self.build_config.jobs
    }

    pub fn rustflags_args(&self, unit: &Unit) -> CargoResult<Vec<String>> {
        env_args(
            self.config,
            &self.build_config,
            self.info(&unit.kind).cfg(),
            unit.kind,
            "RUSTFLAGS",
        )
    }

    pub fn rustdocflags_args(&self, unit: &Unit) -> CargoResult<Vec<String>> {
        env_args(
            self.config,
            &self.build_config,
            self.info(&unit.kind).cfg(),
            unit.kind,
            "RUSTDOCFLAGS",
        )
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

    pub fn extra_args_for(&self, unit: &Unit<'a>) -> Option<&Vec<String>> {
        if let Some((ref args_unit, ref args)) = self.extra_compiler_args {
            if args_unit == unit {
                return Some(args);
            }
        }
        None
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
    build_config: &BuildConfig,
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

    let name = name.chars()
        .flat_map(|c| c.to_lowercase())
        .collect::<String>();
    // Then the target.*.rustflags value...
    let target = build_config
        .requested_target
        .as_ref()
        .map(|s| s.as_str())
        .unwrap_or(build_config.host_triple());
    let key = format!("target.{}.{}", target, name);
    if let Some(args) = config.get_list_or_split_string(&key)? {
        let args = args.val.into_iter();
        rustflags.extend(args);
    }
    // ...including target.'cfg(...)'.rustflags
    if let Some(target_cfg) = target_cfg {
        if let Some(table) = config.get_table("target")? {
            let cfgs = table.val.keys().filter_map(|t| {
                if t.starts_with("cfg(") && t.ends_with(')') {
                    let cfg = &t[4..t.len() - 1];
                    CfgExpr::from_str(cfg).ok().and_then(|c| {
                        if c.matches(target_cfg) {
                            Some(t)
                        } else {
                            None
                        }
                    })
                } else {
                    None
                }
            });

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

    // Then the build.rustflags value
    let key = format!("build.{}", name);
    if let Some(args) = config.get_list_or_split_string(&key)? {
        let args = args.val.into_iter();
        return Ok(args.collect());
    }

    Ok(Vec::new())
}
