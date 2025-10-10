use std::cell::RefCell;
use std::collections::hash_map::{Entry, HashMap};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use annotate_snippets::Level;
use anyhow::{Context as _, anyhow, bail};
use glob::glob;
use itertools::Itertools;
use tracing::debug;
use url::Url;

use crate::core::compiler::Unit;
use crate::core::features::Features;
use crate::core::registry::PackageRegistry;
use crate::core::resolver::ResolveBehavior;
use crate::core::resolver::features::CliFeatures;
use crate::core::{
    Dependency, Edition, FeatureValue, PackageId, PackageIdSpec, PackageIdSpecQuery,
};
use crate::core::{EitherManifest, Package, SourceId, VirtualManifest};
use crate::ops;
use crate::sources::{CRATES_IO_INDEX, CRATES_IO_REGISTRY, PathSource, SourceConfigMap};
use crate::util::context::FeatureUnification;
use crate::util::edit_distance;
use crate::util::errors::{CargoResult, ManifestError};
use crate::util::interning::InternedString;
use crate::util::lints::{
    analyze_cargo_lints_table, blanket_hint_mostly_unused, check_im_a_teapot,
};
use crate::util::toml::{InheritableFields, read_manifest};
use crate::util::{
    Filesystem, GlobalContext, IntoUrl, context::CargoResolverConfig, context::ConfigRelativePath,
    context::IncompatibleRustVersions,
};
use cargo_util::paths;
use cargo_util::paths::normalize_path;
use cargo_util_schemas::manifest;
use cargo_util_schemas::manifest::RustVersion;
use cargo_util_schemas::manifest::{TomlDependency, TomlProfiles};
use pathdiff::diff_paths;

/// The core abstraction in Cargo for working with a workspace of crates.
///
/// A workspace is often created very early on and then threaded through all
/// other functions. It's typically through this object that the current
/// package is loaded and/or learned about.
#[derive(Debug)]
pub struct Workspace<'gctx> {
    /// Cargo configuration information. See [`GlobalContext`].
    gctx: &'gctx GlobalContext,

    /// This path is a path to where the current cargo subcommand was invoked
    /// from. That is the `--manifest-path` argument to Cargo, and
    /// points to the "main crate" that we're going to worry about.
    current_manifest: PathBuf,

    /// A list of packages found in this workspace. Always includes at least the
    /// package mentioned by `current_manifest`.
    packages: Packages<'gctx>,

    /// If this workspace includes more than one crate, this points to the root
    /// of the workspace. This is `None` in the case that `[workspace]` is
    /// missing, `package.workspace` is missing, and no `Cargo.toml` above
    /// `current_manifest` was found on the filesystem with `[workspace]`.
    root_manifest: Option<PathBuf>,

    /// Shared target directory for all the packages of this workspace.
    /// `None` if the default path of `root/target` should be used.
    target_dir: Option<Filesystem>,

    /// Shared build directory for intermediate build artifacts.
    /// This directory may be shared between multiple workspaces.
    build_dir: Option<Filesystem>,

    /// List of members in this workspace with a listing of all their manifest
    /// paths. The packages themselves can be looked up through the `packages`
    /// set above.
    members: Vec<PathBuf>,
    /// Set of ids of workspace members
    member_ids: HashSet<PackageId>,

    /// The subset of `members` that are used by the
    /// `build`, `check`, `test`, and `bench` subcommands
    /// when no package is selected with `--package` / `-p` and `--workspace`
    /// is not used.
    ///
    /// This is set by the `default-members` config
    /// in the `[workspace]` section.
    /// When unset, this is the same as `members` for virtual workspaces
    /// (`--workspace` is implied)
    /// or only the root package for non-virtual workspaces.
    default_members: Vec<PathBuf>,

    /// `true` if this is a temporary workspace created for the purposes of the
    /// `cargo install` or `cargo package` commands.
    is_ephemeral: bool,

    /// `true` if this workspace should enforce optional dependencies even when
    /// not needed; false if this workspace should only enforce dependencies
    /// needed by the current configuration (such as in cargo install). In some
    /// cases `false` also results in the non-enforcement of dev-dependencies.
    require_optional_deps: bool,

    /// A cache of loaded packages for particular paths which is disjoint from
    /// `packages` up above, used in the `load` method down below.
    loaded_packages: RefCell<HashMap<PathBuf, Package>>,

    /// If `true`, then the resolver will ignore any existing `Cargo.lock`
    /// file. This is set for `cargo install` without `--locked`.
    ignore_lock: bool,

    /// Requested path of the lockfile (i.e. passed as the cli flag)
    requested_lockfile_path: Option<PathBuf>,

    /// The resolver behavior specified with the `resolver` field.
    resolve_behavior: ResolveBehavior,
    /// If `true`, then workspace `rust_version` would be used in `cargo resolve`
    /// and other places that use rust version.
    /// This is set based on the resolver version, config settings, and CLI flags.
    resolve_honors_rust_version: bool,
    /// The feature unification mode used when building packages.
    resolve_feature_unification: FeatureUnification,
    /// Workspace-level custom metadata
    custom_metadata: Option<toml::Value>,

    /// Local overlay configuration. See [`crate::sources::overlay`].
    local_overlays: HashMap<SourceId, PathBuf>,
}

// Separate structure for tracking loaded packages (to avoid loading anything
// twice), and this is separate to help appease the borrow checker.
#[derive(Debug)]
struct Packages<'gctx> {
    gctx: &'gctx GlobalContext,
    packages: HashMap<PathBuf, MaybePackage>,
}

#[derive(Debug)]
pub enum MaybePackage {
    Package(Package),
    Virtual(VirtualManifest),
}

/// Configuration of a workspace in a manifest.
#[derive(Debug, Clone)]
pub enum WorkspaceConfig {
    /// Indicates that `[workspace]` was present and the members were
    /// optionally specified as well.
    Root(WorkspaceRootConfig),

    /// Indicates that `[workspace]` was present and the `root` field is the
    /// optional value of `package.workspace`, if present.
    Member { root: Option<String> },
}

impl WorkspaceConfig {
    pub fn inheritable(&self) -> Option<&InheritableFields> {
        match self {
            WorkspaceConfig::Root(root) => Some(&root.inheritable_fields),
            WorkspaceConfig::Member { .. } => None,
        }
    }

    /// Returns the path of the workspace root based on this `[workspace]` configuration.
    ///
    /// Returns `None` if the root is not explicitly known.
    ///
    /// * `self_path` is the path of the manifest this `WorkspaceConfig` is located.
    /// * `look_from` is the path where discovery started (usually the current
    ///   working directory), used for `workspace.exclude` checking.
    fn get_ws_root(&self, self_path: &Path, look_from: &Path) -> Option<PathBuf> {
        match self {
            WorkspaceConfig::Root(ances_root_config) => {
                debug!("find_root - found a root checking exclusion");
                if !ances_root_config.is_excluded(look_from) {
                    debug!("find_root - found!");
                    Some(self_path.to_owned())
                } else {
                    None
                }
            }
            WorkspaceConfig::Member {
                root: Some(path_to_root),
            } => {
                debug!("find_root - found pointer");
                Some(read_root_pointer(self_path, path_to_root))
            }
            WorkspaceConfig::Member { .. } => None,
        }
    }
}

/// Intermediate configuration of a workspace root in a manifest.
///
/// Knows the Workspace Root path, as well as `members` and `exclude` lists of path patterns, which
/// together tell if some path is recognized as a member by this root or not.
#[derive(Debug, Clone)]
pub struct WorkspaceRootConfig {
    root_dir: PathBuf,
    members: Option<Vec<String>>,
    default_members: Option<Vec<String>>,
    exclude: Vec<String>,
    inheritable_fields: InheritableFields,
    custom_metadata: Option<toml::Value>,
}

