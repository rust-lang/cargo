use std::collections::{HashMap, BTreeMap};

use regex::Regex;
use rustc_serialize::{Encodable, Encoder, Decodable, Decoder};

use core::{Package, PackageId, SourceId, Workspace};
use util::{CargoResult, Graph, Config};

use super::Resolve;

#[derive(RustcEncodable, RustcDecodable, Debug)]
pub struct EncodableResolve {
    package: Option<Vec<EncodableDependency>>,
    root: EncodableDependency,
    metadata: Option<Metadata>,
}

pub type Metadata = BTreeMap<String, String>;

impl EncodableResolve {
    pub fn to_resolve(&self, ws: &Workspace) -> CargoResult<Resolve> {
        let path_deps = build_path_deps(ws);
        let default = try!(ws.current()).package_id().source_id();

        let mut g = Graph::new();
        let mut tmp = HashMap::new();
        let mut replacements = HashMap::new();

        let packages = Vec::new();
        let packages = self.package.as_ref().unwrap_or(&packages);

        let id2pkgid = |id: &EncodablePackageId| {
            to_package_id(&id.name, &id.version, id.source.as_ref(),
                          default, &path_deps)
        };
        let dep2pkgid = |dep: &EncodableDependency| {
            to_package_id(&dep.name, &dep.version, dep.source.as_ref(),
                          default, &path_deps)
        };

        let root = try!(dep2pkgid(&self.root));
        let ids = try!(packages.iter().map(&dep2pkgid)
                               .collect::<CargoResult<Vec<_>>>());

        {
            let mut register_pkg = |pkgid: &PackageId| {
                let precise = pkgid.source_id().precise()
                                   .map(|s| s.to_string());
                assert!(tmp.insert(pkgid.clone(), precise).is_none(),
                        "a package was referenced twice in the lockfile");
                g.add(pkgid.clone(), &[]);
            };

            register_pkg(&root);
            for id in ids.iter() {
                register_pkg(id);
            }
        }

        {
            let mut add_dependencies = |id: &PackageId, pkg: &EncodableDependency|
                                        -> CargoResult<()> {
                if let Some(ref replace) = pkg.replace {
                    let replace = try!(id2pkgid(replace));
                    let replace_precise = tmp.get(&replace).map(|p| {
                        replace.with_precise(p.clone())
                    }).unwrap_or(replace);
                    replacements.insert(id.clone(), replace_precise);
                    assert!(pkg.dependencies.is_none());
                    return Ok(())
                }

                let deps = match pkg.dependencies {
                    Some(ref deps) => deps,
                    None => return Ok(()),
                };
                for edge in deps.iter() {
                    let to_depend_on = try!(id2pkgid(edge));
                    let precise_pkgid =
                        tmp.get(&to_depend_on)
                           .map(|p| to_depend_on.with_precise(p.clone()))
                           .unwrap_or(to_depend_on.clone());
                    g.link(id.clone(), precise_pkgid);
                }
                Ok(())
            };

            try!(add_dependencies(&root, &self.root));
            for (id, pkg) in ids.iter().zip(packages) {
                try!(add_dependencies(id, pkg));
            }
        }

        Ok(Resolve {
            graph: g,
            root: root,
            features: HashMap::new(),
            metadata: self.metadata.clone(),
            replacements: replacements,
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
    for member in members.iter() {
        ret.insert(member.package_id().name().to_string(),
                   member.package_id().source_id().clone());
    }
    for member in members.iter() {
        build(member, ws.config(), &mut ret);
    }

    return ret;

    fn build(pkg: &Package,
             config: &Config,
             ret: &mut HashMap<String, SourceId>) {
        let deps = pkg.dependencies()
                      .iter()
                      .filter(|d| !ret.contains_key(d.name()))
                      .map(|d| d.source_id())
                      .filter(|id| id.is_path())
                      .filter_map(|id| id.url().to_file_path().ok())
                      .map(|path| path.join("Cargo.toml"))
                      .filter_map(|path| Package::for_path(&path, config).ok())
                      .collect::<Vec<_>>();
        for pkg in deps {
            ret.insert(pkg.name().to_string(),
                       pkg.package_id().source_id().clone());
            build(&pkg, config, ret);
        }
    }
}

fn to_package_id(name: &str,
                 version: &str,
                 source: Option<&SourceId>,
                 default_source: &SourceId,
                 path_sources: &HashMap<String, SourceId>)
                 -> CargoResult<PackageId> {
    let source = source.or(path_sources.get(name)).unwrap_or(default_source);
    PackageId::new(name, version, source)
}


#[derive(RustcEncodable, RustcDecodable, Debug, PartialOrd, Ord, PartialEq, Eq)]
pub struct EncodableDependency {
    name: String,
    version: String,
    source: Option<SourceId>,
    dependencies: Option<Vec<EncodablePackageId>>,
    replace: Option<EncodablePackageId>,
}

#[derive(Debug, PartialOrd, Ord, PartialEq, Eq)]
pub struct EncodablePackageId {
    name: String,
    version: String,
    source: Option<SourceId>
}

impl Encodable for EncodablePackageId {
    fn encode<S: Encoder>(&self, s: &mut S) -> Result<(), S::Error> {
        let mut out = format!("{} {}", self.name, self.version);
        if let Some(ref s) = self.source {
            out.push_str(&format!(" ({})", s.to_url()));
        }
        out.encode(s)
    }
}

impl Decodable for EncodablePackageId {
    fn decode<D: Decoder>(d: &mut D) -> Result<EncodablePackageId, D::Error> {
        let string: String = try!(Decodable::decode(d));
        let regex = Regex::new(r"^([^ ]+) ([^ ]+)(?: \(([^\)]+)\))?$").unwrap();
        let captures = try!(regex.captures(&string).ok_or_else(|| {
            d.error("invalid serialized PackageId")
        }));

        let name = captures.at(1).unwrap();
        let version = captures.at(2).unwrap();

        let source_id = match captures.at(3) {
            Some(s) => {
                Some(try!(SourceId::from_url(s).map_err(|e| {
                    d.error(&e.to_string())
                })))
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

pub struct WorkspaceResolve<'a, 'cfg: 'a> {
    pub ws: &'a Workspace<'cfg>,
    pub resolve: &'a Resolve,
}

impl<'a, 'cfg> Encodable for WorkspaceResolve<'a, 'cfg> {
    fn encode<S: Encoder>(&self, s: &mut S) -> Result<(), S::Error> {
        let mut ids: Vec<&PackageId> = self.resolve.graph.iter().collect();
        ids.sort();

        let root = self.ws.members().max_by_key(|member| {
            member.name()
        }).unwrap().package_id();

        let encodable = ids.iter().filter_map(|&id| {
            if root == id {
                return None
            }

            Some(encodable_resolve_node(id, self.resolve))
        }).collect::<Vec<EncodableDependency>>();

        EncodableResolve {
            package: Some(encodable),
            root: encodable_resolve_node(&root, self.resolve),
            metadata: self.resolve.metadata.clone(),
        }.encode(s)
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

    let source = if id.source_id().is_path() {
        None
    } else {
        Some(id.source_id().clone())
    };

    EncodableDependency {
        name: id.name().to_string(),
        version: id.version().to_string(),
        source: source,
        dependencies: deps,
        replace: replace,
    }
}

fn encodable_package_id(id: &PackageId) -> EncodablePackageId {
    let source = if id.source_id().is_path() {
        None
    } else {
        Some(id.source_id().with_precise(None))
    };
    EncodablePackageId {
        name: id.name().to_string(),
        version: id.version().to_string(),
        source: source,
    }
}
