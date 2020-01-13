use crate::core::compiler::unit::UnitInterner;
use crate::core::compiler::CompileTarget;
use crate::core::compiler::{BuildConfig, BuildOutput, CompileKind, Unit};
use crate::core::profiles::Profiles;
use crate::core::{Dependency, InternedString, Workspace};
use crate::core::{PackageId, PackageSet};
use crate::util::config::{Config, TargetConfig};
use crate::util::errors::CargoResult;
use crate::util::Rustc;
use cargo_platform::Cfg;
use std::collections::HashMap;
use std::path::PathBuf;
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
    pub profiles: Profiles,
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
        profiles: Profiles,
        units: &'a UnitInterner<'a>,
        extra_compiler_args: HashMap<Unit<'a>, Vec<String>>,
    ) -> CargoResult<BuildContext<'a, 'cfg>> {
        let rustc = config.load_global_rustc(Some(ws))?;

        let host_config = config.target_cfg_triple(&rustc.host)?;
        let host_info = TargetInfo::new(
            config,
            build_config.requested_kind,
            &rustc,
            CompileKind::Host,
        )?;
        let mut target_config = HashMap::new();
        let mut target_info = HashMap::new();
        if let CompileKind::Target(target) = build_config.requested_kind {
            let tcfg = config.target_cfg_triple(target.short_name())?;
            target_config.insert(target, tcfg);
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
    pub fn linker(&self, kind: CompileKind) -> Option<PathBuf> {
        self.target_config(kind)
            .linker
            .as_ref()
            .map(|l| l.val.clone().resolve_program(self.config))
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
        self.target_config(kind).links_overrides.get(lib_name)
    }
}