impl<'gctx> Workspace<'gctx> {
    /// Creates a new workspace given the target manifest pointed to by
    /// `manifest_path`.
    ///
    /// This function will construct the entire workspace by determining the
    /// root and all member packages. It will then validate the workspace
    /// before returning it, so `Ok` is only returned for valid workspaces.
    pub fn new(manifest_path: &Path, gctx: &'gctx GlobalContext) -> CargoResult<Workspace<'gctx>> {
        let mut ws = Workspace::new_default(manifest_path.to_path_buf(), gctx);

        if manifest_path.is_relative() {
            bail!(
                "manifest_path:{:?} is not an absolute path. Please provide an absolute path.",
                manifest_path
            )
        } else {
            ws.root_manifest = ws.find_root(manifest_path)?;
        }

        ws.target_dir = gctx.target_dir()?;
        ws.build_dir = gctx.build_dir(ws.root_manifest())?;

        ws.custom_metadata = ws
            .load_workspace_config()?
            .and_then(|cfg| cfg.custom_metadata);
        ws.find_members()?;
        ws.set_resolve_behavior()?;
        ws.validate()?;
        Ok(ws)
    }

    fn new_default(current_manifest: PathBuf, gctx: &'gctx GlobalContext) -> Workspace<'gctx> {
        Workspace {
            gctx,
            current_manifest,
            packages: Packages {
                gctx,
                packages: HashMap::new(),
            },
            root_manifest: None,
            target_dir: None,
            build_dir: None,
            members: Vec::new(),
            member_ids: HashSet::new(),
            default_members: Vec::new(),
            is_ephemeral: false,
            require_optional_deps: true,
            loaded_packages: RefCell::new(HashMap::new()),
            ignore_lock: false,
            requested_lockfile_path: None,
            resolve_behavior: ResolveBehavior::V1,
            resolve_honors_rust_version: false,
            resolve_feature_unification: FeatureUnification::Selected,
            custom_metadata: None,
            local_overlays: HashMap::new(),
        }
    }

    /// Creates a "temporary workspace" from one package which only contains
    /// that package.
    ///
    /// This constructor will not touch the filesystem and only creates an
    /// in-memory workspace. That is, all configuration is ignored, it's just
    /// intended for that one package.
    ///
    /// This is currently only used in niche situations like `cargo install` or
    /// `cargo package`.
    pub fn ephemeral(
        package: Package,
        gctx: &'gctx GlobalContext,
        target_dir: Option<Filesystem>,
        require_optional_deps: bool,
    ) -> CargoResult<Workspace<'gctx>> {
        let mut ws = Workspace::new_default(package.manifest_path().to_path_buf(), gctx);
        ws.is_ephemeral = true;
        ws.require_optional_deps = require_optional_deps;
        let id = package.package_id();
        let package = MaybePackage::Package(package);
        ws.packages
            .packages
            .insert(ws.current_manifest.clone(), package);
        ws.target_dir = if let Some(dir) = target_dir {
            Some(dir)
        } else {
            ws.gctx.target_dir()?
        };
        ws.build_dir = ws.target_dir.clone();
        ws.members.push(ws.current_manifest.clone());
        ws.member_ids.insert(id);
        ws.default_members.push(ws.current_manifest.clone());
        ws.set_resolve_behavior()?;
        Ok(ws)
    }

    /// Reloads the workspace.
    ///
    /// This is useful if the workspace has been updated, such as with `cargo
    /// fix` modifying the `Cargo.toml` file.
    pub fn reload(&self, gctx: &'gctx GlobalContext) -> CargoResult<Workspace<'gctx>> {
        let mut ws = Workspace::new(&self.current_manifest, gctx)?;
        ws.set_resolve_honors_rust_version(Some(self.resolve_honors_rust_version));
        ws.set_resolve_feature_unification(self.resolve_feature_unification);
        ws.set_requested_lockfile_path(self.requested_lockfile_path.clone());
        Ok(ws)
    }

    fn set_resolve_behavior(&mut self) -> CargoResult<()> {
        // - If resolver is specified in the workspace definition, use that.
        // - If the root package specifies the resolver, use that.
        // - If the root package specifies edition 2021, use v2.
        // - Otherwise, use the default v1.
        self.resolve_behavior = match self.root_maybe() {
            MaybePackage::Package(p) => p
                .manifest()
                .resolve_behavior()
                .unwrap_or_else(|| p.manifest().edition().default_resolve_behavior()),
            MaybePackage::Virtual(vm) => vm.resolve_behavior().unwrap_or(ResolveBehavior::V1),
        };

        match self.resolve_behavior() {
            ResolveBehavior::V1 | ResolveBehavior::V2 => {}
            ResolveBehavior::V3 => {
                if self.resolve_behavior == ResolveBehavior::V3 {
                    self.resolve_honors_rust_version = true;
                }
            }
        }
        let config = self.gctx().get::<CargoResolverConfig>("resolver")?;
        if let Some(incompatible_rust_versions) = config.incompatible_rust_versions {
            self.resolve_honors_rust_version =
                incompatible_rust_versions == IncompatibleRustVersions::Fallback;
        }
        if self.gctx().cli_unstable().feature_unification {
            self.resolve_feature_unification = config
                .feature_unification
                .unwrap_or(FeatureUnification::Selected);
        } else if config.feature_unification.is_some() {
            self.gctx()
                .shell()
                .warn("ignoring `resolver.feature-unification` without `-Zfeature-unification`")?;
        };

        Ok(())
    }

    /// Returns the current package of this workspace.
    ///
    /// Note that this can return an error if it the current manifest is
    /// actually a "virtual Cargo.toml", in which case an error is returned
    /// indicating that something else should be passed.
    pub fn current(&self) -> CargoResult<&Package> {
        let pkg = self.current_opt().ok_or_else(|| {
            anyhow::format_err!(
                "manifest path `{}` is a virtual manifest, but this \
                 command requires running against an actual package in \
                 this workspace",
                self.current_manifest.display()
            )
        })?;
        Ok(pkg)
    }

    pub fn current_mut(&mut self) -> CargoResult<&mut Package> {
        let cm = self.current_manifest.clone();
        let pkg = self.current_opt_mut().ok_or_else(|| {
            anyhow::format_err!(
                "manifest path `{}` is a virtual manifest, but this \
                 command requires running against an actual package in \
                 this workspace",
                cm.display()
            )
        })?;
        Ok(pkg)
    }

    pub fn current_opt(&self) -> Option<&Package> {
        match *self.packages.get(&self.current_manifest) {
            MaybePackage::Package(ref p) => Some(p),
            MaybePackage::Virtual(..) => None,
        }
    }

    pub fn current_opt_mut(&mut self) -> Option<&mut Package> {
        match *self.packages.get_mut(&self.current_manifest) {
            MaybePackage::Package(ref mut p) => Some(p),
            MaybePackage::Virtual(..) => None,
        }
    }

    pub fn is_virtual(&self) -> bool {
        match *self.packages.get(&self.current_manifest) {
            MaybePackage::Package(..) => false,
            MaybePackage::Virtual(..) => true,
        }
    }

    /// Returns the `GlobalContext` this workspace is associated with.
    pub fn gctx(&self) -> &'gctx GlobalContext {
        self.gctx
    }

    pub fn profiles(&self) -> Option<&TomlProfiles> {
        self.root_maybe().profiles()
    }

    /// Returns the root path of this workspace.
    ///
    /// That is, this returns the path of the directory containing the
    /// `Cargo.toml` which is the root of this workspace.
    pub fn root(&self) -> &Path {
        self.root_manifest().parent().unwrap()
    }

    /// Returns the path of the `Cargo.toml` which is the root of this
    /// workspace.
    pub fn root_manifest(&self) -> &Path {
        self.root_manifest
            .as_ref()
            .unwrap_or(&self.current_manifest)
    }

    /// Returns the root Package or `VirtualManifest`.
    pub fn root_maybe(&self) -> &MaybePackage {
        self.packages.get(self.root_manifest())
    }

    pub fn target_dir(&self) -> Filesystem {
        self.target_dir
            .clone()
            .unwrap_or_else(|| self.default_target_dir())
    }

    pub fn build_dir(&self) -> Filesystem {
        self.build_dir
            .clone()
            .or_else(|| self.target_dir.clone())
            .unwrap_or_else(|| self.default_build_dir())
    }

    fn default_target_dir(&self) -> Filesystem {
        if self.root_maybe().is_embedded() {
            self.build_dir().join("target")
        } else {
            Filesystem::new(self.root().join("target"))
        }
    }

    fn default_build_dir(&self) -> Filesystem {
        if self.root_maybe().is_embedded() {
            let default = ConfigRelativePath::new(
                "{cargo-cache-home}/build/{workspace-path-hash}"
                    .to_owned()
                    .into(),
            );
            self.gctx()
                .custom_build_dir(&default, self.root_manifest())
                .expect("template is correct")
        } else {
            self.default_target_dir()
        }
    }

    /// Returns the root `[replace]` section of this workspace.
    ///
    /// This may be from a virtual crate or an actual crate.
    pub fn root_replace(&self) -> &[(PackageIdSpec, Dependency)] {
        match self.root_maybe() {
            MaybePackage::Package(p) => p.manifest().replace(),
            MaybePackage::Virtual(vm) => vm.replace(),
        }
    }

    fn config_patch(&self) -> CargoResult<HashMap<Url, Vec<Dependency>>> {
        let config_patch: Option<
            BTreeMap<String, BTreeMap<String, TomlDependency<ConfigRelativePath>>>,
        > = self.gctx.get("patch")?;

        let source = SourceId::for_manifest_path(self.root_manifest())?;

        let mut warnings = Vec::new();

        let mut patch = HashMap::new();
        for (url, deps) in config_patch.into_iter().flatten() {
            let url = match &url[..] {
                CRATES_IO_REGISTRY => CRATES_IO_INDEX.parse().unwrap(),
                url => self
                    .gctx
                    .get_registry_index(url)
                    .or_else(|_| url.into_url())
                    .with_context(|| {
                        format!("[patch] entry `{}` should be a URL or registry name", url)
                    })?,
            };
            patch.insert(
                url,
                deps.iter()
                    .map(|(name, dep)| {
                        crate::util::toml::to_dependency(
                            dep,
                            name,
                            source,
                            self.gctx,
                            &mut warnings,
                            /* platform */ None,
                            // NOTE: Since we use ConfigRelativePath, this root isn't used as
                            // any relative paths are resolved before they'd be joined with root.
                            Path::new("unused-relative-path"),
                            /* kind */ None,
                        )
                    })
                    .collect::<CargoResult<Vec<_>>>()?,
            );
        }

        for message in warnings {
            self.gctx
                .shell()
                .warn(format!("[patch] in cargo config: {}", message))?
        }

        Ok(patch)
    }

    /// Returns the root `[patch]` section of this workspace.
    ///
    /// This may be from a virtual crate or an actual crate.
    pub fn root_patch(&self) -> CargoResult<HashMap<Url, Vec<Dependency>>> {
        let from_manifest = match self.root_maybe() {
            MaybePackage::Package(p) => p.manifest().patch(),
            MaybePackage::Virtual(vm) => vm.patch(),
        };

        let from_config = self.config_patch()?;
        if from_config.is_empty() {
            return Ok(from_manifest.clone());
        }
        if from_manifest.is_empty() {
            return Ok(from_config);
        }

        // We could just chain from_manifest and from_config,
        // but that's not quite right as it won't deal with overlaps.
        let mut combined = from_config;
        for (url, deps_from_manifest) in from_manifest {
            if let Some(deps_from_config) = combined.get_mut(url) {
                // We want from_config to take precedence for each patched name.
                // NOTE: This is inefficient if the number of patches is large!
                let mut from_manifest_pruned = deps_from_manifest.clone();
                for dep_from_config in &mut *deps_from_config {
                    if let Some(i) = from_manifest_pruned.iter().position(|dep_from_manifest| {
                        // XXX: should this also take into account version numbers?
                        dep_from_config.name_in_toml() == dep_from_manifest.name_in_toml()
                    }) {
                        from_manifest_pruned.swap_remove(i);
                    }
                }
                // Whatever is left does not exist in manifest dependencies.
                deps_from_config.extend(from_manifest_pruned);
            } else {
                combined.insert(url.clone(), deps_from_manifest.clone());
            }
        }
        Ok(combined)
    }

    /// Returns an iterator over all packages in this workspace
    pub fn members(&self) -> impl Iterator<Item = &Package> {
        let packages = &self.packages;
        self.members
            .iter()
            .filter_map(move |path| match packages.get(path) {
                MaybePackage::Package(p) => Some(p),
                _ => None,
            })
    }

    /// Returns a mutable iterator over all packages in this workspace
    pub fn members_mut(&mut self) -> impl Iterator<Item = &mut Package> {
        let packages = &mut self.packages.packages;
        let members: HashSet<_> = self.members.iter().map(|path| path).collect();

        packages.iter_mut().filter_map(move |(path, package)| {
            if members.contains(path) {
                if let MaybePackage::Package(p) = package {
                    return Some(p);
                }
            }

            None
        })
    }

    /// Returns an iterator over default packages in this workspace
    pub fn default_members<'a>(&'a self) -> impl Iterator<Item = &'a Package> {
        let packages = &self.packages;
        self.default_members
            .iter()
            .filter_map(move |path| match packages.get(path) {
                MaybePackage::Package(p) => Some(p),
                _ => None,
            })
    }

    /// Returns an iterator over default packages in this workspace
    pub fn default_members_mut(&mut self) -> impl Iterator<Item = &mut Package> {
        let packages = &mut self.packages.packages;
        let members: HashSet<_> = self
            .default_members
            .iter()
            .map(|path| path.parent().unwrap().to_owned())
            .collect();

        packages.iter_mut().filter_map(move |(path, package)| {
            if members.contains(path) {
                if let MaybePackage::Package(p) = package {
                    return Some(p);
                }
            }

            None
        })
    }

    /// Returns true if the package is a member of the workspace.
    pub fn is_member(&self, pkg: &Package) -> bool {
        self.member_ids.contains(&pkg.package_id())
    }

    /// Returns true if the given package_id is a member of the workspace.
    pub fn is_member_id(&self, package_id: PackageId) -> bool {
        self.member_ids.contains(&package_id)
    }

    pub fn is_ephemeral(&self) -> bool {
        self.is_ephemeral
    }

    pub fn require_optional_deps(&self) -> bool {
        self.require_optional_deps
    }

    pub fn set_require_optional_deps(
        &mut self,
        require_optional_deps: bool,
    ) -> &mut Workspace<'gctx> {
        self.require_optional_deps = require_optional_deps;
        self
    }

    pub fn ignore_lock(&self) -> bool {
        self.ignore_lock
    }

    pub fn set_ignore_lock(&mut self, ignore_lock: bool) -> &mut Workspace<'gctx> {
        self.ignore_lock = ignore_lock;
        self
    }

    /// Returns the directory where the lockfile is in.
    pub fn lock_root(&self) -> Filesystem {
        if let Some(requested) = self.requested_lockfile_path.as_ref() {
            return Filesystem::new(
                requested
                    .parent()
                    .expect("Lockfile path can't be root")
                    .to_owned(),
            );
        }
        self.default_lock_root()
    }

    fn default_lock_root(&self) -> Filesystem {
        if self.root_maybe().is_embedded() {
            self.build_dir()
        } else {
            Filesystem::new(self.root().to_owned())
        }
    }

    pub fn set_requested_lockfile_path(&mut self, path: Option<PathBuf>) {
        self.requested_lockfile_path = path;
    }

    pub fn requested_lockfile_path(&self) -> Option<&Path> {
        self.requested_lockfile_path.as_deref()
    }

    /// Get the lowest-common denominator `package.rust-version` within the workspace, if specified
    /// anywhere
    pub fn lowest_rust_version(&self) -> Option<&RustVersion> {
        self.members().filter_map(|pkg| pkg.rust_version()).min()
    }

    pub fn set_resolve_honors_rust_version(&mut self, honor_rust_version: Option<bool>) {
        if let Some(honor_rust_version) = honor_rust_version {
            self.resolve_honors_rust_version = honor_rust_version;
        }
    }

    pub fn resolve_honors_rust_version(&self) -> bool {
        self.resolve_honors_rust_version
    }

    pub fn set_resolve_feature_unification(&mut self, feature_unification: FeatureUnification) {
        self.resolve_feature_unification = feature_unification;
    }

    pub fn resolve_feature_unification(&self) -> FeatureUnification {
        self.resolve_feature_unification
    }

    pub fn custom_metadata(&self) -> Option<&toml::Value> {
        self.custom_metadata.as_ref()
    }

    pub fn load_workspace_config(&mut self) -> CargoResult<Option<WorkspaceRootConfig>> {
        // If we didn't find a root, it must mean there is no [workspace] section, and thus no
        // metadata.
        if let Some(root_path) = &self.root_manifest {
            let root_package = self.packages.load(root_path)?;
            match root_package.workspace_config() {
                WorkspaceConfig::Root(root_config) => {
                    return Ok(Some(root_config.clone()));
                }

                _ => bail!(
                    "root of a workspace inferred but wasn't a root: {}",
                    root_path.display()
                ),
            }
        }

        Ok(None)
    }

    /// Finds the root of a workspace for the crate whose manifest is located
    /// at `manifest_path`.
    ///
    /// This will parse the `Cargo.toml` at `manifest_path` and then interpret
    /// the workspace configuration, optionally walking up the filesystem
    /// looking for other workspace roots.
    ///
    /// Returns an error if `manifest_path` isn't actually a valid manifest or
    /// if some other transient error happens.
    fn find_root(&mut self, manifest_path: &Path) -> CargoResult<Option<PathBuf>> {
        let current = self.packages.load(manifest_path)?;
        match current
            .workspace_config()
            .get_ws_root(manifest_path, manifest_path)
        {
            Some(root_path) => {
                debug!("find_root - is root {}", manifest_path.display());
                Ok(Some(root_path))
            }
            None => find_workspace_root_with_loader(manifest_path, self.gctx, |self_path| {
                Ok(self
                    .packages
                    .load(self_path)?
                    .workspace_config()
                    .get_ws_root(self_path, manifest_path))
            }),
        }
    }

    /// After the root of a workspace has been located, probes for all members
    /// of a workspace.
    ///
    /// If the `workspace.members` configuration is present, then this just
    /// verifies that those are all valid packages to point to. Otherwise, this
    /// will transitively follow all `path` dependencies looking for members of
    /// the workspace.
    #[tracing::instrument(skip_all)]
    fn find_members(&mut self) -> CargoResult<()> {
        let Some(workspace_config) = self.load_workspace_config()? else {
            debug!("find_members - only me as a member");
            self.members.push(self.current_manifest.clone());
            self.default_members.push(self.current_manifest.clone());
            if let Ok(pkg) = self.current() {
                let id = pkg.package_id();
                self.member_ids.insert(id);
            }
            return Ok(());
        };

        // self.root_manifest must be Some to have retrieved workspace_config
        let root_manifest_path = self.root_manifest.clone().unwrap();

        let members_paths = workspace_config
            .members_paths(workspace_config.members.as_deref().unwrap_or_default())?;
        let default_members_paths = if root_manifest_path == self.current_manifest {
            if let Some(ref default) = workspace_config.default_members {
                Some(workspace_config.members_paths(default)?)
            } else {
                None
            }
        } else {
            None
        };

        for (path, glob) in &members_paths {
            self.find_path_deps(&path.join("Cargo.toml"), &root_manifest_path, false)
                .with_context(|| {
                    format!(
                        "failed to load manifest for workspace member `{}`\n\
                        referenced{} by workspace at `{}`",
                        path.display(),
                        glob.map(|g| format!(" via `{g}`")).unwrap_or_default(),
                        root_manifest_path.display(),
                    )
                })?;
        }

        self.find_path_deps(&root_manifest_path, &root_manifest_path, false)?;

        if let Some(default) = default_members_paths {
            for (path, default_member_glob) in default {
                let normalized_path = paths::normalize_path(&path);
                let manifest_path = normalized_path.join("Cargo.toml");
                if !self.members.contains(&manifest_path) {
                    // default-members are allowed to be excluded, but they
                    // still must be referred to by the original (unfiltered)
                    // members list. Note that we aren't testing against the
                    // manifest path, both because `members_paths` doesn't
                    // include `/Cargo.toml`, and because excluded paths may not
                    // be crates.
                    let exclude = members_paths.iter().any(|(m, _)| *m == normalized_path)
                        && workspace_config.is_excluded(&normalized_path);
                    if exclude {
                        continue;
                    }
                    bail!(
                        "package `{}` is listed in default-members{} but is not a member\n\
                        for workspace at `{}`.",
                        path.display(),
                        default_member_glob
                            .map(|g| format!(" via `{g}`"))
                            .unwrap_or_default(),
                        root_manifest_path.display(),
                    )
                }
                self.default_members.push(manifest_path)
            }
        } else if self.is_virtual() {
            self.default_members = self.members.clone()
        } else {
            self.default_members.push(self.current_manifest.clone())
        }

        Ok(())
    }

    fn find_path_deps(
        &mut self,
        manifest_path: &Path,
        root_manifest: &Path,
        is_path_dep: bool,
    ) -> CargoResult<()> {
        let manifest_path = paths::normalize_path(manifest_path);
        if self.members.contains(&manifest_path) {
            return Ok(());
        }
        if is_path_dep && self.root_maybe().is_embedded() {
            // Embedded manifests cannot have workspace members
            return Ok(());
        }
        if is_path_dep
            && !manifest_path.parent().unwrap().starts_with(self.root())
            && self.find_root(&manifest_path)? != self.root_manifest
        {
            // If `manifest_path` is a path dependency outside of the workspace,
            // don't add it, or any of its dependencies, as a members.
            return Ok(());
        }

        if let WorkspaceConfig::Root(ref root_config) =
            *self.packages.load(root_manifest)?.workspace_config()
        {
            if root_config.is_excluded(&manifest_path) {
                return Ok(());
            }
        }

        debug!("find_path_deps - {}", manifest_path.display());
        self.members.push(manifest_path.clone());

        let candidates = {
            let pkg = match *self.packages.load(&manifest_path)? {
                MaybePackage::Package(ref p) => p,
                MaybePackage::Virtual(_) => return Ok(()),
            };
            self.member_ids.insert(pkg.package_id());
            pkg.dependencies()
                .iter()
                .map(|d| (d.source_id(), d.package_name()))
                .filter(|(s, _)| s.is_path())
                .filter_map(|(s, n)| s.url().to_file_path().ok().map(|p| (p, n)))
                .map(|(p, n)| (p.join("Cargo.toml"), n))
                .collect::<Vec<_>>()
        };
        for (path, name) in candidates {
            self.find_path_deps(&path, root_manifest, true)
                .with_context(|| format!("failed to load manifest for dependency `{}`", name))
                .map_err(|err| ManifestError::new(err, manifest_path.clone()))?;
        }
        Ok(())
    }

    /// Returns the unstable nightly-only features enabled via `cargo-features` in the manifest.
    pub fn unstable_features(&self) -> &Features {
        self.root_maybe().unstable_features()
    }

    pub fn resolve_behavior(&self) -> ResolveBehavior {
        self.resolve_behavior
    }

    /// Returns `true` if this workspace uses the new CLI features behavior.
    ///
    /// The old behavior only allowed choosing the features from the package
    /// in the current directory, regardless of which packages were chosen
    /// with the -p flags. The new behavior allows selecting features from the
    /// packages chosen on the command line (with -p or --workspace flags),
    /// ignoring whatever is in the current directory.
    pub fn allows_new_cli_feature_behavior(&self) -> bool {
        self.is_virtual()
            || match self.resolve_behavior() {
                ResolveBehavior::V1 => false,
                ResolveBehavior::V2 | ResolveBehavior::V3 => true,
            }
    }

    /// Validates a workspace, ensuring that a number of invariants are upheld:
    ///
    /// 1. A workspace only has one root.
    /// 2. All workspace members agree on this one root as the root.
    /// 3. The current crate is a member of this workspace.
    #[tracing::instrument(skip_all)]
    fn validate(&mut self) -> CargoResult<()> {
        // The rest of the checks require a VirtualManifest or multiple members.
        if self.root_manifest.is_none() {
            return Ok(());
        }

        self.validate_unique_names()?;
        self.validate_workspace_roots()?;
        self.validate_members()?;
        self.error_if_manifest_not_in_members()?;
        self.validate_manifest()
    }

    fn validate_unique_names(&self) -> CargoResult<()> {
        let mut names = BTreeMap::new();
        for member in self.members.iter() {
            let package = self.packages.get(member);
            let name = match *package {
                MaybePackage::Package(ref p) => p.name(),
                MaybePackage::Virtual(_) => continue,
            };
            if let Some(prev) = names.insert(name, member) {
                bail!(
                    "two packages named `{}` in this workspace:\n\
                         - {}\n\
                         - {}",
                    name,
                    prev.display(),
                    member.display()
                );
            }
        }
        Ok(())
    }

    fn validate_workspace_roots(&self) -> CargoResult<()> {
        let roots: Vec<PathBuf> = self
            .members
            .iter()
            .filter(|&member| {
                let config = self.packages.get(member).workspace_config();
                matches!(config, WorkspaceConfig::Root(_))
            })
            .map(|member| member.parent().unwrap().to_path_buf())
            .collect();
        match roots.len() {
            1 => Ok(()),
            0 => bail!(
                "`package.workspace` configuration points to a crate \
                 which is not configured with [workspace]: \n\
                 configuration at: {}\n\
                 points to: {}",
                self.current_manifest.display(),
                self.root_manifest.as_ref().unwrap().display()
            ),
            _ => {
                bail!(
                    "multiple workspace roots found in the same workspace:\n{}",
                    roots
                        .iter()
                        .map(|r| format!("  {}", r.display()))
                        .collect::<Vec<_>>()
                        .join("\n")
                );
            }
        }
    }

    #[tracing::instrument(skip_all)]
    fn validate_members(&mut self) -> CargoResult<()> {
        for member in self.members.clone() {
            let root = self.find_root(&member)?;
            if root == self.root_manifest {
                continue;
            }

            match root {
                Some(root) => {
                    bail!(
                        "package `{}` is a member of the wrong workspace\n\
                         expected: {}\n\
                         actual:   {}",
                        member.display(),
                        self.root_manifest.as_ref().unwrap().display(),
                        root.display()
                    );
                }
                None => {
                    bail!(
                        "workspace member `{}` is not hierarchically below \
                         the workspace root `{}`",
                        member.display(),
                        self.root_manifest.as_ref().unwrap().display()
                    );
                }
            }
        }
        Ok(())
    }

    fn error_if_manifest_not_in_members(&mut self) -> CargoResult<()> {
        if self.members.contains(&self.current_manifest) {
            return Ok(());
        }

        let root = self.root_manifest.as_ref().unwrap();
        let root_dir = root.parent().unwrap();
        let current_dir = self.current_manifest.parent().unwrap();
        let root_pkg = self.packages.get(root);

        // FIXME: Make this more generic by using a relative path resolver between member and root.
        let members_msg = match current_dir.strip_prefix(root_dir) {
            Ok(rel) => format!(
                "this may be fixable by adding `{}` to the \
                     `workspace.members` array of the manifest \
                     located at: {}",
                rel.display(),
                root.display()
            ),
            Err(_) => format!(
                "this may be fixable by adding a member to \
                     the `workspace.members` array of the \
                     manifest located at: {}",
                root.display()
            ),
        };
        let extra = match *root_pkg {
            MaybePackage::Virtual(_) => members_msg,
            MaybePackage::Package(ref p) => {
                let has_members_list = match *p.manifest().workspace_config() {
                    WorkspaceConfig::Root(ref root_config) => root_config.has_members_list(),
                    WorkspaceConfig::Member { .. } => unreachable!(),
                };
                if !has_members_list {
                    format!(
                        "this may be fixable by ensuring that this \
                             crate is depended on by the workspace \
                             root: {}",
                        root.display()
                    )
                } else {
                    members_msg
                }
            }
        };
        bail!(
            "current package believes it's in a workspace when it's not:\n\
                 current:   {}\n\
                 workspace: {}\n\n{}\n\
                 Alternatively, to keep it out of the workspace, add the package \
                 to the `workspace.exclude` array, or add an empty `[workspace]` \
                 table to the package's manifest.",
            self.current_manifest.display(),
            root.display(),
            extra
        );
    }

    fn validate_manifest(&mut self) -> CargoResult<()> {
        if let Some(ref root_manifest) = self.root_manifest {
            for pkg in self
                .members()
                .filter(|p| p.manifest_path() != root_manifest)
            {
                let manifest = pkg.manifest();
                let emit_warning = |what| -> CargoResult<()> {
                    let msg = format!(
                        "{} for the non root package will be ignored, \
                         specify {} at the workspace root:\n\
                         package:   {}\n\
                         workspace: {}",
                        what,
                        what,
                        pkg.manifest_path().display(),
                        root_manifest.display(),
                    );
                    self.gctx.shell().warn(&msg)
                };
                if manifest.normalized_toml().has_profiles() {
                    emit_warning("profiles")?;
                }
                if !manifest.replace().is_empty() {
                    emit_warning("replace")?;
                }
                if !manifest.patch().is_empty() {
                    emit_warning("patch")?;
                }
                if let Some(behavior) = manifest.resolve_behavior() {
                    if behavior != self.resolve_behavior {
                        // Only warn if they don't match.
                        emit_warning("resolver")?;
                    }
                }
            }
            if let MaybePackage::Virtual(vm) = self.root_maybe() {
                if vm.resolve_behavior().is_none() {
                    if let Some(edition) = self
                        .members()
                        .filter(|p| p.manifest_path() != root_manifest)
                        .map(|p| p.manifest().edition())
                        .filter(|&e| e >= Edition::Edition2021)
                        .max()
                    {
                        let resolver = edition.default_resolve_behavior().to_manifest();
                        let report = &[Level::WARNING
                            .primary_title(format!(
                                "virtual workspace defaulting to `resolver = \"1\"` despite one or more workspace members being on edition {edition} which implies `resolver = \"{resolver}\"`"
                            ))
                            .elements([
                                Level::NOTE.message("to keep the current resolver, specify `workspace.resolver = \"1\"` in the workspace root's manifest"),
                                Level::NOTE.message(
                                    format!("to use the edition {edition} resolver, specify `workspace.resolver = \"{resolver}\"` in the workspace root's manifest"),
                                ),
                                Level::NOTE.message("for more details see https://doc.rust-lang.org/cargo/reference/resolver.html#resolver-versions"),
                            ])];
                        self.gctx.shell().print_report(report, false)?;
                    }
                }
            }
        }
        Ok(())
    }

    pub fn load(&self, manifest_path: &Path) -> CargoResult<Package> {
        match self.packages.maybe_get(manifest_path) {
            Some(MaybePackage::Package(p)) => return Ok(p.clone()),
            Some(&MaybePackage::Virtual(_)) => bail!("cannot load workspace root"),
            None => {}
        }

        let mut loaded = self.loaded_packages.borrow_mut();
        if let Some(p) = loaded.get(manifest_path).cloned() {
            return Ok(p);
        }
        let source_id = SourceId::for_manifest_path(manifest_path)?;
        let package = ops::read_package(manifest_path, source_id, self.gctx)?;
        loaded.insert(manifest_path.to_path_buf(), package.clone());
        Ok(package)
    }

    /// Preload the provided registry with already loaded packages.
    ///
    /// A workspace may load packages during construction/parsing/early phases
    /// for various operations, and this preload step avoids doubly-loading and
    /// parsing crates on the filesystem by inserting them all into the registry
    /// with their in-memory formats.
    pub fn preload(&self, registry: &mut PackageRegistry<'gctx>) {
        // These can get weird as this generally represents a workspace during
        // `cargo install`. Things like git repositories will actually have a
        // `PathSource` with multiple entries in it, so the logic below is
        // mostly just an optimization for normal `cargo build` in workspaces
        // during development.
        if self.is_ephemeral {
            return;
        }

        for pkg in self.packages.packages.values() {
            let pkg = match *pkg {
                MaybePackage::Package(ref p) => p.clone(),
                MaybePackage::Virtual(_) => continue,
            };
            let src = PathSource::preload_with(pkg, self.gctx);
            registry.add_preloaded(Box::new(src));
        }
    }

    pub fn emit_warnings(&self) -> CargoResult<()> {
        let mut first_emitted_error = None;

        if let Err(e) = self.emit_ws_lints() {
            first_emitted_error = Some(e);
        }

        for (path, maybe_pkg) in &self.packages.packages {
            if let MaybePackage::Package(pkg) = maybe_pkg {
                if let Err(e) = self.emit_pkg_lints(pkg, &path)
                    && first_emitted_error.is_none()
                {
                    first_emitted_error = Some(e);
                }
            }
            let warnings = match maybe_pkg {
                MaybePackage::Package(pkg) => pkg.manifest().warnings().warnings(),
                MaybePackage::Virtual(vm) => vm.warnings().warnings(),
            };
            for warning in warnings {
                if warning.is_critical {
                    let err = anyhow::format_err!("{}", warning.message);
                    let cx =
                        anyhow::format_err!("failed to parse manifest at `{}`", path.display());
                    if first_emitted_error.is_none() {
                        first_emitted_error = Some(err.context(cx));
                    }
                } else {
                    let msg = if self.root_manifest.is_none() {
                        warning.message.to_string()
                    } else {
                        // In a workspace, it can be confusing where a warning
                        // originated, so include the path.
                        format!("{}: {}", path.display(), warning.message)
                    };
                    self.gctx.shell().warn(msg)?
                }
            }
        }

        if let Some(error) = first_emitted_error {
            Err(error)
        } else {
            Ok(())
        }
    }

    pub fn emit_pkg_lints(&self, pkg: &Package, path: &Path) -> CargoResult<()> {
        let mut error_count = 0;
        let toml_lints = pkg
            .manifest()
            .normalized_toml()
            .lints
            .clone()
            .map(|lints| lints.lints)
            .unwrap_or(manifest::TomlLints::default());
        let cargo_lints = toml_lints
            .get("cargo")
            .cloned()
            .unwrap_or(manifest::TomlToolLints::default());

        let ws_contents = self.root_maybe().contents();

        let ws_document = self.root_maybe().document();

        if self.gctx.cli_unstable().cargo_lints {
            analyze_cargo_lints_table(
                pkg,
                &path,
                &cargo_lints,
                ws_contents,
                ws_document,
                self.root_manifest(),
                self.gctx,
            )?;
            check_im_a_teapot(pkg, &path, &cargo_lints, &mut error_count, self.gctx)?;
        }

        if error_count > 0 {
            Err(crate::util::errors::AlreadyPrintedError::new(anyhow!(
                "encountered {error_count} errors(s) while running lints"
            ))
            .into())
        } else {
            Ok(())
        }
    }

    pub fn emit_ws_lints(&self) -> CargoResult<()> {
        let mut error_count = 0;

        let cargo_lints = match self.root_maybe() {
            MaybePackage::Package(pkg) => {
                let toml = pkg.manifest().normalized_toml();
                if let Some(ws) = &toml.workspace {
                    ws.lints.as_ref()
                } else {
                    toml.lints.as_ref().map(|l| &l.lints)
                }
            }
            MaybePackage::Virtual(vm) => vm
                .normalized_toml()
                .workspace
                .as_ref()
                .unwrap()
                .lints
                .as_ref(),
        }
        .and_then(|t| t.get("cargo"))
        .cloned()
        .unwrap_or(manifest::TomlToolLints::default());

        if self.gctx.cli_unstable().cargo_lints {
            // Calls to lint functions go in here
        }

        // This is a short term hack to allow `blanket_hint_mostly_unused`
        // to run without requiring `-Zcargo-lints`, which should hopefully
        // improve the testing expierience while we are collecting feedback
        if self.gctx.cli_unstable().profile_hint_mostly_unused {
            blanket_hint_mostly_unused(
                self.root_maybe(),
                self.root_manifest(),
                &cargo_lints,
                &mut error_count,
                self.gctx,
            )?;
        }

        if error_count > 0 {
            Err(crate::util::errors::AlreadyPrintedError::new(anyhow!(
                "encountered {error_count} errors(s) while running lints"
            ))
            .into())
        } else {
            Ok(())
        }
    }

    pub fn set_target_dir(&mut self, target_dir: Filesystem) {
        self.target_dir = Some(target_dir);
    }

    /// Returns a Vec of `(&Package, CliFeatures)` tuples that
    /// represent the workspace members that were requested on the command-line.
    ///
    /// `specs` may be empty, which indicates it should return all workspace
    /// members. In this case, `requested_features.all_features` must be
    /// `true`. This is used for generating `Cargo.lock`, which must include
    /// all members with all features enabled.
    pub fn members_with_features(
        &self,
        specs: &[PackageIdSpec],
        cli_features: &CliFeatures,
    ) -> CargoResult<Vec<(&Package, CliFeatures)>> {
        assert!(
            !specs.is_empty() || cli_features.all_features,
            "no specs requires all_features"
        );
        if specs.is_empty() {
            // When resolving the entire workspace, resolve each member with
            // all features enabled.
            return Ok(self
                .members()
                .map(|m| (m, CliFeatures::new_all(true)))
                .collect());
        }
        if self.allows_new_cli_feature_behavior() {
            self.members_with_features_new(specs, cli_features)
        } else {
            Ok(self.members_with_features_old(specs, cli_features))
        }
    }

    /// Returns the requested features for the given member.
    /// This filters out any named features that the member does not have.
    fn collect_matching_features(
        member: &Package,
        cli_features: &CliFeatures,
        found_features: &mut BTreeSet<FeatureValue>,
    ) -> CliFeatures {
        if cli_features.features.is_empty() {
            return cli_features.clone();
        }

        // Only include features this member defines.
        let summary = member.summary();

        // Features defined in the manifest
        let summary_features = summary.features();

        // Dependency name -> dependency
        let dependencies: BTreeMap<InternedString, &Dependency> = summary
            .dependencies()
            .iter()
            .map(|dep| (dep.name_in_toml(), dep))
            .collect();

        // Features that enable optional dependencies
        let optional_dependency_names: BTreeSet<_> = dependencies
            .iter()
            .filter(|(_, dep)| dep.is_optional())
            .map(|(name, _)| name)
            .copied()
            .collect();

        let mut features = BTreeSet::new();

        // Checks if a member contains the given feature.
        let summary_or_opt_dependency_feature = |feature: &InternedString| -> bool {
            summary_features.contains_key(feature) || optional_dependency_names.contains(feature)
        };

        for feature in cli_features.features.iter() {
            match feature {
                FeatureValue::Feature(f) => {
                    if summary_or_opt_dependency_feature(f) {
                        // feature exists in this member.
                        features.insert(feature.clone());
                        found_features.insert(feature.clone());
                    }
                }
                // This should be enforced by CliFeatures.
                FeatureValue::Dep { .. } => panic!("unexpected dep: syntax {}", feature),
                FeatureValue::DepFeature {
                    dep_name,
                    dep_feature,
                    weak: _,
                } => {
                    if dependencies.contains_key(dep_name) {
                        // pkg/feat for a dependency.
                        // Will rely on the dependency resolver to validate `dep_feature`.
                        features.insert(feature.clone());
                        found_features.insert(feature.clone());
                    } else if *dep_name == member.name()
                        && summary_or_opt_dependency_feature(dep_feature)
                    {
                        // member/feat where "feat" is a feature in member.
                        //
                        // `weak` can be ignored here, because the member
                        // either is or isn't being built.
                        features.insert(FeatureValue::Feature(*dep_feature));
                        found_features.insert(feature.clone());
                    }
                }
            }
        }
        CliFeatures {
            features: Rc::new(features),
            all_features: cli_features.all_features,
            uses_default_features: cli_features.uses_default_features,
        }
    }

    fn missing_feature_spelling_suggestions(
        &self,
        selected_members: &[&Package],
        cli_features: &CliFeatures,
        found_features: &BTreeSet<FeatureValue>,
    ) -> Vec<String> {
        // Keeps track of which features were contained in summary of `member` to suggest similar features in errors
        let mut summary_features: Vec<InternedString> = Default::default();

        // Keeps track of `member` dependencies (`dep/feature`) and their features names to suggest similar features in error
        let mut dependencies_features: BTreeMap<InternedString, &[InternedString]> =
            Default::default();

        // Keeps track of `member` optional dependencies names (which can be enabled with feature) to suggest similar features in error
        let mut optional_dependency_names: Vec<InternedString> = Default::default();

        // Keeps track of which features were contained in summary of `member` to suggest similar features in errors
        let mut summary_features_per_member: BTreeMap<&Package, BTreeSet<InternedString>> =
            Default::default();

        // Keeps track of `member` optional dependencies (which can be enabled with feature) to suggest similar features in error
        let mut optional_dependency_names_per_member: BTreeMap<&Package, BTreeSet<InternedString>> =
            Default::default();

        for &member in selected_members {
            // Only include features this member defines.
            let summary = member.summary();

            // Features defined in the manifest
            summary_features.extend(summary.features().keys());
            summary_features_per_member
                .insert(member, summary.features().keys().copied().collect());

            // Dependency name -> dependency
            let dependencies: BTreeMap<InternedString, &Dependency> = summary
                .dependencies()
                .iter()
                .map(|dep| (dep.name_in_toml(), dep))
                .collect();

            dependencies_features.extend(
                dependencies
                    .iter()
                    .map(|(name, dep)| (*name, dep.features())),
            );

            // Features that enable optional dependencies
            let optional_dependency_names_raw: BTreeSet<_> = dependencies
                .iter()
                .filter(|(_, dep)| dep.is_optional())
                .map(|(name, _)| name)
                .copied()
                .collect();

            optional_dependency_names.extend(optional_dependency_names_raw.iter());
            optional_dependency_names_per_member.insert(member, optional_dependency_names_raw);
        }

        let edit_distance_test = |a: InternedString, b: InternedString| {
            edit_distance(a.as_str(), b.as_str(), 3).is_some()
        };

        cli_features
            .features
            .difference(found_features)
            .map(|feature| match feature {
                // Simple feature, check if any of the optional dependency features or member features are close enough
                FeatureValue::Feature(typo) => {
                    // Finds member features which are similar to the requested feature.
                    let summary_features = summary_features
                        .iter()
                        .filter(move |feature| edit_distance_test(**feature, *typo));

                    // Finds optional dependencies which name is similar to the feature
                    let optional_dependency_features = optional_dependency_names
                        .iter()
                        .filter(move |feature| edit_distance_test(**feature, *typo));

                    summary_features
                        .chain(optional_dependency_features)
                        .map(|s| s.to_string())
                        .collect::<Vec<_>>()
                }
                FeatureValue::Dep { .. } => panic!("unexpected dep: syntax {}", feature),
                FeatureValue::DepFeature {
                    dep_name,
                    dep_feature,
                    weak: _,
                } => {
                    // Finds set of `pkg/feat` that are very similar to current `pkg/feat`.
                    let pkg_feat_similar = dependencies_features
                        .iter()
                        .filter(|(name, _)| edit_distance_test(**name, *dep_name))
                        .map(|(name, features)| {
                            (
                                name,
                                features
                                    .iter()
                                    .filter(|feature| edit_distance_test(**feature, *dep_feature))
                                    .collect::<Vec<_>>(),
                            )
                        })
                        .map(|(name, features)| {
                            features
                                .into_iter()
                                .map(move |feature| format!("{}/{}", name, feature))
                        })
                        .flatten();

                    // Finds set of `member/optional_dep` features which name is similar to current `pkg/feat`.
                    let optional_dependency_features = optional_dependency_names_per_member
                        .iter()
                        .filter(|(package, _)| edit_distance_test(package.name(), *dep_name))
                        .map(|(package, optional_dependencies)| {
                            optional_dependencies
                                .into_iter()
                                .filter(|optional_dependency| {
                                    edit_distance_test(**optional_dependency, *dep_name)
                                })
                                .map(move |optional_dependency| {
                                    format!("{}/{}", package.name(), optional_dependency)
                                })
                        })
                        .flatten();

                    // Finds set of `member/feat` features which name is similar to current `pkg/feat`.
                    let summary_features = summary_features_per_member
                        .iter()
                        .filter(|(package, _)| edit_distance_test(package.name(), *dep_name))
                        .map(|(package, summary_features)| {
                            summary_features
                                .into_iter()
                                .filter(|summary_feature| {
                                    edit_distance_test(**summary_feature, *dep_feature)
                                })
                                .map(move |summary_feature| {
                                    format!("{}/{}", package.name(), summary_feature)
                                })
                        })
                        .flatten();

                    pkg_feat_similar
                        .chain(optional_dependency_features)
                        .chain(summary_features)
                        .collect::<Vec<_>>()
                }
            })
            .map(|v| v.into_iter())
            .flatten()
            .unique()
            .filter(|element| {
                let feature = FeatureValue::new(element.into());
                !cli_features.features.contains(&feature) && !found_features.contains(&feature)
            })
            .sorted()
            .take(5)
            .collect()
    }

    fn report_unknown_features_error(
        &self,
        specs: &[PackageIdSpec],
        cli_features: &CliFeatures,
        found_features: &BTreeSet<FeatureValue>,
    ) -> CargoResult<()> {
        let unknown: Vec<_> = cli_features
            .features
            .difference(found_features)
            .map(|feature| feature.to_string())
            .sorted()
            .collect();

        let (selected_members, unselected_members): (Vec<_>, Vec<_>) = self
            .members()
            .partition(|member| specs.iter().any(|spec| spec.matches(member.package_id())));

        let missing_packages_with_the_features = unselected_members
            .into_iter()
            .filter(|member| {
                unknown
                    .iter()
                    .any(|feature| member.summary().features().contains_key(&**feature))
            })
            .map(|m| m.name())
            .collect_vec();

        let these_features = if unknown.len() == 1 {
            "this feature"
        } else {
            "these features"
        };
        let mut msg = if let [singular] = &selected_members[..] {
            format!(
                "the package '{}' does not contain {these_features}: {}",
                singular.name(),
                unknown.join(", ")
            )
        } else {
            let names = selected_members.iter().map(|m| m.name()).join(", ");
            format!(
                "none of the selected packages contains {these_features}: {}\nselected packages: {names}",
                unknown.join(", ")
            )
        };

        use std::fmt::Write;
        if !missing_packages_with_the_features.is_empty() {
            write!(
                &mut msg,
                "\nhelp: package{} with the missing feature{}: {}",
                if missing_packages_with_the_features.len() != 1 {
                    "s"
                } else {
                    ""
                },
                if unknown.len() != 1 { "s" } else { "" },
                missing_packages_with_the_features.join(", ")
            )?;
        } else {
            let suggestions = self.missing_feature_spelling_suggestions(
                &selected_members,
                cli_features,
                found_features,
            );
            if !suggestions.is_empty() {
                write!(
                    &mut msg,
                    "\nhelp: there {}: {}",
                    if suggestions.len() == 1 {
                        "is a similarly named feature"
                    } else {
                        "are similarly named features"
                    },
                    suggestions.join(", ")
                )?;
            }
        }

        bail!("{msg}")
    }

    /// New command-line feature selection behavior with resolver = "2" or the
    /// root of a virtual workspace. See `allows_new_cli_feature_behavior`.
    fn members_with_features_new(
        &self,
        specs: &[PackageIdSpec],
        cli_features: &CliFeatures,
    ) -> CargoResult<Vec<(&Package, CliFeatures)>> {
        // Keeps track of which features matched `member` to produce an error
        // if any of them did not match anywhere.
        let mut found_features = Default::default();

        let members: Vec<(&Package, CliFeatures)> = self
            .members()
            .filter(|m| specs.iter().any(|spec| spec.matches(m.package_id())))
            .map(|m| {
                (
                    m,
                    Workspace::collect_matching_features(m, cli_features, &mut found_features),
                )
            })
            .collect();

        if members.is_empty() {
            // `cargo build -p foo`, where `foo` is not a member.
            // Do not allow any command-line flags (defaults only).
            if !(cli_features.features.is_empty()
                && !cli_features.all_features
                && cli_features.uses_default_features)
            {
                bail!("cannot specify features for packages outside of workspace");
            }
            // Add all members from the workspace so we can ensure `-p nonmember`
            // is in the resolve graph.
            return Ok(self
                .members()
                .map(|m| (m, CliFeatures::new_all(false)))
                .collect());
        }
        if *cli_features.features != found_features {
            self.report_unknown_features_error(specs, cli_features, &found_features)?;
        }
        Ok(members)
    }

    /// This is the "old" behavior for command-line feature selection.
    /// See `allows_new_cli_feature_behavior`.
    fn members_with_features_old(
        &self,
        specs: &[PackageIdSpec],
        cli_features: &CliFeatures,
    ) -> Vec<(&Package, CliFeatures)> {
        // Split off any features with the syntax `member-name/feature-name` into a map
        // so that those features can be applied directly to those workspace-members.
        let mut member_specific_features: HashMap<InternedString, BTreeSet<FeatureValue>> =
            HashMap::new();
        // Features for the member in the current directory.
        let mut cwd_features = BTreeSet::new();
        for feature in cli_features.features.iter() {
            match feature {
                FeatureValue::Feature(_) => {
                    cwd_features.insert(feature.clone());
                }
                // This should be enforced by CliFeatures.
                FeatureValue::Dep { .. } => panic!("unexpected dep: syntax {}", feature),
                FeatureValue::DepFeature {
                    dep_name,
                    dep_feature,
                    weak: _,
                } => {
                    // I think weak can be ignored here.
                    // * With `--features member?/feat -p member`, the ? doesn't
                    //   really mean anything (either the member is built or it isn't).
                    // * With `--features nonmember?/feat`, cwd_features will
                    //   handle processing it correctly.
                    let is_member = self.members().any(|member| {
                        // Check if `dep_name` is member of the workspace, but isn't associated with current package.
                        self.current_opt() != Some(member) && member.name() == *dep_name
                    });
                    if is_member && specs.iter().any(|spec| spec.name() == dep_name.as_str()) {
                        member_specific_features
                            .entry(*dep_name)
                            .or_default()
                            .insert(FeatureValue::Feature(*dep_feature));
                    } else {
                        cwd_features.insert(feature.clone());
                    }
                }
            }
        }

        let ms: Vec<_> = self
            .members()
            .filter_map(|member| {
                let member_id = member.package_id();
                match self.current_opt() {
                    // The features passed on the command-line only apply to
                    // the "current" package (determined by the cwd).
                    Some(current) if member_id == current.package_id() => {
                        let feats = CliFeatures {
                            features: Rc::new(cwd_features.clone()),
                            all_features: cli_features.all_features,
                            uses_default_features: cli_features.uses_default_features,
                        };
                        Some((member, feats))
                    }
                    _ => {
                        // Ignore members that are not enabled on the command-line.
                        if specs.iter().any(|spec| spec.matches(member_id)) {
                            // -p for a workspace member that is not the "current"
                            // one.
                            //
                            // The odd behavior here is due to backwards
                            // compatibility. `--features` and
                            // `--no-default-features` used to only apply to the
                            // "current" package. As an extension, this allows
                            // member-name/feature-name to set member-specific
                            // features, which should be backwards-compatible.
                            let feats = CliFeatures {
                                features: Rc::new(
                                    member_specific_features
                                        .remove(member.name().as_str())
                                        .unwrap_or_default(),
                                ),
                                uses_default_features: true,
                                all_features: cli_features.all_features,
                            };
                            Some((member, feats))
                        } else {
                            // This member was not requested on the command-line, skip.
                            None
                        }
                    }
                }
            })
            .collect();

        // If any member specific features were not removed while iterating over members
        // some features will be ignored.
        assert!(member_specific_features.is_empty());

        ms
    }

    /// Returns true if `unit` should depend on the output of Docscrape units.
    pub fn unit_needs_doc_scrape(&self, unit: &Unit) -> bool {
        // We do not add scraped units for Host units, as they're either build scripts
        // (not documented) or proc macros (have no scrape-able exports). Additionally,
        // naively passing a proc macro's unit_for to new_unit_dep will currently cause
        // Cargo to panic, see issue #10545.
        self.is_member(&unit.pkg) && !(unit.target.for_host() || unit.pkg.proc_macro())
    }

    /// Adds a local package registry overlaying a `SourceId`.
    ///
    /// See [`crate::sources::overlay::DependencyConfusionThreatOverlaySource`] for why you shouldn't use this.
    pub fn add_local_overlay(&mut self, id: SourceId, registry_path: PathBuf) {
        self.local_overlays.insert(id, registry_path);
    }

    /// Builds a package registry that reflects this workspace configuration.
    pub fn package_registry(&self) -> CargoResult<PackageRegistry<'gctx>> {
        let source_config =
            SourceConfigMap::new_with_overlays(self.gctx(), self.local_overlays()?)?;
        PackageRegistry::new_with_source_config(self.gctx(), source_config)
    }

    /// Returns all the configured local overlays, including the ones from our secret environment variable.
    fn local_overlays(&self) -> CargoResult<impl Iterator<Item = (SourceId, SourceId)>> {
        let mut ret = self
            .local_overlays
            .iter()
            .map(|(id, path)| Ok((*id, SourceId::for_local_registry(path)?)))
            .collect::<CargoResult<Vec<_>>>()?;

        if let Ok(overlay) = self
            .gctx
            .get_env("__CARGO_TEST_DEPENDENCY_CONFUSION_VULNERABILITY_DO_NOT_USE_THIS")
        {
            let (url, path) = overlay.split_once('=').ok_or(anyhow::anyhow!(
                "invalid overlay format. I won't tell you why; you shouldn't be using it anyway"
            ))?;
            ret.push((
                SourceId::from_url(url)?,
                SourceId::for_local_registry(path.as_ref())?,
            ));
        }

        Ok(ret.into_iter())
    }
}

