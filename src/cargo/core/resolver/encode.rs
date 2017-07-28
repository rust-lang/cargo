use std::collections::{HashMap, HashSet, BTreeMap};
use std::fmt;
use std::str::FromStr;

use serde::ser;
use serde::de;

use core::{Package, PackageId, SourceId, Workspace};
use util::{Graph, Config, internal};
use util::errors::{CargoResult, CargoResultExt, CargoError};

use super::Resolve;

#[derive(Serialize, Deserialize, Debug)]
pub struct EncodableResolve {
    package: Option<Vec<EncodableDependency>>,
    /// `root` is optional to allow forward compatibility.
    root: Option<EncodableDependency>,
    metadata: Option<Metadata>,

    #[serde(default, skip_serializing_if = "Patch::is_empty")]
    patch: Patch,
}

#[derive(Serialize, Deserialize, Debug, Default)]
struct Patch {
    unused: Vec<EncodableDependency>,
}

pub type Metadata = BTreeMap<String, String>;

impl EncodableResolve {
    pub fn into_resolve(self, ws: &Workspace) -> CargoResult<Resolve> {
        // Get all dependencies that have a path
        // The lookup_id function requires a mapping from SourceId to a vector of PackageIds.
        // These PackageIds are the dependencies of the package on the given SourceId.
        // The live_pkgs require a mapping from String (package name) to a vector of SourceIds.
        // These SourceIds are the paths to packages with the given name.
        let (path_deps_lookup_id, path_deps_live_pkgs) = build_path_deps(ws);

        let packages = {
            let mut packages = self.package.unwrap_or(Vec::new());
            if let Some(root) = self.root {
                packages.insert(0, root);
            }
            packages
        };

        // `PackageId`s in the lock file don't include the `source` part
        // for workspace members, so we reconstruct proper ids.
        let live_pkgs = {
            let mut live_pkgs = HashMap::new();
            let mut path_deps = path_deps_live_pkgs.iter()
                                   .map(|v| (v.0, v.1.iter())).collect::<HashMap<_, _>>();

            for pkg in packages.iter() {
                let id = match pkg.source.as_ref()
                    .or_else(|| path_deps.get_mut(&pkg.name).and_then(|mut v| v.next())) {
                    // We failed to find a local package in the workspace.
                    // It must have been removed and should be ignored.
                    None => continue,
                    Some(source) => PackageId::new(&pkg.name, &pkg.version, &source)?
                };

                let enc_id = EncodablePackageId {
                    name: pkg.name.clone(),
                    version: pkg.version.clone(),
                    source: Some(id.source_id().clone()),
                };

                if !live_pkgs.insert(enc_id, (id, pkg)).is_none() {
                    return Err(internal(format!("package `{}` is specified twice in the lockfile",
                                                pkg.name)));
                }
            }
            live_pkgs
        };

        let lookup_id = |enc_id: &EncodablePackageId, ppkg: &PackageId| -> CargoResult<Option<PackageId>> {
            if let Some(&(ref id, _)) = live_pkgs.get(enc_id) {
                return Ok(Some(id.clone()));
            }

            // If we could not find the package in the live package list, we look for the path
            // dependencies of the parent package.
            if let Some(ref deps) = path_deps_lookup_id.get(ppkg.source_id()) {
                // A package can not have two dependencies with the same name,
                // so just search for the package name
                if let Some(id) = deps.iter().filter(|d| d.name() == enc_id.name).last() {
                    return Ok(Some(PackageId::new(&enc_id.name, &enc_id.version, id.source_id())?));
                }
            }

            // Check if the package existed in the old package list.
            // If we found it in the old package list, the package was removed
            if let Some(_) = packages.iter().filter(|p| p.name == enc_id.name
                                                    && p.version == enc_id.version
                                                    && p.source == enc_id.source).last() {
                return Ok(None);
            }

            Err(internal(format!("package `{}` is specified as a dependency, \
                                  but is missing from the package list", enc_id)))
        };

        let g = {
            let mut g = Graph::new();

            for &(ref id, _) in live_pkgs.values() {
                g.add(id.clone(), &[]);
            }

            for &(ref id, ref pkg) in live_pkgs.values() {
                let deps = match pkg.dependencies {
                    Some(ref deps) => deps,
                    None => continue
                };

                for edge in deps.iter() {
                    if let Some(to_depend_on) = lookup_id(edge, id)? {
                        g.link(id.clone(), to_depend_on);
                    }
                }
            }
            g
        };

        let lookup_id_replace = |replace: &EncodablePackageId, to_replace: &PackageId| -> CargoResult<Option<PackageId>> {
            if let Ok(p) = lookup_id(replace, to_replace) {
                return Ok(p);
            }

            // Search for the to_replace package in the workspace replace list, if we find a replace
            // that fulfills our requested version and has the same name, we use the replace.
            if let Some(r) = ws.root_replace().iter()
                                .filter(|r| r.1.version_req().matches(to_replace.version())
                                            && to_replace.name() == r.1.name()).last() {
                return Ok(Some(PackageId::new(&replace.name, &replace.version, r.1.source_id())?));
            }

            Err(internal(format!("package `{}` is specified as replacement,\
                                  but is missing from the replacement list", replace)))
        };

        let replacements = {
            let mut replacements = HashMap::new();
            for &(ref id, ref pkg) in live_pkgs.values() {
                if let Some(ref replace) = pkg.replace {
                    assert!(pkg.dependencies.is_none());
                    if let Some(replace_id) = lookup_id_replace(replace, id)? {
                        replacements.insert(id.clone(), replace_id);
                    }
                }
            }
            replacements
        };

        let mut metadata = self.metadata.unwrap_or(BTreeMap::new());

        // Parse out all package checksums. After we do this we can be in a few
        // situations:
        //
        // * We parsed no checksums. In this situation we're dealing with an old
        //   lock file and we're gonna fill them all in.
        // * We parsed some checksums, but not one for all packages listed. It
        //   could have been the case that some were listed, then an older Cargo
        //   client added more dependencies, and now we're going to fill in the
        //   missing ones.
        // * There are too many checksums listed, indicative of an older Cargo
        //   client removing a package but not updating the checksums listed.
        //
        // In all of these situations they're part of normal usage, so we don't
        // really worry about it. We just try to slurp up as many checksums as
        // possible.
        let mut checksums = HashMap::new();
        let prefix = "checksum ";
        let mut to_remove = Vec::new();
        for (k, v) in metadata.iter().filter(|p| p.0.starts_with(prefix)) {
            to_remove.push(k.to_string());
            let k = &k[prefix.len()..];
            let enc_id: EncodablePackageId = k.parse().chain_err(|| {
                internal("invalid encoding of checksum in lockfile")
            })?;
            let id = match live_pkgs.get(&enc_id) {
                Some(&(ref id, _)) => id.clone(),
                _ => continue,
            };

            let v = if v == "<none>" {
                None
            } else {
                Some(v.to_string())
            };
            checksums.insert(id, v);
        }

        for k in to_remove {
            metadata.remove(&k);
        }
        
        let lookup_id_patch = |patch: &EncodableDependency| -> CargoResult<Option<SourceId>> {
            if let Some(ref src) = patch.source {
                return Ok(Some(src.clone()));
            }

            if let Some(ids) = path_deps_live_pkgs.get(&patch.name) {
                if ids.len() > 1 {
                    return Err(internal(format!("package `{:?}` has ambiguous\
                                                 local replacements: {:?}", patch, ids)));
                }

                Ok(ids.iter().last().map(|v| v.clone()))
            } else {
               Ok(None)
            }
        };

        let mut unused_patches = Vec::new();
        for pkg in self.patch.unused {
            let id = match lookup_id_patch(&pkg)? {
                Some(src) => PackageId::new(&pkg.name, &pkg.version, &src)?,
                None => continue,
            };
            unused_patches.push(id);
        }

        Ok(Resolve {
            graph: g,
            empty_features: HashSet::new(),
            features: HashMap::new(),
            replacements: replacements,
            checksums: checksums,
            metadata: metadata,
            unused_patches: unused_patches,
        })
    }
}

