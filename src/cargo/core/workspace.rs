use std::collections::hash_map::{HashMap, Entry};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::slice;

use core::{Package, VirtualManifest, EitherManifest, SourceId};
use core::{PackageIdSpec, Dependency, Profile, Profiles};
use ops;
use util::{Config, CargoResult, Filesystem, human};
use util::paths;

/// The core abstraction in Cargo for working with a workspace of crates.
///
/// A workspace is often created very early on and then threaded through all
/// other functions. It's typically through this object that the current
/// package is loaded and/or learned about.
pub struct Workspace<'cfg> {
    config: &'cfg Config,

    // This path is a path to where the current cargo subcommand was invoked
    // from. That is, this is the `--manifest-path` argument to Cargo, and
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

    // True, if this is a temporary workspace created for the purposes of
    // cargo install or cargo package.
    is_ephemeral: bool,
}

// Separate structure for tracking loaded packages (to avoid loading anything
// twice), and this is separate to help appease the borrow checker.
struct Packages<'cfg> {
    config: &'cfg Config,
    packages: HashMap<PathBuf, MaybePackage>,
}

enum MaybePackage {
    Package(Package),
    Virtual(VirtualManifest),
}

/// Configuration of a workspace in a manifest.
#[derive(Debug, Clone)]
pub enum WorkspaceConfig {
    /// Indicates that `[workspace]` was present and the members were
    /// optionally specified as well.
    Root { members: Option<Vec<String>> },

    /// Indicates that `[workspace]` was present and the `root` field is the
    /// optional value of `package.workspace`, if present.
    Member { root: Option<String> },
}