impl<'gctx> Packages<'gctx> {
    fn get(&self, manifest_path: &Path) -> &MaybePackage {
        self.maybe_get(manifest_path).unwrap()
    }

    fn get_mut(&mut self, manifest_path: &Path) -> &mut MaybePackage {
        self.maybe_get_mut(manifest_path).unwrap()
    }

    fn maybe_get(&self, manifest_path: &Path) -> Option<&MaybePackage> {
        self.packages.get(manifest_path)
    }

    fn maybe_get_mut(&mut self, manifest_path: &Path) -> Option<&mut MaybePackage> {
        self.packages.get_mut(manifest_path)
    }

    fn load(&mut self, manifest_path: &Path) -> CargoResult<&MaybePackage> {
        match self.packages.entry(manifest_path.to_path_buf()) {
            Entry::Occupied(e) => Ok(e.into_mut()),
            Entry::Vacant(v) => {
                let source_id = SourceId::for_manifest_path(manifest_path)?;
                let manifest = read_manifest(manifest_path, source_id, self.gctx)?;
                Ok(v.insert(match manifest {
                    EitherManifest::Real(manifest) => {
                        MaybePackage::Package(Package::new(manifest, manifest_path))
                    }
                    EitherManifest::Virtual(vm) => MaybePackage::Virtual(vm),
                }))
            }
        }
    }
}