fn build_path_deps(ws: &Workspace) -> (HashMap<SourceId, Vec<PackageId>>,
                                       HashMap<String, HashSet<SourceId>>) {
    // If a crate is *not* a path source, then we're probably in a situation
    // such as `cargo install` with a lock file from a remote dependency. In
    // that case we don't need to fixup any path dependencies (as they're not
    // actually path dependencies any more), so we ignore them.
    let mut ret_pkgs = HashMap::new();
    let mut ret_ids = HashMap::new();
    let mut visited = HashMap::new();
    for member in ws.members().filter(|p| { p.package_id().source_id().is_path() }) {
        build(member, ws.config(), ws, &mut ret_pkgs, &mut ret_ids, &mut visited);
    }

    return (ret_pkgs, ret_ids);

    fn build(ppkg: &Package,
             config: &Config,
             ws: &Workspace,
             ret_pkgs: &mut HashMap<SourceId, Vec<PackageId>>,
             ret_ids: &mut HashMap<String, HashSet<SourceId>>,
             visited: &mut HashMap<PackageId, HashSet<SourceId>>) {
        ret_ids.entry(ppkg.package_id().name().to_owned()).or_insert_with(|| HashSet::new())
            .insert(ppkg.package_id().source_id().clone());

        let replace = ppkg.manifest().replace().iter().map(|p| &p.1);
        let patch = ppkg.manifest().patch().values().flat_map(|v| v);
        let deps = ppkg.dependencies()
                      .iter()
                      .chain(replace)
                      .chain(patch)
                      .map(|d| d.source_id())
                      .filter(|id| id.is_path())
                      .filter(|id| !visited.get(ppkg.package_id())
                                      .map(|c| c.contains(id)).unwrap_or(false))
                      .filter_map(|id| id.url().to_file_path().ok())
                      .map(|path| path.join("Cargo.toml"))
                      .filter_map(|path| Package::for_path(&path, config).ok())
                      .collect::<Vec<_>>();
        for pkg in deps {
            ret_pkgs.entry(ppkg.package_id().source_id().clone())
                .or_insert_with(|| Vec::new()).push(pkg.package_id().clone());
            visited.entry(ppkg.package_id().clone())
                .or_insert_with(|| HashSet::new()).insert(pkg.package_id().source_id().clone());
            build(&pkg, config, ws, ret_pkgs, ret_ids, visited);
        }
    }
}

