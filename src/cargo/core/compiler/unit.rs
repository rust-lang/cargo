//! Types and impls for [`Unit`].

use crate::core::Package;
use crate::core::compiler::unit_dependencies::IsArtifact;
use crate::core::compiler::{CompileKind, CompileMode, CompileTarget, CrateType};
use crate::core::manifest::{Target, TargetKind};
use crate::core::profiles::Profile;
use crate::util::GlobalContext;
use crate::util::interning::InternedString;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::rc::Rc;

use super::BuildOutput;

/// All information needed to define a unit.
///
/// A unit is an object that has enough information so that cargo knows how to build it.
/// For example, if your package has dependencies, then every dependency will be built as a library
/// unit. If your package is a library, then it will be built as a library unit as well, or if it
/// is a binary with `main.rs`, then a binary will be output. There are also separate unit types
/// for `test`ing and `check`ing, amongst others.
///
/// The unit also holds information about all possible metadata about the package in `pkg`.
///
/// A unit needs to know extra information in addition to the type and root source file. For
/// example, it needs to know the target architecture (OS, chip arch etc.) and it needs to know
/// whether you want a debug or release build. There is enough information in this struct to figure
/// all that out.
#[derive(Clone, PartialOrd, Ord)]
pub struct Unit {
    inner: Rc<UnitInner>,
}

/// Internal fields of `Unit` which `Unit` will dereference to.
#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnitInner {
    /// Information about available targets, which files to include/exclude, etc. Basically stuff in
    /// `Cargo.toml`.
    pub pkg: Package,
    /// Information about the specific target to build, out of the possible targets in `pkg`. Not
    /// to be confused with *target-triple* (or *target architecture* ...), the target arch for a
    /// build.
    pub target: Target,
    /// The profile contains information about *how* the build should be run, including debug
    /// level, etc.
    pub profile: Profile,
    /// Whether this compilation unit is for the host or target architecture.
    ///
    /// For example, when
    /// cross compiling and using a custom build script, the build script needs to be compiled for
    /// the host architecture so the host rustc can use it (when compiling to the target
    /// architecture).
    pub kind: CompileKind,
    /// The "mode" this unit is being compiled for. See [`CompileMode`] for more details.
    pub mode: CompileMode,
    /// The `cfg` features to enable for this unit.
    /// This must be sorted.
    pub features: Vec<InternedString>,
    /// Extra compiler flags to pass to `rustc` for a given unit.
    ///
    /// Although it depends on the caller, in the current Cargo implementation,
    /// these flags take precedence over those from [`BuildContext::extra_args_for`].
    ///
    /// As of now, these flags come from environment variables and configurations.
    /// See [`TargetInfo.rustflags`] for more on how Cargo collects them.
    ///
    /// [`BuildContext::extra_args_for`]: crate::core::compiler::build_context::BuildContext::extra_args_for
    /// [`TargetInfo.rustflags`]: crate::core::compiler::build_context::TargetInfo::rustflags
    pub rustflags: Rc<[String]>,
    /// Extra compiler flags to pass to `rustdoc` for a given unit.
    ///
    /// Although it depends on the caller, in the current Cargo implementation,
    /// these flags take precedence over those from [`BuildContext::extra_args_for`].
    ///
    /// As of now, these flags come from environment variables and configurations.
    /// See [`TargetInfo.rustdocflags`] for more on how Cargo collects them.
    ///
    /// [`BuildContext::extra_args_for`]: crate::core::compiler::build_context::BuildContext::extra_args_for
    /// [`TargetInfo.rustdocflags`]: crate::core::compiler::build_context::TargetInfo::rustdocflags
    pub rustdocflags: Rc<[String]>,
    /// Build script override for the given library name.
    ///
    /// Any package with a `links` value for the given library name will skip
    /// running its build script and instead use the given output from the
    /// config file.
    pub links_overrides: Rc<BTreeMap<String, BuildOutput>>,
    // if `true`, the dependency is an artifact dependency, requiring special handling when
    // calculating output directories, linkage and environment variables provided to builds.
    pub artifact: IsArtifact,
    /// Whether this is a standard library unit.
    pub is_std: bool,
    /// A hash of all dependencies of this unit.
    ///
    /// This is used to keep the `Unit` unique in the situation where two
    /// otherwise identical units need to link to different dependencies. This
    /// can happen, for example, when there are shared dependencies that need
    /// to be built with different features between normal and build
    /// dependencies. See `rebuild_unit_graph_shared` for more on why this is
    /// done.
    ///
    /// This value initially starts as 0, and then is filled in via a
    /// second-pass after all the unit dependencies have been computed.
    pub dep_hash: u64,

    /// This is used for target-dependent feature resolution and is copied from
    /// [`FeaturesFor::ArtifactDep`], if the enum matches the variant.
    ///
    /// [`FeaturesFor::ArtifactDep`]: crate::core::resolver::features::FeaturesFor::ArtifactDep
    pub artifact_target_for_features: Option<CompileTarget>,

    /// Skip compiling this unit because `--compile-time-deps` flag is set and
    /// this is not a compile time dependency.
    ///
    /// Since dependencies of this unit might be compile time dependencies, we
    /// set this field instead of completely dropping out this unit from unit graph.
    pub skip_non_compile_time_dep: bool,
}