impl MaybePackage {
    fn workspace_config(&self) -> &WorkspaceConfig {
        match *self {
            MaybePackage::Package(ref p) => p.manifest().workspace_config(),
            MaybePackage::Virtual(ref vm) => vm.workspace_config(),
        }
    }

    /// Has an embedded manifest (single-file package)
    pub fn is_embedded(&self) -> bool {
        match self {
            MaybePackage::Package(p) => p.manifest().is_embedded(),
            MaybePackage::Virtual(_) => false,
        }
    }

    pub fn contents(&self) -> &str {
        match self {
            MaybePackage::Package(p) => p.manifest().contents(),
            MaybePackage::Virtual(v) => v.contents(),
        }
    }

    pub fn document(&self) -> &toml::Spanned<toml::de::DeTable<'static>> {
        match self {
            MaybePackage::Package(p) => p.manifest().document(),
            MaybePackage::Virtual(v) => v.document(),
        }
    }

    pub fn edition(&self) -> Edition {
        match self {
            MaybePackage::Package(p) => p.manifest().edition(),
            MaybePackage::Virtual(_) => Edition::default(),
        }
    }

    pub fn profiles(&self) -> Option<&TomlProfiles> {
        match self {
            MaybePackage::Package(p) => p.manifest().profiles(),
            MaybePackage::Virtual(v) => v.profiles(),
        }
    }

    pub fn unstable_features(&self) -> &Features {
        match self {
            MaybePackage::Package(p) => p.manifest().unstable_features(),
            MaybePackage::Virtual(vm) => vm.unstable_features(),
        }
    }
}

