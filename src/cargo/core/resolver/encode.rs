use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::str::FromStr;

use log::debug;
use serde::de;
use serde::ser;
use serde::{Deserialize, Serialize};

use crate::core::{Dependency, Package, PackageId, SourceId, Workspace};
use crate::util::errors::{CargoResult, CargoResultExt};
use crate::util::{internal, Graph};

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
    pub fn into_resolve(self, ws: &Workspace<'_>) -> CargoResult<Resolve> {
        let path_deps = build_path_deps(ws);

        let packages = {
            let mut packages = self.package.unwrap_or_default();
            if let Some(root) = self.root {
                packages.insert(0, root);
            }
            packages
        };

        // `PackageId`s in the lock file don't include the `source` part
        // for workspace members, so we reconstruct proper IDs.
        let live_pkgs = {
            let mut live_pkgs = HashMap::new();
            let mut all_pkgs = HashSet::new();
            for pkg in packages.iter() {
                let enc_id = EncodablePackageId {
                    name: pkg.name.clone(),
                    version: pkg.version.clone(),
                    source: pkg.source,
                };

                if !all_pkgs.insert(enc_id.clone()) {
                    failure::bail!("package `{}` is specified twice in the lockfile", pkg.name);
                }
                let id = match pkg.source.as_ref().or_else(|| path_deps.get(&pkg.name)) {
                    // We failed to find a local package in the workspace.
                    // It must have been removed and should be ignored.
                    None => {
                        debug!("path dependency now missing {} v{}", pkg.name, pkg.version);
                        continue;
                    }
                    Some(&source) => PackageId::new(&pkg.name, &pkg.version, source)?,
                };

                assert!(live_pkgs.insert(enc_id, (id, pkg)).is_none())
            }
            live_pkgs
        };

        let lookup_id = |enc_id: &EncodablePackageId| -> Option<PackageId> {
            live_pkgs.get(enc_id).map(|&(id, _)| id)
        };

        let g = {
            let mut g = Graph::new();

            for &(ref id, _) in live_pkgs.values() {
                g.add(id.clone());
            }

            for &(ref id, pkg) in live_pkgs.values() {
                let deps = match pkg.dependencies {
                    Some(ref deps) => deps,
                    None => continue,
                };

                for edge in deps.iter() {
                    if let Some(to_depend_on) = lookup_id(edge) {
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
                    if let Some(replace_id) = lookup_id(replace) {
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
            let enc_id: EncodablePackageId = k
                .parse()
                .chain_err(|| internal("invalid encoding of checksum in lockfile"))?;
            let id = match lookup_id(&enc_id) {
                Some(id) => id,
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
                Some(&src) => PackageId::new(&pkg.name, &pkg.version, src)?,
                None => continue,
            };
            unused_patches.push(id);
        }

        Ok(Resolve::new(
            g,
            replacements,
            HashMap::new(),
            checksums,
            metadata,
            unused_patches,
        ))
    }
}

fn build_path_deps(ws: &Workspace<'_>) -> HashMap<String, SourceId> {
    // If a crate is **not** a path source, then we're probably in a situation
    // such as `cargo install` with a lock file from a remote dependency. In
    // that case we don't need to fixup any path dependencies (as they're not
    // actually path dependencies any more), so we ignore them.
    let members = ws
        .members()
        .filter(|p| p.package_id().source_id().is_path())
        .collect::<Vec<_>>();

    let mut ret = HashMap::new();
    let mut visited = HashSet::new();
    for member in members.iter() {
        ret.insert(
            member.package_id().name().to_string(),
            member.package_id().source_id(),
        );
        visited.insert(member.package_id().source_id());
    }
    for member in members.iter() {
        build_pkg(member, ws, &mut ret, &mut visited);
    }
    for deps in ws.root_patch().values() {
        for dep in deps {
            build_dep(dep, ws, &mut ret, &mut visited);
        }
    }
    for &(_, ref dep) in ws.root_replace() {
        build_dep(dep, ws, &mut ret, &mut visited);
    }

    return ret;

    fn build_pkg(
        pkg: &Package,
        ws: &Workspace<'_>,
        ret: &mut HashMap<String, SourceId>,
        visited: &mut HashSet<SourceId>,
    ) {
        for dep in pkg.dependencies() {
            build_dep(dep, ws, ret, visited);
        }
    }

    fn build_dep(
        dep: &Dependency,
        ws: &Workspace<'_>,
        ret: &mut HashMap<String, SourceId>,
        visited: &mut HashSet<SourceId>,
    ) {
        let id = dep.source_id();
        if visited.contains(&id) || !id.is_path() {
            return;
        }
        let path = match id.url().to_file_path() {
            Ok(p) => p.join("Cargo.toml"),
            Err(_) => return,
        };
        let pkg = match ws.load(&path) {
            Ok(p) => p,
            Err(_) => return,
        };
        ret.insert(pkg.name().to_string(), pkg.package_id().source_id());
        visited.insert(pkg.package_id().source_id());
        build_pkg(&pkg, ws, ret, visited);
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
    source: Option<SourceId>,
}

impl fmt::Display for EncodablePackageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.name, self.version)?;
        if let Some(ref s) = self.source {
            write!(f, " ({})", s.to_url())?;
        }
        Ok(())
    }
}

impl FromStr for EncodablePackageId {
    type Err = failure::Error;

    fn from_str(s: &str) -> CargoResult<EncodablePackageId> {
        let mut s = s.splitn(3, ' ');
        let name = s.next().unwrap();
        let version = s
            .next()
            .ok_or_else(|| internal("invalid serialized PackageId"))?;
        let source_id = match s.next() {
            Some(s) => {
                if s.starts_with('(') && s.ends_with(')') {
                    Some(SourceId::from_url(&s[1..s.len() - 1])?)
                } else {
                    failure::bail!("invalid serialized PackageId")
                }
            }
            None => None,
        };

        Ok(EncodablePackageId {
            name: name.to_string(),
            version: version.to_string(),
            source: source_id,
        })
    }
}

impl ser::Serialize for EncodablePackageId {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        s.collect_str(self)
    }
}

