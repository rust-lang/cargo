use crate::core::compiler::unit_graph::UnitGraph;
use crate::core::compiler::{BuildConfig, CompileKind, Unit};
use crate::core::profiles::Profiles;
use crate::core::PackageSet;
use crate::core::Workspace;
use crate::util::config::Config;
use crate::util::errors::CargoResult;
use crate::util::interning::InternedString;
use crate::util::Rustc;
use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::PathBuf;

mod target_info;
pub use self::target_info::{
    FileFlavor, FileType, RustDocFingerprint, RustcTargetData, TargetInfo,
};

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
    pub extra_compiler_args: HashMap<Unit, Vec<OsString>>,

    /// Package downloader.
    ///
    /// This holds ownership of the `Package` objects.
    pub packages: PackageSet<'cfg>,

    /// Information about rustc and the target platform.
    pub target_data: RustcTargetData<'cfg>,

    /// The root units of `unit_graph` (units requested on the command-line).
    pub roots: Vec<Unit>,

    /// The dependency graph of units to compile.
    pub unit_graph: UnitGraph,

    /// The list of all kinds that are involved in this build
    pub all_kinds: HashSet<CompileKind>,
}

impl<'a, 'cfg> BuildContext<'a, 'cfg> {
    pub fn new(
        ws: &'a Workspace<'cfg>,
        packages: PackageSet<'cfg>,
        build_config: &'a BuildConfig,
        profiles: Profiles,
        extra_compiler_args: HashMap<Unit, Vec<OsString>>,
        target_data: RustcTargetData<'cfg>,
        roots: Vec<Unit>,
        unit_graph: UnitGraph,
    ) -> CargoResult<BuildContext<'a, 'cfg>> {
        let all_kinds = unit_graph
            .keys()
            .map(|u| u.kind)
            .chain(build_config.requested_kinds.iter().copied())
            .chain(std::iter::once(CompileKind::Host))
            .collect();

        Ok(BuildContext {
            ws,
            config: ws.config(),
            packages,
            build_config,
            profiles,
            extra_compiler_args,
            target_data,
            roots,
            unit_graph,
            all_kinds,
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

    pub fn rustflags_args(&self, unit: &Unit) -> &[String] {
        &self.target_data.info(unit.kind).rustflags
    }

    pub fn rustdocflags_args(&self, unit: &Unit) -> &[String] {
        &self.target_data.info(unit.kind).rustdocflags
    }

    pub fn extra_args_for(&self, unit: &Unit) -> Option<&Vec<OsString>> {
        self.extra_compiler_args.get(unit)
    }
}