impl WorkspaceRootConfig {
    /// Creates a new Intermediate Workspace Root configuration.
    pub fn new(
        root_dir: &Path,
        members: &Option<Vec<String>>,
        default_members: &Option<Vec<String>>,
        exclude: &Option<Vec<String>>,
        inheritable: &Option<InheritableFields>,
        custom_metadata: &Option<toml::Value>,
    ) -> WorkspaceRootConfig {
        WorkspaceRootConfig {
            root_dir: root_dir.to_path_buf(),
            members: members.clone(),
            default_members: default_members.clone(),
            exclude: exclude.clone().unwrap_or_default(),
            inheritable_fields: inheritable.clone().unwrap_or_default(),
            custom_metadata: custom_metadata.clone(),
        }
    }
    /// Checks the path against the `excluded` list.
    ///
    /// This method does **not** consider the `members` list.
    fn is_excluded(&self, manifest_path: &Path) -> bool {
        let excluded = self
            .exclude
            .iter()
            .any(|ex| manifest_path.starts_with(self.root_dir.join(ex)));

        let explicit_member = match self.members {
            Some(ref members) => members
                .iter()
                .any(|mem| manifest_path.starts_with(self.root_dir.join(mem))),
            None => false,
        };

        !explicit_member && excluded
    }

    fn has_members_list(&self) -> bool {
        self.members.is_some()
    }

