//! [`BuildContext`] is a (mostly) static information about a build task.

use crate::core::PackageSet;
use crate::core::Workspace;
use crate::core::compiler::unit_graph::UnitGraph;
use crate::core::compiler::{BuildConfig, CompileKind, Unit};
use crate::core::profiles::Profiles;
use crate::util::Rustc;
use crate::util::context::GlobalContext;
use crate::util::errors::CargoResult;
use crate::util::interning::InternedString;
use crate::util::logger::BuildLogger;
use std::collections::{HashMap, HashSet};

mod target_info;
pub use self::target_info::{
    FileFlavor, FileType, RustDocFingerprint, RustcTargetData, TargetInfo,
};

/// The build context, containing complete information needed for a build task
/// before it gets started.
///
/// It is intended that this is mostly static information. Stuff that mutates
/// during the build can be found in the parent [`BuildRunner`]. (I say mostly,
/// because this has internal caching, but nothing that should be observable
/// or require &mut.)
///
/// As a result, almost every field on `BuildContext` is public, including
///
/// * a resolved [`UnitGraph`] of your dependencies,
/// * a [`Profiles`] containing compiler flags presets,
/// * a [`RustcTargetData`] containing host and target platform information,
/// * and a [`PackageSet`] for further package downloads,
///
/// just to name a few. Learn more on each own documentation.
///
/// # How to use
///
/// To prepare a build task, you may not want to use [`BuildContext::new`] directly,
/// since it is often too lower-level.
/// Instead, [`ops::create_bcx`] is usually what you are looking for.
///
/// After a `BuildContext` is built, the next stage of building is handled in [`BuildRunner`].
///
/// [`BuildRunner`]: crate::core::compiler::BuildRunner
/// [`ops::create_bcx`]: crate::ops::create_bcx
pub struct BuildContext<'a, 'gctx> {
    /// The workspace the build is for.
    pub ws: &'a Workspace<'gctx>,

    /// The cargo context.
    pub gctx: &'gctx GlobalContext,

    /// Build logger for `-Zbuild-analysis`.
    pub logger: Option<&'a BuildLogger>,

    /// This contains a collection of compiler flags presets.
    pub profiles: Profiles,

    /// Configuration information for a rustc build.
    pub build_config: &'a BuildConfig,

    /// Extra compiler args for either `rustc` or `rustdoc`.
    pub extra_compiler_args: HashMap<Unit, Vec<String>>,

    /// Package downloader.
    ///
    /// This holds ownership of the `Package` objects.
    pub packages: PackageSet<'gctx>,

    /// Information about rustc and the target platform.
    pub target_data: RustcTargetData<'gctx>,

    /// The root units of `unit_graph` (units requested on the command-line).
    pub roots: Vec<Unit>,

    /// The dependency graph of units to compile.
    pub unit_graph: UnitGraph,

    /// Reverse-dependencies of documented units, used by the `rustdoc --scrape-examples` flag.
    pub scrape_units: Vec<Unit>,

    /// The list of all kinds that are involved in this build
    pub all_kinds: HashSet<CompileKind>,
}

impl<'a, 'gctx> BuildContext<'a, 'gctx> {
    pub fn new(
        ws: &'a Workspace<'gctx>,
        logger: Option<&'a BuildLogger>,
        packages: PackageSet<'gctx>,
        build_config: &'a BuildConfig,
        profiles: Profiles,
        extra_compiler_args: HashMap<Unit, Vec<String>>,
        target_data: RustcTargetData<'gctx>,
        roots: Vec<Unit>,
        unit_graph: UnitGraph,
        scrape_units: Vec<Unit>,
    ) -> CargoResult<BuildContext<'a, 'gctx>> {
        let all_kinds = unit_graph
            .keys()
            .map(|u| u.kind)
            .chain(build_config.requested_kinds.iter().copied())
            .chain(std::iter::once(CompileKind::Host))
            .collect();

        Ok(BuildContext {
            ws,
            gctx: ws.gctx(),
            logger,
            packages,
            build_config,
            profiles,
            extra_compiler_args,
            target_data,
            roots,
            unit_graph,
            scrape_units,
            all_kinds,
        })
    }

    /// Information of the `rustc` this build task will use.
    pub fn rustc(&self) -> &Rustc {
        &self.target_data.rustc
    }

    /// Gets the host architecture triple.
    ///
    /// For example, `x86_64-unknown-linux-gnu`, would be
    /// - machine: `x86_64`,
    /// - hardware-platform: `unknown`,
    /// - operating system: `linux-gnu`.
    pub fn host_triple(&self) -> InternedString {
        self.target_data.rustc.host
    }

    /// Gets the number of jobs specified for this build.
    pub fn jobs(&self) -> u32 {
        self.build_config.jobs
    }

    /// Extra compiler args for either `rustc` or `rustdoc`.
    ///
    /// As of now, these flags come from the trailing args of either
    /// `cargo rustc` or `cargo rustdoc`.
    pub fn extra_args_for(&self, unit: &Unit) -> Option<&Vec<String>> {
        self.extra_compiler_args.get(unit)
    }
}
