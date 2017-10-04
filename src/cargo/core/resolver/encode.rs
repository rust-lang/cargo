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
    /// `root` is optional to allow backward compatibility.
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
        let path_deps = build_path_deps(ws);

        let packages = {
            let mut packages = self.package.unwrap_or_default();
            if let Some(root) = self.root {
                packages.insert(0, root);
            }
            packages
        };

        // `PackageId`s in the lock file don't include the `source` part
        // for workspace members, so we reconstruct proper ids.
        let (live_pkgs, all_pkgs) = {
            let mut live_pkgs = HashMap::new();
            let mut all_pkgs = HashSet::new();
            for pkg in packages.iter() {
                let enc_id = EncodablePackageId {
                    name: pkg.name.clone(),
                    version: pkg.version.clone(),
                    source: pkg.source.clone(),
                };

                if !all_pkgs.insert(enc_id.clone()) {
                    return Err(internal(format!("package `{}` is specified twice in the lockfile",
                                                pkg.name)));
                }
                let id = match pkg.source.as_ref().or_else(|| path_deps.get(&pkg.name)) {
                    // We failed to find a local package in the workspace.
                    // It must have been removed and should be ignored.
                    None => continue,
                    Some(source) => PackageId::new(&pkg.name, &pkg.version, source)?
                };

                assert!(live_pkgs.insert(enc_id, (id, pkg)).is_none())
            }
            (live_pkgs, all_pkgs)
        };

        let lookup_id = |enc_id: &EncodablePackageId| -> CargoResult<Option<PackageId>> {
            match live_pkgs.get(enc_id) {
                Some(&(ref id, _)) => Ok(Some(id.clone())),
                None => if all_pkgs.contains(enc_id) {
                    // Package is found in the lockfile, but it is
                    // no longer a member of the workspace.
                    Ok(None)
                } else {
                    Err(internal(format!("package `{}` is specified as a dependency, \
                                          but is missing from the package list", enc_id)))
                }
            }
        };

        let g = {
            let mut g = Graph::new();

            for &(ref id, _) in live_pkgs.values() {
                g.add(id.clone(), &[]);
            }

            for &(ref id, pkg) in live_pkgs.values() {
                let deps = match pkg.dependencies {
                    Some(ref deps) => deps,
                    None => continue
                };

                for edge in deps.iter() {
                    if let Some(to_depend_on) = lookup_id(edge)? {
                        g.link(id.clone(), to_depend_on);
                    }
                }
            }
            g
        };

        let replacements = {
            let mut replacements = HashMap::new();
            for &(ref id, pkg) in live_pkgs.values() {
                if let Some(ref replace) = pkg.replace {
                    assert!(pkg.dependencies.is_none());
                    if let Some(replace_id) = lookup_id(replace)? {
                        replacements.insert(id.clone(), replace_id);
                    }
                }
            }
            replacements
        };

        let mut metadata = self.metadata.unwrap_or_default();

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
            let id = match lookup_id(&enc_id) {
                Ok(Some(id)) => id,
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

        let mut unused_patches = Vec::new();
        for pkg in self.patch.unused {
            let id = match pkg.source.as_ref().or_else(|| path_deps.get(&pkg.name)) {
                Some(src) => PackageId::new(&pkg.name, &pkg.version, src)?,
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

fn build_path_deps(ws: &Workspace) -> HashMap<String, SourceId> {
    // If a crate is *not* a path source, then we're probably in a situation
    // such as `cargo install` with a lock file from a remote dependency. In
    // that case we don't need to fixup any path dependencies (as they're not
    // actually path dependencies any more), so we ignore them.
    let members = ws.members().filter(|p| {
        p.package_id().source_id().is_path()
    }).collect::<Vec<_>>();

    let mut ret = HashMap::new();
    let mut visited = HashSet::new();
    for member in members.iter() {
        ret.insert(member.package_id().name().to_string(),
                   member.package_id().source_id().clone());
        visited.insert(member.package_id().source_id().clone());
    }
    for member in members.iter() {
        build(member, ws.config(), &mut ret, &mut visited);
    }

    return ret;

    fn build(pkg: &Package,
             config: &Config,
             ret: &mut HashMap<String, SourceId>,
             visited: &mut HashSet<SourceId>) {
        let replace = pkg.manifest().replace().iter().map(|p| &p.1);
        let patch = pkg.manifest().patch().values().flat_map(|v| v);
        let deps = pkg.dependencies()
                      .iter()
                      .chain(replace)
                      .chain(patch)
                      .map(|d| d.source_id())
                      .filter(|id| !visited.contains(id) && id.is_path())
                      .filter_map(|id| id.url().to_file_path().ok())
                      .map(|path| path.join("Cargo.toml"))
                      .filter_map(|path| Package::for_path(&path, config).ok())
                      .collect::<Vec<_>>();
        for pkg in deps {
            ret.insert(pkg.name().to_string(),
                       pkg.package_id().source_id().clone());
            visited.insert(pkg.package_id().source_id().clone());
            build(&pkg, config, ret, visited);
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
                if s.starts_with('(') && s.ends_with(')') {
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
}

impl<'a, 'cfg> ser::Serialize for WorkspaceResolve<'a, 'cfg> {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
        where S: ser::Serializer,
    {
        let mut ids: Vec<&PackageId> = self.resolve.graph.iter().collect();
        ids.sort();

        let encodable = ids.iter().filter_map(|&id| {
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

        let metadata = if metadata.is_empty() { None } else { Some(metadata) };

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
            root: None,
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