    /// Returns expanded paths along with the glob that they were expanded from.
    /// The glob is `None` if the path matched exactly.
    #[tracing::instrument(skip_all)]
    fn members_paths<'g>(
        &self,
        globs: &'g [String],
    ) -> CargoResult<Vec<(PathBuf, Option<&'g str>)>> {
        let mut expanded_list = Vec::new();

        for glob in globs {
            let pathbuf = self.root_dir.join(glob);
            let expanded_paths = Self::expand_member_path(&pathbuf)?;

            // If glob does not find any valid paths, then put the original
            // path in the expanded list to maintain backwards compatibility.
            if expanded_paths.is_empty() {
                expanded_list.push((pathbuf, None));
            } else {
                let used_glob_pattern = expanded_paths.len() > 1 || expanded_paths[0] != pathbuf;
                let glob = used_glob_pattern.then_some(glob.as_str());

                // Some OS can create system support files anywhere.
                // (e.g. macOS creates `.DS_Store` file if you visit a directory using Finder.)
                // Such files can be reported as a member path unexpectedly.
                // Check and filter out non-directory paths to prevent pushing such accidental unwanted path
                // as a member.
                for expanded_path in expanded_paths {
                    if expanded_path.is_dir() {
                        expanded_list.push((expanded_path, glob));
                    }
                }
            }
        }

        Ok(expanded_list)
    }

    fn expand_member_path(path: &Path) -> CargoResult<Vec<PathBuf>> {
        let Some(path) = path.to_str() else {
            return Ok(Vec::new());
        };
        let res = glob(path).with_context(|| format!("could not parse pattern `{}`", &path))?;
        let res = res
            .map(|p| p.with_context(|| format!("unable to match path to pattern `{}`", &path)))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(res)
    }

    pub fn inheritable(&self) -> &InheritableFields {
        &self.inheritable_fields
    }
}