impl<'de> de::Deserialize<'de> for EncodablePackageId {
    fn deserialize<D>(d: D) -> Result<EncodablePackageId, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        String::deserialize(d).and_then(|string| {
            string
                .parse::<EncodablePackageId>()
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
    where
        S: ser::Serializer,
    {
        let mut ids: Vec<_> = self.resolve.iter().collect();
        ids.sort();

        let encodable = ids
            .iter()
            .map(|&id| encodable_resolve_node(id, self.resolve))
            .collect::<Vec<_>>();

        let mut metadata = self.resolve.metadata().clone();

        for &id in ids.iter().filter(|id| !id.source_id().is_path()) {
            let checksum = match self.resolve.checksums()[&id] {
                Some(ref s) => &s[..],
                None => "<none>",
            };
            let id = encodable_package_id(id);
            metadata.insert(format!("checksum {}", id.to_string()), checksum.to_string());
        }

        let metadata = if metadata.is_empty() {
            None
        } else {
            Some(metadata)
        };

        let patch = Patch {
            unused: self
                .resolve
                .unused_patches()
                .iter()
                .map(|id| EncodableDependency {
                    name: id.name().to_string(),
                    version: id.version().to_string(),
                    source: encode_source(id.source_id()),
                    dependencies: None,
                    replace: None,
                })
                .collect(),
        };
        EncodableResolve {
            package: Some(encodable),
            root: None,
            metadata,
            patch,
        }
        .serialize(s)
    }
}

fn encodable_resolve_node(id: PackageId, resolve: &Resolve) -> EncodableDependency {
    let (replace, deps) = match resolve.replacement(id) {
        Some(id) => (Some(encodable_package_id(id)), None),
        None => {
            let mut deps = resolve
                .deps_not_replaced(id)
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
        replace,
    }
}

pub fn encodable_package_id(id: PackageId) -> EncodablePackageId {
    EncodablePackageId {
        name: id.name().to_string(),
        version: id.version().to_string(),
        source: encode_source(id.source_id()).map(|s| s.with_precise(None)),
    }
}

fn encode_source(id: SourceId) -> Option<SourceId> {
    if id.is_path() {
        None
    } else {
        Some(id)
    }
}
