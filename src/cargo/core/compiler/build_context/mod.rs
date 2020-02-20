use crate::core::compiler::unit::UnitInterner;
use crate::core::compiler::{BuildConfig, BuildOutput, CompileKind, Unit};
use crate::core::profiles::Profiles;
use crate::core::{InternedString, Workspace};
use crate::core::{PackageId, PackageSet};
use crate::util::config::Config;
use crate::util::errors::CargoResult;
use crate::util::Rustc;
use std::collections::HashMap;
use std::path::PathBuf;
use std::str;

mod target_info;
pub use self::target_info::{FileFlavor, RustcTargetData, TargetInfo};

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
    /// Package downloader.
    pub packages: &'a PackageSet<'cfg>,

    /// Source of interning new units as they're created.
    pub units: &'a UnitInterner<'a>,

    /// Information about rustc and the target platform.
    pub target_data: RustcTargetData,
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
        target_data: RustcTargetData,
    ) -> CargoResult<BuildContext<'a, 'cfg>> {
        Ok(BuildContext {
            ws,
            packages,
            config,
            build_config,
            profiles,
            extra_compiler_args,
            units,
            target_data,
        })
    }

    pub fn rustc(&self) -> &Rustc {
        &self.target_data.rustc
    }

    /// Gets the user-specified linker for a particular host or target.
    pub fn linker(&self, kind: CompileKind) -> Option<PathBuf> {
        self.target_data
            .target_config(kind)
            .linker
            .as_ref()
            .map(|l| l.val.clone().resolve_program(self.config))
    }

    /// Gets the host architecture triple.
    ///
    /// For example, x86_64-unknown-linux-gnu, would be
    /// - machine: x86_64,
    /// - hardware-platform: unknown,
    /// - operating system: linux-gnu.
    pub fn host_triple(&self) -> InternedString {
        self.target_data.rustc.host
    }

    /// Gets the number of jobs specified for this build.
    pub fn jobs(&self) -> u32 {
        self.build_config.jobs
    }

    pub fn rustflags_args(&self, unit: &Unit<'_>) -> &[String] {
        &self.target_data.info(unit.kind).rustflags
    }

    pub fn rustdocflags_args(&self, unit: &Unit<'_>) -> &[String] {
        &self.target_data.info(unit.kind).rustdocflags
    }

    pub fn show_warnings(&self, pkg: PackageId) -> bool {
        pkg.source_id().is_path() || self.config.extra_verbose()
    }

    pub fn extra_args_for(&self, unit: &Unit<'a>) -> Option<&Vec<String>> {
        self.extra_compiler_args.get(unit)
    }

    /// If a build script is overridden, this returns the `BuildOutput` to use.
    ///
    /// `lib_name` is the `links` library name and `kind` is whether it is for
    /// Host or Target.
    pub fn script_override(&self, lib_name: &str, kind: CompileKind) -> Option<&BuildOutput> {
        self.target_data
            .target_config(kind)
            .links_overrides
            .get(lib_name)
    }
}