pub fn resolve_relative_path(
    label: &str,
    old_root: &Path,
    new_root: &Path,
    rel_path: &str,
) -> CargoResult<String> {
    let joined_path = normalize_path(&old_root.join(rel_path));
    match diff_paths(joined_path, new_root) {
        None => Err(anyhow!(
            "`{}` was defined in {} but could not be resolved with {}",
            label,
            old_root.display(),
            new_root.display()
        )),
        Some(path) => Ok(path
            .to_str()
            .ok_or_else(|| {
                anyhow!(
                    "`{}` resolved to non-UTF value (`{}`)",
                    label,
                    path.display()
                )
            })?
            .to_owned()),
    }
}

/// Finds the path of the root of the workspace.
pub fn find_workspace_root(
    manifest_path: &Path,
    gctx: &GlobalContext,
) -> CargoResult<Option<PathBuf>> {
    find_workspace_root_with_loader(manifest_path, gctx, |self_path| {
        let source_id = SourceId::for_manifest_path(self_path)?;
        let manifest = read_manifest(self_path, source_id, gctx)?;
        Ok(manifest
            .workspace_config()
            .get_ws_root(self_path, manifest_path))
    })
}

/// Finds the path of the root of the workspace.
///
/// This uses a callback to determine if the given path tells us what the
/// workspace root is.
fn find_workspace_root_with_loader(
    manifest_path: &Path,
    gctx: &GlobalContext,
    mut loader: impl FnMut(&Path) -> CargoResult<Option<PathBuf>>,
) -> CargoResult<Option<PathBuf>> {
    // Check if there are any workspace roots that have already been found that would work
    {
        let roots = gctx.ws_roots();
        // Iterate through the manifests parent directories until we find a workspace
        // root. Note we skip the first item since that is just the path itself
        for current in manifest_path.ancestors().skip(1) {
            if let Some(ws_config) = roots.get(current) {
                if !ws_config.is_excluded(manifest_path) {
                    // Add `Cargo.toml` since ws_root is the root and not the file
                    return Ok(Some(current.join("Cargo.toml")));
                }
            }
        }
    }

    for ances_manifest_path in find_root_iter(manifest_path, gctx) {
        debug!("find_root - trying {}", ances_manifest_path.display());
        if let Some(ws_root_path) = loader(&ances_manifest_path)? {
            return Ok(Some(ws_root_path));
        }
    }
    Ok(None)
}