/// An iterator over the member packages of a workspace, returned by
/// `Workspace::members`
pub struct Members<'a, 'cfg: 'a> {
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
    pub fn new(manifest_path: &Path, config: &'cfg Config)
               -> CargoResult<Workspace<'cfg>> {
        let target_dir = config.target_dir()?;

        let mut ws = Workspace {
            config: config,
            current_manifest: manifest_path.to_path_buf(),
            packages: Packages {
                config: config,
                packages: HashMap::new(),
            },
            root_manifest: None,
            target_dir: target_dir,
            members: Vec::new(),
            is_ephemeral: false,
        };
        ws.root_manifest = ws.find_root(manifest_path)?;
        ws.find_members()?;
        ws.validate()?;
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
    pub fn ephemeral(package: Package, config: &'cfg Config, target_dir: Option<Filesystem>)
                     -> CargoResult<Workspace<'cfg>> {
        let mut ws = Workspace {
            config: config,
            current_manifest: package.manifest_path().to_path_buf(),
            packages: Packages {
                config: config,
                packages: HashMap::new(),
            },
            root_manifest: None,
            target_dir: None,
            members: Vec::new(),
            is_ephemeral: true,
        };
        {
            let key = ws.current_manifest.parent().unwrap();
            let package = MaybePackage::Package(package);
            ws.packages.packages.insert(key.to_path_buf(), package);
            ws.target_dir = if let Some(dir) = target_dir {
                Some(dir)
            } else {
                ws.config.target_dir()?
            };
            ws.members.push(ws.current_manifest.clone());
        }
        return Ok(ws)
    }

    /// Returns the current package of this workspace.
    ///
    /// Note that this can return an error if it the current manifest is
    /// actually a "virtual Cargo.toml", in which case an error is returned
    /// indicating that something else should be passed.
    pub fn current(&self) -> CargoResult<&Package> {
        self.current_opt().ok_or_else(||
            human(format!("manifest path `{}` is a virtual manifest, but this \
                           command requires running against an actual package in \
                           this workspace", self.current_manifest.display()))
        )
    }

    pub fn current_opt(&self) -> Option<&Package> {
        match *self.packages.get(&self.current_manifest) {
            MaybePackage::Package(ref p) => Some(p),
            MaybePackage::Virtual(..) => None
        }
    }

    /// Returns the `Config` this workspace is associated with.
    pub fn config(&self) -> &'cfg Config {
        self.config
    }

    pub fn profiles(&self) -> &Profiles {
        let root = self.root_manifest.as_ref().unwrap_or(&self.current_manifest);
        match *self.packages.get(root) {
            MaybePackage::Package(ref p) => p.manifest().profiles(),
            MaybePackage::Virtual(ref m) => m.profiles(),
        }
    }

    /// Returns the root path of this workspace.
    ///
    /// That is, this returns the path of the directory containing the
    /// `Cargo.toml` which is the root of this workspace.
    pub fn root(&self) -> &Path {
        match self.root_manifest {
            Some(ref p) => p,
            None => &self.current_manifest
        }.parent().unwrap()
    }

    pub fn target_dir(&self) -> Filesystem {
        self.target_dir.clone().unwrap_or_else(|| {
            Filesystem::new(self.root().join("target"))
        })
    }

    /// Returns the root [replace] section of this workspace.
    ///
    /// This may be from a virtual crate or an actual crate.
    pub fn root_replace(&self) -> &[(PackageIdSpec, Dependency)] {
        let path = match self.root_manifest {
            Some(ref p) => p,
            None => &self.current_manifest,
        };
        match *self.packages.get(path) {
            MaybePackage::Package(ref p) => p.manifest().replace(),
            MaybePackage::Virtual(ref v) => v.replace(),
        }
    }

    /// Returns an iterator over all packages in this workspace
    pub fn members<'a>(&'a self) -> Members<'a, 'cfg> {
        Members {
            ws: self,
            iter: self.members.iter(),
        }
    }

    pub fn is_ephemeral(&self) -> bool {
        self.is_ephemeral
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
    fn find_root(&mut self, manifest_path: &Path)
                 -> CargoResult<Option<PathBuf>> {
        {
            let current = self.packages.load(&manifest_path)?;
            match *current.workspace_config() {
                WorkspaceConfig::Root { .. } => {
                    debug!("find_root - is root {}", manifest_path.display());
                    return Ok(Some(manifest_path.to_path_buf()))
                }
                WorkspaceConfig::Member { root: Some(ref path_to_root) } => {
                    let path = manifest_path.parent().unwrap()
                                            .join(path_to_root)
                                            .join("Cargo.toml");
                    debug!("find_root - pointer {}", path.display());
                    return Ok(Some(paths::normalize_path(&path)))
                }
                WorkspaceConfig::Member { root: None } => {}
            }
        }

        let mut cur = manifest_path.parent().and_then(|p| p.parent());
        while let Some(path) = cur {
            let manifest = path.join("Cargo.toml");
            debug!("find_root - trying {}", manifest.display());
            if manifest.exists() {
                match *self.packages.load(&manifest)?.workspace_config() {
                    WorkspaceConfig::Root { .. } => {
                        debug!("find_root - found");
                        return Ok(Some(manifest))
                    }
                    WorkspaceConfig::Member { .. } => {}
                }
            }
            cur = path.parent();
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
        let root_manifest = match self.root_manifest {
            Some(ref path) => path.clone(),
            None => {
                debug!("find_members - only me as a member");
                self.members.push(self.current_manifest.clone());
                return Ok(())
            }
        };
        let members = {
            let root = self.packages.load(&root_manifest)?;
            match *root.workspace_config() {
                WorkspaceConfig::Root { ref members } => members.clone(),
                _ => bail!("root of a workspace inferred but wasn't a root: {}",
                           root_manifest.display()),
            }
        };

        if let Some(list) = members {
            let root = root_manifest.parent().unwrap();
            for path in list {
                let manifest_path = root.join(path).join("Cargo.toml");
                self.find_path_deps(&manifest_path)?;
            }
        }

        self.find_path_deps(&root_manifest)
    }

    fn find_path_deps(&mut self, manifest_path: &Path) -> CargoResult<()> {
        if self.members.iter().any(|p| p == manifest_path) {
            return Ok(())
        }

        debug!("find_members - {}", manifest_path.display());
        self.members.push(manifest_path.to_path_buf());

        let candidates = {
            let pkg = match *self.packages.load(manifest_path)? {
                MaybePackage::Package(ref p) => p,
                MaybePackage::Virtual(_) => return Ok(()),
            };
            pkg.dependencies()
               .iter()
               .map(|d| d.source_id())
               .filter(|d| d.is_path())
               .filter_map(|d| d.url().to_file_path().ok())
               .map(|p| p.join("Cargo.toml"))
               .collect::<Vec<_>>()
        };
        for candidate in candidates {
            self.find_path_deps(&candidate)?;
        }
        Ok(())
    }

    /// Validates a workspace, ensuring that a number of invariants are upheld:
    ///
    /// 1. A workspace only has one root.
    /// 2. All workspace members agree on this one root as the root.
    /// 3. The current crate is a member of this workspace.
    fn validate(&mut self) -> CargoResult<()> {
        if self.root_manifest.is_none() {
            return Ok(())
        }

        let mut roots = Vec::new();
        {
            let mut names = BTreeMap::new();
            for member in self.members.iter() {
                let package = self.packages.get(member);
                match *package.workspace_config() {
                    WorkspaceConfig::Root { .. } => {
                        roots.push(member.parent().unwrap().to_path_buf());
                    }
                    WorkspaceConfig::Member { .. } => {}
                }
                let name = match *package {
                    MaybePackage::Package(ref p) => p.name(),
                    MaybePackage::Virtual(_) => continue,
                };
                if let Some(prev) = names.insert(name, member) {
                    bail!("two packages named `{}` in this workspace:\n\
                           - {}\n\
                           - {}", name, prev.display(), member.display());
                }
            }
        }

        match roots.len() {
            0 => {
                bail!("`package.workspace` configuration points to a crate \
                       which is not configured with [workspace]: \n\
                       configuration at: {}\n\
                       points to: {}",
                      self.current_manifest.display(),
                      self.root_manifest.as_ref().unwrap().display())
            }
            1 => {}
            _ => {
                bail!("multiple workspace roots found in the same workspace:\n{}",
                      roots.iter()
                           .map(|r| format!("  {}", r.display()))
                           .collect::<Vec<_>>()
                           .join("\n"));
            }
        }

        for member in self.members.clone() {
            let root = self.find_root(&member)?;
            if root == self.root_manifest {
                continue
            }

            match root {
                Some(root) => {
                    bail!("package `{}` is a member of the wrong workspace\n\
                           expected: {}\n\
                           actual:   {}",
                          member.display(),
                          self.root_manifest.as_ref().unwrap().display(),
                          root.display());
                }
                None => {
                    bail!("workspace member `{}` is not hierarchically below \
                           the workspace root `{}`",
                          member.display(),
                          self.root_manifest.as_ref().unwrap().display());
                }
            }
        }

        if !self.members.contains(&self.current_manifest) {
            let root = self.root_manifest.as_ref().unwrap();
            let root_dir = root.parent().unwrap();
            let current_dir = self.current_manifest.parent().unwrap();
            let root_pkg = self.packages.get(root);

            let members_msg = match current_dir.strip_prefix(root_dir) {
                Ok(rel) => {
                    format!("this may be fixable by adding `{}` to the \
                             `workspace.members` array of the manifest \
                             located at: {}",
                             rel.display(),
                             root.display())
                }
                Err(_) => {
                    format!("this may be fixable by adding a member to \
                             the `workspace.members` array of the \
                             manifest located at: {}", root.display())
                }
            };
            let extra = match *root_pkg {
                MaybePackage::Virtual(_) => members_msg,
                MaybePackage::Package(ref p) => {
                    let members = match *p.manifest().workspace_config() {
                        WorkspaceConfig::Root { ref members } => members,
                        WorkspaceConfig::Member { .. } => unreachable!(),
                    };
                    if members.is_none() {
                        format!("this may be fixable by ensuring that this \
                                 crate is depended on by the workspace \
                                 root: {}", root.display())
                    } else {
                        members_msg
                    }
                }
            };
            bail!("current package believes it's in a workspace when it's not:\n\
                   current:   {}\n\
                   workspace: {}\n\n{}",
                  self.current_manifest.display(),
                  root.display(),
                  extra);
        }

        if let Some(ref root_manifest) = self.root_manifest {
            let default_profiles = Profiles {
                release: Profile::default_release(),
                dev: Profile::default_dev(),
                test: Profile::default_test(),
                test_deps: Profile::default_dev(),
                bench: Profile::default_bench(),
                bench_deps: Profile::default_release(),
                doc: Profile::default_doc(),
                custom_build: Profile::default_custom_build(),
                check: Profile::default_check(),
            };

            for pkg in self.members().filter(|p| p.manifest_path() != root_manifest) {
                if pkg.manifest().profiles() != &default_profiles {
                    let message = &format!("profiles for the non root package will be ignored, \
                                            specify profiles at the workspace root:\n\
                                            package:   {}\n\
                                            workspace: {}",
                                           pkg.manifest_path().display(),
                                           root_manifest.display());

                    //TODO: remove `Eq` bound from `Profiles` when the warning is removed.
                    self.config.shell().warn(&message)?;
                }
            }
        }

        Ok(())
    }
}

impl<'cfg> Packages<'cfg> {
    fn get(&self, manifest_path: &Path) -> &MaybePackage {
        &self.packages[manifest_path.parent().unwrap()]
    }

    fn load(&mut self, manifest_path: &Path) -> CargoResult<&MaybePackage> {
        let key = manifest_path.parent().unwrap();
        match self.packages.entry(key.to_path_buf()) {
            Entry::Occupied(e) => Ok(e.into_mut()),
            Entry::Vacant(v) => {
                let source_id = SourceId::for_path(key)?;
                let pair = ops::read_manifest(&manifest_path, &source_id,
                                              self.config)?;
                let (manifest, _nested_paths) = pair;
                Ok(v.insert(match manifest {
                    EitherManifest::Real(manifest) => {
                        MaybePackage::Package(Package::new(manifest,
                                                           manifest_path))
                    }
                    EitherManifest::Virtual(v) => {
                        MaybePackage::Virtual(v)
                    }
                }))
            }
        }
    }
}

impl<'a, 'cfg> Members<'a, 'cfg> {
    pub fn is_empty(self) -> bool {
        self.count() == 0
    }
}

impl<'a, 'cfg> Iterator for Members<'a, 'cfg> {
    type Item = &'a Package;

    fn next(&mut self) -> Option<&'a Package> {
        loop {
            let next = self.iter.next().map(|path| {
                self.ws.packages.get(path)
            });
            match next {
                Some(&MaybePackage::Package(ref p)) => return Some(p),
                Some(&MaybePackage::Virtual(_)) => {}
                None => return None,
            }
        }
    }
}

impl MaybePackage {
    fn workspace_config(&self) -> &WorkspaceConfig {
        match *self {
            MaybePackage::Virtual(ref v) => v.workspace_config(),
            MaybePackage::Package(ref v) => v.manifest().workspace_config(),
        }
    }
}
