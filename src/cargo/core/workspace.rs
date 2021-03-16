use std::cell::RefCell;
use std::collections::hash_map::{Entry, HashMap};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::slice;

use glob::glob;
use log::debug;
use url::Url;

use crate::core::features::Features;
use crate::core::registry::PackageRegistry;
use crate::core::resolver::features::RequestedFeatures;
use crate::core::resolver::ResolveBehavior;
use crate::core::{Dependency, PackageId, PackageIdSpec};
use crate::core::{EitherManifest, Package, SourceId, VirtualManifest};
use crate::ops;
use crate::sources::PathSource;
use crate::util::errors::{CargoResult, CargoResultExt, ManifestError};
use crate::util::interning::InternedString;
use crate::util::paths;
use crate::util::toml::{read_manifest, TomlProfiles};
use crate::util::{Config, Filesystem};

/// The core abstraction in Cargo for working with a workspace of crates.
///
/// A workspace is often created very early on and then threaded through all
/// other functions. It's typically through this object that the current
/// package is loaded and/or learned about.
#[derive(Debug)]
pub struct Workspace<'cfg> {
    config: &'cfg Config,

    // This path is a path to where the current cargo subcommand was invoked
    // from. That is the `--manifest-path` argument to Cargo, and
    // points to the "main crate" that we're going to worry about.
    current_manifest: PathBuf,

    // A list of packages found in this workspace. Always includes at least the
    // package mentioned by `current_manifest`.
    packages: Packages<'cfg>,

    // If this workspace includes more than one crate, this points to the root
    // of the workspace. This is `None` in the case that `[workspace]` is
    // missing, `package.workspace` is missing, and no `Cargo.toml` above
    // `current_manifest` was found on the filesystem with `[workspace]`.
    root_manifest: Option<PathBuf>,

    // Shared target directory for all the packages of this workspace.
    // `None` if the default path of `root/target` should be used.
    target_dir: Option<Filesystem>,

    // List of members in this workspace with a listing of all their manifest
    // paths. The packages themselves can be looked up through the `packages`
    // set above.
    members: Vec<PathBuf>,
    member_ids: HashSet<PackageId>,

    // The subset of `members` that are used by the
    // `build`, `check`, `test`, and `bench` subcommands
    // when no package is selected with `--package` / `-p` and `--workspace`
    // is not used.
    //
    // This is set by the `default-members` config
    // in the `[workspace]` section.
    // When unset, this is the same as `members` for virtual workspaces
    // (`--workspace` is implied)
    // or only the root package for non-virtual workspaces.
    default_members: Vec<PathBuf>,

    // `true` if this is a temporary workspace created for the purposes of the
    // `cargo install` or `cargo package` commands.
    is_ephemeral: bool,

    // `true` if this workspace should enforce optional dependencies even when
    // not needed; false if this workspace should only enforce dependencies
    // needed by the current configuration (such as in cargo install). In some
    // cases `false` also results in the non-enforcement of dev-dependencies.
    require_optional_deps: bool,

    // A cache of loaded packages for particular paths which is disjoint from
    // `packages` up above, used in the `load` method down below.
    loaded_packages: RefCell<HashMap<PathBuf, Package>>,

    // If `true`, then the resolver will ignore any existing `Cargo.lock`
    // file. This is set for `cargo install` without `--locked`.
    ignore_lock: bool,

    /// The resolver behavior specified with the `resolver` field.
    resolve_behavior: Option<ResolveBehavior>,

    /// Workspace-level custom metadata
    custom_metadata: Option<toml::Value>,
}

// Separate structure for tracking loaded packages (to avoid loading anything
// twice), and this is separate to help appease the borrow checker.
#[derive(Debug)]
struct Packages<'cfg> {
    config: &'cfg Config,
    packages: HashMap<PathBuf, MaybePackage>,
}

#[derive(Debug)]
enum MaybePackage {
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
    custom_metadata: Option<toml::Value>,
}

/// An iterator over the member packages of a workspace, returned by
/// `Workspace::members`
pub struct Members<'a, 'cfg> {
    ws: &'a Workspace<'cfg>,
    iter: slice::Iter<'a, PathBuf>,
}