fn read_root_pointer(member_manifest: &Path, root_link: &str) -> PathBuf {
    let path = member_manifest
        .parent()
        .unwrap()
        .join(root_link)
        .join("Cargo.toml");
    debug!("find_root - pointer {}", path.display());
    paths::normalize_path(&path)
}

fn find_root_iter<'a>(
    manifest_path: &'a Path,
    gctx: &'a GlobalContext,
) -> impl Iterator<Item = PathBuf> + 'a {
    LookBehind::new(paths::ancestors(manifest_path, None).skip(2))
        .take_while(|path| !path.curr.ends_with("target/package"))
        // Don't walk across `CARGO_HOME` when we're looking for the
        // workspace root. Sometimes a package will be organized with
        // `CARGO_HOME` pointing inside of the workspace root or in the
        // current package, but we don't want to mistakenly try to put
        // crates.io crates into the workspace by accident.
        .take_while(|path| {
            if let Some(last) = path.last {
                gctx.home() != last
            } else {
                true
            }
        })
        .map(|path| path.curr.join("Cargo.toml"))
        .filter(|ances_manifest_path| ances_manifest_path.exists())
}

struct LookBehindWindow<'a, T: ?Sized> {
    curr: &'a T,
    last: Option<&'a T>,
}

struct LookBehind<'a, T: ?Sized, K: Iterator<Item = &'a T>> {
    iter: K,
    last: Option<&'a T>,
}

impl<'a, T: ?Sized, K: Iterator<Item = &'a T>> LookBehind<'a, T, K> {
    fn new(items: K) -> Self {
        Self {
            iter: items,
            last: None,
        }
    }
}

impl<'a, T: ?Sized, K: Iterator<Item = &'a T>> Iterator for LookBehind<'a, T, K> {
    type Item = LookBehindWindow<'a, T>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.iter.next() {
            None => None,
            Some(next) => {
                let last = self.last;
                self.last = Some(next);
                Some(LookBehindWindow { curr: next, last })
            }
        }
    }
}