impl Patch {
    fn is_empty(&self) -> bool {
        self.unused.is_empty()
    }
}

#[derive(Serialize, Deserialize, Debug, PartialOrd, Ord, PartialEq, Eq)]
pub struct EncodableDependency {
    name: String,
    version: String,
    source: Option<SourceId>,
    dependencies: Option<Vec<EncodablePackageId>>,
    replace: Option<EncodablePackageId>,
}

#[derive(Debug, PartialOrd, Ord, PartialEq, Eq, Hash, Clone)]
pub struct EncodablePackageId {
    name: String,
    version: String,
    source: Option<SourceId>
}

impl fmt::Display for EncodablePackageId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {}", self.name, self.version)?;
        if let Some(ref s) = self.source {
            write!(f, " ({})", s.to_url())?;
        }
        Ok(())
    }
}

impl FromStr for EncodablePackageId {
    type Err = CargoError;

    fn from_str(s: &str) -> CargoResult<EncodablePackageId> {
        let mut s = s.splitn(3, ' ');
        let name = s.next().unwrap();
        let version = s.next().ok_or_else(|| {
            internal("invalid serialized PackageId")
        })?;
        let source_id = match s.next() {
            Some(s) => {
                if s.starts_with("(") && s.ends_with(")") {
                    Some(SourceId::from_url(&s[1..s.len() - 1])?)
                } else {
                    bail!("invalid serialized PackageId")
                }
            }
            None => None,
        };

        Ok(EncodablePackageId {
            name: name.to_string(),
            version: version.to_string(),
            source: source_id
        })
    }
}