impl<'cfg> Workspace<'cfg> {
    /// Creates a new workspace given the target manifest pointed to by
    /// `manifest_path`.
    ///
    /// This function will construct the entire workspace by determining the
    /// root and all member packages. It will then validate the workspace
    /// before returning it, so `Ok` is only returned for valid workspaces.
    pub fn new(manifest_path: &Path, config: &'cfg Config) -> CargoResult<Workspace<'cfg>> {
        let mut ws = Workspace::new_default(manifest_path.to_path_buf(), config);
        ws.target_dir = config.target_dir()?;

        if manifest_path.is_relative() {
            anyhow::bail!(
                "manifest_path:{:?} is not an absolute path. Please provide an absolute path.",
                manifest_path
            )
        } else {
            ws.root_manifest = ws.find_root(manifest_path)?;
        }

        ws.custom_metadata = ws
            .load_workspace_config()?
            .and_then(|cfg| cfg.custom_metadata);
        ws.find_members()?;
        ws.resolve_behavior = match ws.root_maybe() {
            MaybePackage::Package(p) => p.manifest().resolve_behavior(),
            MaybePackage::Virtual(vm) => vm.resolve_behavior(),
        };
        ws.validate()?;
        Ok(ws)
    }

    fn new_default(current_manifest: PathBuf, config: &'cfg Config) -> Workspace<'cfg> {
        Workspace {
            config,
            current_manifest,
            packages: Packages {
                config,
                packages: HashMap::new(),
            },
            root_manifest: None,
            target_dir: None,
            members: Vec::new(),
            member_ids: HashSet::new(),
            default_members: Vec::new(),
            is_ephemeral: false,
            require_optional_deps: true,
            loaded_packages: RefCell::new(HashMap::new()),
            ignore_lock: false,
            resolve_behavior: None,
            custom_metadata: None,
        }
    }

    pub fn new_virtual(
        root_path: PathBuf,
        current_manifest: PathBuf,
        manifest: VirtualManifest,
        config: &'cfg Config,
    ) -> CargoResult<Workspace<'cfg>> {
        let mut ws = Workspace::new_default(current_manifest, config);
        ws.root_manifest = Some(root_path.join("Cargo.toml"));
        ws.target_dir = config.target_dir()?;
        ws.resolve_behavior = manifest.resolve_behavior();
        ws.packages
            .packages
            .insert(root_path, MaybePackage::Virtual(manifest));
        ws.find_members()?;
        // TODO: validation does not work because it walks up the directory
        // tree looking for the root which is a fake file that doesn't exist.
        Ok(ws)
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
        config: &'cfg Config,
        target_dir: Option<Filesystem>,
        require_optional_deps: bool,
    ) -> CargoResult<Workspace<'cfg>> {
        let mut ws = Workspace::new_default(package.manifest_path().to_path_buf(), config);
        ws.is_ephemeral = true;
        ws.require_optional_deps = require_optional_deps;
        ws.resolve_behavior = package.manifest().resolve_behavior();
        let key = ws.current_manifest.parent().unwrap();
        let id = package.package_id();
        let package = MaybePackage::Package(package);
        ws.packages.packages.insert(key.to_path_buf(), package);
        ws.target_dir = if let Some(dir) = target_dir {
            Some(dir)
        } else {
            ws.config.target_dir()?
        };
        ws.members.push(ws.current_manifest.clone());
        ws.member_ids.insert(id);
        ws.default_members.push(ws.current_manifest.clone());
        Ok(ws)
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

    /// Returns the `Config` this workspace is associated with.
    pub fn config(&self) -> &'cfg Config {
        self.config
    }

    pub fn profiles(&self) -> Option<&TomlProfiles> {
        match self.root_maybe() {
            MaybePackage::Package(p) => p.manifest().profiles(),
            MaybePackage::Virtual(vm) => vm.profiles(),
        }
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

    /// Returns the root Package or VirtualManifest.
    fn root_maybe(&self) -> &MaybePackage {
        self.packages.get(self.root_manifest())
    }

    pub fn target_dir(&self) -> Filesystem {
        self.target_dir
            .clone()
            .unwrap_or_else(|| Filesystem::new(self.root().join("target")))
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

    /// Returns the root `[patch]` section of this workspace.
    ///
    /// This may be from a virtual crate or an actual crate.
    pub fn root_patch(&self) -> &HashMap<Url, Vec<Dependency>> {
        match self.root_maybe() {
            MaybePackage::Package(p) => p.manifest().patch(),
            MaybePackage::Virtual(vm) => vm.patch(),
        }
    }

    /// Returns an iterator over all packages in this workspace
    pub fn members<'a>(&'a self) -> Members<'a, 'cfg> {
        Members {
            ws: self,
            iter: self.members.iter(),
        }
    }

    /// Returns an iterator over default packages in this workspace
    pub fn default_members<'a>(&'a self) -> Members<'a, 'cfg> {
        Members {
            ws: self,
            iter: self.default_members.iter(),
        }
    }

