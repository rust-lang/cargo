use std::collections::{HashMap, BTreeMap};

use regex::Regex;
use rustc_serialize::{Encodable, Encoder, Decodable, Decoder};

use core::{Package, PackageId, SourceId};
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
    pub fn to_resolve(&self, root: &Package, config: &Config)
                      -> CargoResult<Resolve> {
        let mut path_deps = HashMap::new();
        try!(build_path_deps(root, &mut path_deps, config));
        let default = root.package_id().source_id();

        let mut g = Graph::new();
        let mut tmp = HashMap::new();

        let packages = Vec::new();
        let packages = self.package.as_ref().unwrap_or(&packages);

        let root = try!(to_package_id(&self.root.name,
                                      &self.root.version,
                                      self.root.source.as_ref(),
                                      default, &path_deps));
        let ids = try!(packages.iter().map(|p| {
            to_package_id(&p.name, &p.version, p.source.as_ref(),
                          default, &path_deps)
        }).collect::<CargoResult<Vec<_>>>());

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
                let deps = match pkg.dependencies {
                    Some(ref deps) => deps,
                    None => return Ok(()),
                };
                for edge in deps.iter() {
                    let to_depend_on = try!(to_package_id(&edge.name,
                                                          &edge.version,
                                                          edge.source.as_ref(),
                                                          default,
                                                          &path_deps));
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
        })
    }
}

fn build_path_deps(root: &Package,
                   map: &mut HashMap<String, SourceId>,
                   config: &Config)
                   -> CargoResult<()> {
    // If the root crate is *not* a path source, then we're probably in a
    // situation such as `cargo install` with a lock file from a remote
    // dependency. In that case we don't need to fixup any path dependencies (as
    // they're not actually path dependencies any more), so we ignore them.
    if !root.package_id().source_id().is_path() {
        return Ok(())
    }

    let deps = root.dependencies()
                   .iter()
                   .map(|d| d.source_id())
                   .filter(|id| id.is_path())
                   .filter_map(|id| id.url().to_file_path().ok())
                   .map(|path| path.join("Cargo.toml"))
                   .filter_map(|path| Package::for_path(&path, config).ok());
    for pkg in deps {
        let source_id = pkg.package_id().source_id();
        if map.insert(pkg.name().to_string(), source_id.clone()).is_none() {
            try!(build_path_deps(&pkg, map, config));
        }
    }

    Ok(())
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
    dependencies: Option<Vec<EncodablePackageId>>
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
        let captures = regex.captures(&string)
                            .expect("invalid serialized PackageId");

        let name = captures.at(1).unwrap();
        let version = captures.at(2).unwrap();

        let source = captures.at(3);

        let source_id = source.map(|s| SourceId::from_url(s.to_string()));

        Ok(EncodablePackageId {
            name: name.to_string(),
            version: version.to_string(),
            source: source_id
        })
    }
}

impl Encodable for Resolve {
    fn encode<S: Encoder>(&self, s: &mut S) -> Result<(), S::Error> {
        let mut ids: Vec<&PackageId> = self.graph.iter().collect();
        ids.sort();

        let encodable = ids.iter().filter_map(|&id| {
            if self.root == *id { return None; }

            Some(encodable_resolve_node(id, &self.graph))
        }).collect::<Vec<EncodableDependency>>();

        EncodableResolve {
            package: Some(encodable),
            root: encodable_resolve_node(&self.root, &self.graph),
            metadata: self.metadata.clone(),
        }.encode(s)
    }
}

fn encodable_resolve_node(id: &PackageId, graph: &Graph<PackageId>)
                          -> EncodableDependency {
    let deps = graph.edges(id).map(|edge| {
        let mut deps = edge.map(encodable_package_id).collect::<Vec<_>>();
        deps.sort();
        deps
    });

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