impl ser::Serialize for EncodablePackageId {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
        where S: ser::Serializer,
    {
        s.collect_str(self)
    }
}

impl<'de> de::Deserialize<'de> for EncodablePackageId {
    fn deserialize<D>(d: D) -> Result<EncodablePackageId, D::Error>
        where D: de::Deserializer<'de>,
    {
        String::deserialize(d).and_then(|string| {
            string.parse::<EncodablePackageId>()
                  .map_err(de::Error::custom)
        })
    }
}

pub struct WorkspaceResolve<'a, 'cfg: 'a> {
    pub ws: &'a Workspace<'cfg>,
    pub resolve: &'a Resolve,
    pub use_root_key: bool,
}

impl<'a, 'cfg> ser::Serialize for WorkspaceResolve<'a, 'cfg> {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
        where S: ser::Serializer,
    {
        let mut ids: Vec<&PackageId> = self.resolve.graph.iter().collect();
        ids.sort();

        let root = self.ws.members().max_by_key(|member| {
            member.name()
        }).map(Package::package_id);

        let encodable = ids.iter().filter_map(|&id| {
            if self.use_root_key && root.unwrap() == id {
                return None
            }

            Some(encodable_resolve_node(id, self.resolve))
        }).collect::<Vec<_>>();

        let mut metadata = self.resolve.metadata.clone();

        for id in ids.iter().filter(|id| !id.source_id().is_path()) {
            let checksum = match self.resolve.checksums[*id] {
                Some(ref s) => &s[..],
                None => "<none>",
            };
            let id = encodable_package_id(id);
            metadata.insert(format!("checksum {}", id.to_string()),
                            checksum.to_string());
        }

        let metadata = if metadata.len() == 0 {None} else {Some(metadata)};

        let root = match root {
            Some(root) if self.use_root_key => Some(encodable_resolve_node(&root, self.resolve)),
            _ => None,
        };

        let patch = Patch {
            unused: self.resolve.unused_patches().iter().map(|id| {
                EncodableDependency {
                    name: id.name().to_string(),
                    version: id.version().to_string(),
                    source: encode_source(id.source_id()),
                    dependencies: None,
                    replace: None,
                }
            }).collect(),
        };
        EncodableResolve {
            package: Some(encodable),
            root: root,
            metadata: metadata,
            patch: patch,
        }.serialize(s)
    }
}

fn encodable_resolve_node(id: &PackageId, resolve: &Resolve)
                          -> EncodableDependency {
    let (replace, deps) = match resolve.replacement(id) {
        Some(id) => {
            (Some(encodable_package_id(id)), None)
        }
        None => {
            let mut deps = resolve.graph.edges(id)
                                  .into_iter().flat_map(|a| a)
                                  .map(encodable_package_id)
                                  .collect::<Vec<_>>();
            deps.sort();
            (None, Some(deps))
        }
    };

    EncodableDependency {
        name: id.name().to_string(),
        version: id.version().to_string(),
        source: encode_source(id.source_id()),
        dependencies: deps,
        replace: replace,
    }
}

fn encodable_package_id(id: &PackageId) -> EncodablePackageId {
    EncodablePackageId {
        name: id.name().to_string(),
        version: id.version().to_string(),
        source: encode_source(id.source_id()).map(|s| s.with_precise(None)),
    }
}

fn encode_source(id: &SourceId) -> Option<SourceId> {
    if id.is_path() {
        None
    } else {
        Some(id.clone())
    }
}