impl UnitInner {
    /// Returns whether compilation of this unit requires all upstream artifacts
    /// to be available.
    ///
    /// This effectively means that this unit is a synchronization point (if the
    /// return value is `true`) that all previously pipelined units need to
    /// finish in their entirety before this one is started.
    pub fn requires_upstream_objects(&self) -> bool {
        self.mode.is_any_test() || self.target.kind().requires_upstream_objects()
    }

    /// Returns whether compilation of this unit could benefit from splitting metadata
    /// into a .rmeta file.
    pub fn benefits_from_no_embed_metadata(&self) -> bool {
        matches!(self.mode, CompileMode::Build)
            && self.target.kind().benefits_from_no_embed_metadata()
    }

    /// Returns whether or not this is a "local" package.
    ///
    /// A "local" package is one that the user can likely edit, or otherwise
    /// wants warnings, etc.
    pub fn is_local(&self) -> bool {
        self.pkg.package_id().source_id().is_path() && !self.is_std
    }

    /// Returns whether or not warnings should be displayed for this unit.
    pub fn show_warnings(&self, gctx: &GlobalContext) -> bool {
        self.is_local() || gctx.extra_verbose()
    }
}

// Just hash the pointer for fast hashing
impl Hash for Unit {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        std::ptr::hash(&*self.inner, hasher)
    }
}

// Just equate the pointer since these are interned
impl PartialEq for Unit {
    fn eq(&self, other: &Unit) -> bool {
        std::ptr::eq(&*self.inner, &*other.inner)
    }
}

impl Eq for Unit {}

impl Deref for Unit {
    type Target = UnitInner;

    fn deref(&self) -> &UnitInner {
        &*self.inner
    }
}

impl fmt::Debug for Unit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Unit")
            .field("pkg", &self.pkg)
            .field("target", &self.target)
            .field("profile", &self.profile)
            .field("kind", &self.kind)
            .field("mode", &self.mode)
            .field("features", &self.features)
            .field("rustflags", &self.rustflags)
            .field("rustdocflags", &self.rustdocflags)
            .field("links_overrides", &self.links_overrides)
            .field("artifact", &self.artifact.is_true())
            .field(
                "artifact_target_for_features",
                &self.artifact_target_for_features,
            )
            .field("is_std", &self.is_std)
            .field("dep_hash", &self.dep_hash)
            .finish()
    }
}

/// A small structure used to "intern" `Unit` values.
///
/// A `Unit` is just a thin pointer to an internal `UnitInner`. This is done to
/// ensure that `Unit` itself is quite small as well as enabling a very
/// efficient hash/equality implementation for `Unit`. All units are
/// manufactured through an interner which guarantees that each equivalent value
/// is only produced once.
pub struct UnitInterner {
    state: RefCell<InternerState>,
}

struct InternerState {
    cache: HashSet<Rc<UnitInner>>,
}

impl UnitInterner {
    /// Creates a new blank interner
    pub fn new() -> UnitInterner {
        UnitInterner {
            state: RefCell::new(InternerState {
                cache: HashSet::new(),
            }),
        }
    }

    /// Creates a new `unit` from its components. The returned `Unit`'s fields
    /// will all be equivalent to the provided arguments, although they may not
    /// be the exact same instance.
    pub fn intern(
        &self,
        pkg: &Package,
        target: &Target,
        profile: Profile,
        kind: CompileKind,
        mode: CompileMode,
        features: Vec<InternedString>,
        rustflags: Rc<[String]>,
        rustdocflags: Rc<[String]>,
        links_overrides: Rc<BTreeMap<String, BuildOutput>>,
        is_std: bool,
        dep_hash: u64,
        artifact: IsArtifact,
        artifact_target_for_features: Option<CompileTarget>,
        skip_non_compile_time_dep: bool,
    ) -> Unit {
        let target = match (is_std, target.kind()) {
            // This is a horrible hack to support build-std. `libstd` declares
            // itself with both rlib and dylib. We don't want the dylib for a
            // few reasons:
            //
            // - dylibs don't have a hash in the filename. If you do something
            //   (like switch rustc versions), it will stomp on the dylib
            //   file, invalidating the entire cache (because std is a dep of
            //   everything).
            // - We don't want to publicize the presence of dylib for the
            //   standard library.
            //
            // At some point in the future, it would be nice to have a
            // first-class way of overriding or specifying crate-types.
            (true, TargetKind::Lib(crate_types)) if crate_types.contains(&CrateType::Dylib) => {
                let mut new_target = Target::clone(target);
                new_target.set_kind(TargetKind::Lib(vec![CrateType::Rlib]));
                new_target
            }
            _ => target.clone(),
        };
        let inner = self.intern_inner(&UnitInner {
            pkg: pkg.clone(),
            target,
            profile,
            kind,
            mode,
            features,
            rustflags,
            rustdocflags,
            links_overrides,
            is_std,
            dep_hash,
            artifact,
            artifact_target_for_features,
            skip_non_compile_time_dep,
        });
        Unit { inner }
    }

    fn intern_inner(&self, item: &UnitInner) -> Rc<UnitInner> {
        let mut me = self.state.borrow_mut();
        if let Some(item) = me.cache.get(item) {
            return item.clone();
        }
        let item = Rc::new(item.clone());
        me.cache.insert(item.clone());
        item
    }
}