    /// Returns true if the package is a member of the workspace.
    pub fn is_member(&self, pkg: &Package) -> bool {
        self.member_ids.contains(&pkg.package_id())
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
    ) -> &mut Workspace<'cfg> {
        self.require_optional_deps = require_optional_deps;
        self
    }

    pub fn ignore_lock(&self) -> bool {
        self.ignore_lock
    }

    pub fn set_ignore_lock(&mut self, ignore_lock: bool) -> &mut Workspace<'cfg> {
        self.ignore_lock = ignore_lock;
        self
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
                WorkspaceConfig::Root(ref root_config) => {
                    return Ok(Some(root_config.clone()));
                }

                _ => anyhow::bail!(
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
        fn read_root_pointer(member_manifest: &Path, root_link: &str) -> PathBuf {
            let path = member_manifest
                .parent()
                .unwrap()
                .join(root_link)
                .join("Cargo.toml");
            debug!("find_root - pointer {}", path.display());
            paths::normalize_path(&path)
        }

        {
            let current = self.packages.load(manifest_path)?;
            match *current.workspace_config() {
                WorkspaceConfig::Root(_) => {
                    debug!("find_root - is root {}", manifest_path.display());
                    return Ok(Some(manifest_path.to_path_buf()));
                }
                WorkspaceConfig::Member {
                    root: Some(ref path_to_root),
                } => return Ok(Some(read_root_pointer(manifest_path, path_to_root))),
                WorkspaceConfig::Member { root: None } => {}
            }
        }

        for path in paths::ancestors(manifest_path).skip(2) {
            if path.ends_with("target/package") {
                break;
            }

            let ances_manifest_path = path.join("Cargo.toml");
            debug!("find_root - trying {}", ances_manifest_path.display());
            if ances_manifest_path.exists() {
                match *self.packages.load(&ances_manifest_path)?.workspace_config() {
                    WorkspaceConfig::Root(ref ances_root_config) => {
                        debug!("find_root - found a root checking exclusion");
                        if !ances_root_config.is_excluded(manifest_path) {
                            debug!("find_root - found!");
                            return Ok(Some(ances_manifest_path));
                        }
                    }
                    WorkspaceConfig::Member {
                        root: Some(ref path_to_root),
                    } => {
                        debug!("find_root - found pointer");
                        return Ok(Some(read_root_pointer(&ances_manifest_path, path_to_root)));
                    }
                    WorkspaceConfig::Member { .. } => {}
                }
            }

            // Don't walk across `CARGO_HOME` when we're looking for the
            // workspace root. Sometimes a package will be organized with
            // `CARGO_HOME` pointing inside of the workspace root or in the
            // current package, but we don't want to mistakenly try to put
            // crates.io crates into the workspace by accident.
            if self.config.home() == path {
                break;
            }
        }

        Ok(None)
    }

    /// After the root of a workspace has been located, probes for all members
    /// of a workspace.
    ///
    /// If the `workspace.members` configuration is present, then this just
    /// verifies that those are all valid packages to point to. Otherwise, this
    /// will transitively follow all `path` dependencies looking for members of
    /// the workspace.
    fn find_members(&mut self) -> CargoResult<()> {
        let workspace_config = match self.load_workspace_config()? {
            Some(workspace_config) => workspace_config,
            None => {
                debug!("find_members - only me as a member");
                self.members.push(self.current_manifest.clone());
                self.default_members.push(self.current_manifest.clone());
                if let Ok(pkg) = self.current() {
                    let id = pkg.package_id();
                    self.member_ids.insert(id);
                }
                return Ok(());
            }
        };

        // self.root_manifest must be Some to have retrieved workspace_config
        let root_manifest_path = self.root_manifest.clone().unwrap();

        let members_paths =
            workspace_config.members_paths(workspace_config.members.as_ref().unwrap_or(&vec![]))?;
        let default_members_paths = if root_manifest_path == self.current_manifest {
            if let Some(ref default) = workspace_config.default_members {
                Some(workspace_config.members_paths(default)?)
            } else {
                None
            }
        } else {
            None
        };

        for path in &members_paths {
            self.find_path_deps(&path.join("Cargo.toml"), &root_manifest_path, false)?;
        }

        if let Some(default) = default_members_paths {
            for path in default {
                let normalized_path = paths::normalize_path(&path);
                let manifest_path = normalized_path.join("Cargo.toml");
                if !self.members.contains(&manifest_path) {
                    // default-members are allowed to be excluded, but they
                    // still must be referred to by the original (unfiltered)
                    // members list. Note that we aren't testing against the
                    // manifest path, both because `members_paths` doesn't
                    // include `/Cargo.toml`, and because excluded paths may not
                    // be crates.
                    let exclude = members_paths.contains(&normalized_path)
                        && workspace_config.is_excluded(&normalized_path);
                    if exclude {
                        continue;
                    }
                    anyhow::bail!(
                        "package `{}` is listed in workspace’s default-members \
                         but is not a member.",
                        path.display()
                    )
                }
                self.default_members.push(manifest_path)
            }
        } else if self.is_virtual() {
            self.default_members = self.members.clone()
        } else {
            self.default_members.push(self.current_manifest.clone())
        }

        self.find_path_deps(&root_manifest_path, &root_manifest_path, false)
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

        debug!("find_members - {}", manifest_path.display());
        self.members.push(manifest_path.clone());

        let candidates = {
            let pkg = match *self.packages.load(&manifest_path)? {
                MaybePackage::Package(ref p) => p,
                MaybePackage::Virtual(_) => return Ok(()),
            };
            self.member_ids.insert(pkg.package_id());
            pkg.dependencies()
                .iter()
                .map(|d| d.source_id())
                .filter(|d| d.is_path())
                .filter_map(|d| d.url().to_file_path().ok())
                .map(|p| p.join("Cargo.toml"))
                .collect::<Vec<_>>()
        };
        for candidate in candidates {
            self.find_path_deps(&candidate, root_manifest, true)
                .map_err(|err| ManifestError::new(err, manifest_path.clone()))?;
        }
        Ok(())
    }

    /// Returns the unstable nightly-only features enabled via `cargo-features` in the manifest.
    pub fn unstable_features(&self) -> &Features {
        match self.root_maybe() {
            MaybePackage::Package(p) => p.manifest().unstable_features(),
            MaybePackage::Virtual(vm) => vm.unstable_features(),
        }
    }

    pub fn resolve_behavior(&self) -> ResolveBehavior {
        self.resolve_behavior.unwrap_or(ResolveBehavior::V1)
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
                ResolveBehavior::V2 => true,
            }
    }

    /// Validates a workspace, ensuring that a number of invariants are upheld:
    ///
    /// 1. A workspace only has one root.
    /// 2. All workspace members agree on this one root as the root.
    /// 3. The current crate is a member of this workspace.
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
                anyhow::bail!(
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
            0 => anyhow::bail!(
                "`package.workspace` configuration points to a crate \
                 which is not configured with [workspace]: \n\
                 configuration at: {}\n\
                 points to: {}",
                self.current_manifest.display(),
                self.root_manifest.as_ref().unwrap().display()
            ),
            _ => {
                anyhow::bail!(
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

    fn validate_members(&mut self) -> CargoResult<()> {
        for member in self.members.clone() {
            let root = self.find_root(&member)?;
            if root == self.root_manifest {
                continue;
            }

            match root {
                Some(root) => {
                    anyhow::bail!(
                        "package `{}` is a member of the wrong workspace\n\
                         expected: {}\n\
                         actual:   {}",
                        member.display(),
                        self.root_manifest.as_ref().unwrap().display(),
                        root.display()
                    );
                }
                None => {
                    anyhow::bail!(
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
        anyhow::bail!(
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
                    self.config.shell().warn(&msg)
                };
                if manifest.original().has_profiles() {
                    emit_warning("profiles")?;
                }
                if !manifest.replace().is_empty() {
                    emit_warning("replace")?;
                }
                if !manifest.patch().is_empty() {
                    emit_warning("patch")?;
                }
                if manifest.resolve_behavior().is_some()
                    && manifest.resolve_behavior() != self.resolve_behavior
                {
                    // Only warn if they don't match.
                    emit_warning("resolver")?;
                }
            }
        }
        Ok(())
    }

    pub fn load(&self, manifest_path: &Path) -> CargoResult<Package> {
        match self.packages.maybe_get(manifest_path) {
            Some(&MaybePackage::Package(ref p)) => return Ok(p.clone()),
            Some(&MaybePackage::Virtual(_)) => anyhow::bail!("cannot load workspace root"),
            None => {}
        }

        let mut loaded = self.loaded_packages.borrow_mut();
        if let Some(p) = loaded.get(manifest_path).cloned() {
            return Ok(p);
        }
        let source_id = SourceId::for_path(manifest_path.parent().unwrap())?;
        let (package, _nested_paths) = ops::read_package(manifest_path, source_id, self.config)?;
        loaded.insert(manifest_path.to_path_buf(), package.clone());
        Ok(package)
    }

    /// Preload the provided registry with already loaded packages.
    ///
    /// A workspace may load packages during construction/parsing/early phases
    /// for various operations, and this preload step avoids doubly-loading and
    /// parsing crates on the filesystem by inserting them all into the registry
    /// with their in-memory formats.
    pub fn preload(&self, registry: &mut PackageRegistry<'cfg>) {
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
            let mut src = PathSource::new(pkg.root(), pkg.package_id().source_id(), self.config);
            src.preload_with(pkg);
            registry.add_preloaded(Box::new(src));
        }
    }

    pub fn emit_warnings(&self) -> CargoResult<()> {
        for (path, maybe_pkg) in &self.packages.packages {
            let warnings = match maybe_pkg {
                MaybePackage::Package(pkg) => pkg.manifest().warnings().warnings(),
                MaybePackage::Virtual(vm) => vm.warnings().warnings(),
            };
            let path = path.join("Cargo.toml");
            for warning in warnings {
                if warning.is_critical {
                    let err = anyhow::format_err!("{}", warning.message);
                    let cx =
                        anyhow::format_err!("failed to parse manifest at `{}`", path.display());
                    return Err(err.context(cx));
                } else {
                    let msg = if self.root_manifest.is_none() {
                        warning.message.to_string()
                    } else {
                        // In a workspace, it can be confusing where a warning
                        // originated, so include the path.
                        format!("{}: {}", path.display(), warning.message)
                    };
                    self.config.shell().warn(msg)?
                }
            }
        }
        Ok(())
    }

    pub fn set_target_dir(&mut self, target_dir: Filesystem) {
        self.target_dir = Some(target_dir);
    }

    /// Returns a Vec of `(&Package, RequestedFeatures)` tuples that
    /// represent the workspace members that were requested on the command-line.
    ///
    /// `specs` may be empty, which indicates it should return all workspace
    /// members. In this case, `requested_features.all_features` must be
    /// `true`. This is used for generating `Cargo.lock`, which must include
    /// all members with all features enabled.
    pub fn members_with_features(
        &self,
        specs: &[PackageIdSpec],
        requested_features: &RequestedFeatures,
    ) -> CargoResult<Vec<(&Package, RequestedFeatures)>> {
        assert!(
            !specs.is_empty() || requested_features.all_features,
            "no specs requires all_features"
        );
        if specs.is_empty() {
            // When resolving the entire workspace, resolve each member with
            // all features enabled.
            return Ok(self
                .members()
                .map(|m| (m, RequestedFeatures::new_all(true)))
                .collect());
        }
        if self.allows_new_cli_feature_behavior() {
            self.members_with_features_new(specs, requested_features)
        } else {
            Ok(self.members_with_features_old(specs, requested_features))
        }
    }

    /// New command-line feature selection behavior with resolver = "2" or the
    /// root of a virtual workspace. See `allows_new_cli_feature_behavior`.
    fn members_with_features_new(
        &self,
        specs: &[PackageIdSpec],
        requested_features: &RequestedFeatures,
    ) -> CargoResult<Vec<(&Package, RequestedFeatures)>> {
        // Keep track of which features matched *any* member, to produce an error
        // if any of them did not match anywhere.
        let mut found: BTreeSet<InternedString> = BTreeSet::new();

        // Returns the requested features for the given member.
        // This filters out any named features that the member does not have.
        let mut matching_features = |member: &Package| -> RequestedFeatures {
            if requested_features.features.is_empty() || requested_features.all_features {
                return requested_features.clone();
            }
            // Only include features this member defines.
            let summary = member.summary();
            let member_features = summary.features();
            let mut features = BTreeSet::new();

            // Checks if a member contains the given feature.
            let contains = |feature: InternedString| -> bool {
                member_features.contains_key(&feature)
                    || summary
                        .dependencies()
                        .iter()
                        .any(|dep| dep.is_optional() && dep.name_in_toml() == feature)
            };

            for feature in requested_features.features.iter() {
                let mut split = feature.splitn(2, '/');
                let split = (split.next().unwrap(), split.next());
                if let (pkg, Some(pkg_feature)) = split {
                    let pkg = InternedString::new(pkg);
                    let pkg_feature = InternedString::new(pkg_feature);
                    if summary
                        .dependencies()
                        .iter()
                        .any(|dep| dep.name_in_toml() == pkg)
                    {
                        // pkg/feat for a dependency.
                        // Will rely on the dependency resolver to validate `feat`.
                        features.insert(*feature);
                        found.insert(*feature);
                    } else if pkg == member.name() && contains(pkg_feature) {
                        // member/feat where "feat" is a feature in member.
                        features.insert(pkg_feature);
                        found.insert(*feature);
                    }
                } else if contains(*feature) {
                    // feature exists in this member.
                    features.insert(*feature);
                    found.insert(*feature);
                }
            }
            RequestedFeatures {
                features: Rc::new(features),
                all_features: false,
                uses_default_features: requested_features.uses_default_features,
            }
        };

        let members: Vec<(&Package, RequestedFeatures)> = self
            .members()
            .filter(|m| specs.iter().any(|spec| spec.matches(m.package_id())))
            .map(|m| (m, matching_features(m)))
            .collect();
        if members.is_empty() {
            // `cargo build -p foo`, where `foo` is not a member.
            // Do not allow any command-line flags (defaults only).
            if !(requested_features.features.is_empty()
                && !requested_features.all_features
                && requested_features.uses_default_features)
            {
                anyhow::bail!("cannot specify features for packages outside of workspace");
            }
            // Add all members from the workspace so we can ensure `-p nonmember`
            // is in the resolve graph.
            return Ok(self
                .members()
                .map(|m| (m, RequestedFeatures::new_all(false)))
                .collect());
        }
        if *requested_features.features != found {
            let missing: Vec<_> = requested_features
                .features
                .difference(&found)
                .copied()
                .collect();
            // TODO: typo suggestions would be good here.
            anyhow::bail!(
                "none of the selected packages contains these features: {}",
                missing.join(", ")
            );
        }
        Ok(members)
    }

    /// This is the "old" behavior for command-line feature selection.
    /// See `allows_new_cli_feature_behavior`.
    fn members_with_features_old(
        &self,
        specs: &[PackageIdSpec],
        requested_features: &RequestedFeatures,
    ) -> Vec<(&Package, RequestedFeatures)> {
        // Split off any features with the syntax `member-name/feature-name` into a map
        // so that those features can be applied directly to those workspace-members.
        let mut member_specific_features: HashMap<&str, BTreeSet<InternedString>> = HashMap::new();
        // Features for the member in the current directory.
        let mut cwd_features = BTreeSet::new();
        for feature in requested_features.features.iter() {
            if let Some(index) = feature.find('/') {
                let name = &feature[..index];
                let is_member = self.members().any(|member| member.name() == name);
                if is_member && specs.iter().any(|spec| spec.name() == name) {
                    member_specific_features
                        .entry(name)
                        .or_default()
                        .insert(InternedString::new(&feature[index + 1..]));
                } else {
                    cwd_features.insert(*feature);
                }
            } else {
                cwd_features.insert(*feature);
            };
        }

        let ms = self.members().filter_map(|member| {
            let member_id = member.package_id();
            match self.current_opt() {
                // The features passed on the command-line only apply to
                // the "current" package (determined by the cwd).
                Some(current) if member_id == current.package_id() => {
                    let feats = RequestedFeatures {
                        features: Rc::new(cwd_features.clone()),
                        all_features: requested_features.all_features,
                        uses_default_features: requested_features.uses_default_features,
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
                        let feats = RequestedFeatures {
                            features: Rc::new(
                                member_specific_features
                                    .remove(member.name().as_str())
                                    .unwrap_or_default(),
                            ),
                            uses_default_features: true,
                            all_features: requested_features.all_features,
                        };
                        Some((member, feats))
                    } else {
                        // This member was not requested on the command-line, skip.
                        None
                    }
                }
            }
        });
        ms.collect()
    }
}

impl<'cfg> Packages<'cfg> {
    fn get(&self, manifest_path: &Path) -> &MaybePackage {
        self.maybe_get(manifest_path).unwrap()
    }

    fn get_mut(&mut self, manifest_path: &Path) -> &mut MaybePackage {
        self.maybe_get_mut(manifest_path).unwrap()
    }

    fn maybe_get(&self, manifest_path: &Path) -> Option<&MaybePackage> {
        self.packages.get(manifest_path.parent().unwrap())
    }

    fn maybe_get_mut(&mut self, manifest_path: &Path) -> Option<&mut MaybePackage> {
        self.packages.get_mut(manifest_path.parent().unwrap())
    }

    fn load(&mut self, manifest_path: &Path) -> CargoResult<&MaybePackage> {
        let key = manifest_path.parent().unwrap();
        match self.packages.entry(key.to_path_buf()) {
            Entry::Occupied(e) => Ok(e.into_mut()),
            Entry::Vacant(v) => {
                let source_id = SourceId::for_path(key)?;
                let (manifest, _nested_paths) =
                    read_manifest(manifest_path, source_id, self.config)?;
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

impl<'a, 'cfg> Iterator for Members<'a, 'cfg> {
    type Item = &'a Package;

    fn next(&mut self) -> Option<&'a Package> {
        loop {
            let next = self.iter.next().map(|path| self.ws.packages.get(path));
            match next {
                Some(&MaybePackage::Package(ref p)) => return Some(p),
                Some(&MaybePackage::Virtual(_)) => {}
                None => return None,
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (_, upper) = self.iter.size_hint();
        (0, upper)
    }
}

impl MaybePackage {
    fn workspace_config(&self) -> &WorkspaceConfig {
        match *self {
            MaybePackage::Package(ref p) => p.manifest().workspace_config(),
            MaybePackage::Virtual(ref vm) => vm.workspace_config(),
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
        custom_metadata: &Option<toml::Value>,
    ) -> WorkspaceRootConfig {
        WorkspaceRootConfig {
            root_dir: root_dir.to_path_buf(),
            members: members.clone(),
            default_members: default_members.clone(),
            exclude: exclude.clone().unwrap_or_default(),
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

    fn members_paths(&self, globs: &[String]) -> CargoResult<Vec<PathBuf>> {
        let mut expanded_list = Vec::new();

        for glob in globs {
            let pathbuf = self.root_dir.join(glob);
            let expanded_paths = Self::expand_member_path(&pathbuf)?;

            // If glob does not find any valid paths, then put the original
            // path in the expanded list to maintain backwards compatibility.
            if expanded_paths.is_empty() {
                expanded_list.push(pathbuf);
            } else {
                // Some OS can create system support files anywhere.
                // (e.g. macOS creates `.DS_Store` file if you visit a directory using Finder.)
                // Such files can be reported as a member path unexpectedly.
                // Check and filter out non-directory paths to prevent pushing such accidental unwanted path
                // as a member.
                for expanded_path in expanded_paths {
                    if expanded_path.is_dir() {
                        expanded_list.push(expanded_path);
                    }
                }
            }
        }

        Ok(expanded_list)
    }

    fn expand_member_path(path: &Path) -> CargoResult<Vec<PathBuf>> {
        let path = match path.to_str() {
            Some(p) => p,
            None => return Ok(Vec::new()),
        };
        let res =
            glob(path).chain_err(|| anyhow::format_err!("could not parse pattern `{}`", &path))?;
        let res = res
            .map(|p| {
                p.chain_err(|| anyhow::format_err!("unable to match path to pattern `{}`", &path))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(res)
    }
}
