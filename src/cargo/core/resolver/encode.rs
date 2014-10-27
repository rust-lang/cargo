use std::collections::{HashMap, TreeMap};

use regex::Regex;
use serialize::{Encodable, Encoder, Decodable, Decoder};

use core::{PackageId, SourceId};
use util::{CargoResult, Graph};

use super::Resolve;

#[deriving(Encodable, Decodable, Show)]
pub struct EncodableResolve {
    package: Option<Vec<EncodableDependency>>,
    root: EncodableDependency,
    metadata: Option<Metadata>,
}

pub type Metadata = TreeMap<String, String>;

impl EncodableResolve {
    pub fn to_resolve(&self, default: &SourceId) -> CargoResult<Resolve> {
        let mut g = Graph::new();
        let mut tmp = HashMap::new();

        let packages = Vec::new();
        let packages = self.package.as_ref().unwrap_or(&packages);

        {
            let register_pkg = |pkg: &EncodableDependency| {
                let pkgid = try!(pkg.to_package_id(default));
                let precise = pkgid.get_source_id().get_precise()
                                   .map(|s| s.to_string());
                assert!(tmp.insert(pkgid.clone(), precise),
                        "a package was referenced twice in the lockfile");
                g.add(try!(pkg.to_package_id(default)), []);
                Ok(())
            };

            try!(register_pkg(&self.root));
            for pkg in packages.iter() {
                try!(register_pkg(pkg));
            }
        }

        {
            let add_dependencies = |pkg: &EncodableDependency| {
                let package_id = try!(pkg.to_package_id(default));

                let deps = match pkg.dependencies {
                    Some(ref deps) => deps,
                    None => return Ok(()),
                };
                for edge in deps.iter() {
                    let to_depend_on = try!(edge.to_package_id(default));
                    let precise_pkgid =
                        tmp.find(&to_depend_on)
                           .map(|p| to_depend_on.with_precise(p.clone()))
                           .unwrap_or(to_depend_on.clone());
                    g.link(package_id.clone(), precise_pkgid);
                }
                Ok(())
            };

            try!(add_dependencies(&self.root));
            for pkg in packages.iter() {
                try!(add_dependencies(pkg));
            }
        }

        Ok(Resolve {
            graph: g,
            root: try!(self.root.to_package_id(default)),
            features: HashMap::new(),
            metadata: self.metadata.clone(),
        })
    }
}

#[deriving(Encodable, Decodable, Show, PartialOrd, Ord, PartialEq, Eq)]
pub struct EncodableDependency {
    name: String,
    version: String,
    source: Option<SourceId>,
    dependencies: Option<Vec<EncodablePackageId>>
}

impl EncodableDependency {
    fn to_package_id(&self, default_source: &SourceId) -> CargoResult<PackageId> {
        PackageId::new(
            self.name.as_slice(),
            self.version.as_slice(),
            self.source.as_ref().unwrap_or(default_source))
    }
}

#[deriving(Show, PartialOrd, Ord, PartialEq, Eq)]
pub struct EncodablePackageId {
    name: String,
    version: String,
    source: Option<SourceId>
}

impl<E, S: Encoder<E>> Encodable<S, E> for EncodablePackageId {
    fn encode(&self, s: &mut S) -> Result<(), E> {
        let mut out = format!("{} {}", self.name, self.version);
        if let Some(ref s) = self.source {
            out.push_str(format!(" ({})", s.to_url()).as_slice());
        }
        out.encode(s)
    }
}

impl<E, D: Decoder<E>> Decodable<D, E> for EncodablePackageId {
    fn decode(d: &mut D) -> Result<EncodablePackageId, E> {
        let string: String = raw_try!(Decodable::decode(d));
        let regex = Regex::new(r"^([^ ]+) ([^ ]+)(?: \(([^\)]+)\))?$").unwrap();
        let captures = regex.captures(string.as_slice())
                            .expect("invalid serialized PackageId");

        let name = captures.at(1);
        let version = captures.at(2);

        let source = captures.at(3);

        let source_id = if source == "" {
            None
        } else {
            Some(SourceId::from_url(source.to_string()))
        };

        Ok(EncodablePackageId {
            name: name.to_string(),
            version: version.to_string(),
            source: source_id
        })
    }
}

impl EncodablePackageId {
    fn to_package_id(&self, default_source: &SourceId) -> CargoResult<PackageId> {
        PackageId::new(
            self.name.as_slice(),
            self.version.as_slice(),
            self.source.as_ref().unwrap_or(default_source))
    }
}

impl<E, S: Encoder<E>> Encodable<S, E> for Resolve {
    fn encode(&self, s: &mut S) -> Result<(), E> {
        let mut ids: Vec<&PackageId> = self.graph.iter().collect();
        ids.sort();

        let encodable = ids.iter().filter_map(|&id| {
            if self.root == *id { return None; }

            Some(encodable_resolve_node(id, &self.root, &self.graph))
        }).collect::<Vec<EncodableDependency>>();

        EncodableResolve {
            package: Some(encodable),
            root: encodable_resolve_node(&self.root, &self.root, &self.graph),
            metadata: self.metadata.clone(),
        }.encode(s)
    }
}

fn encodable_resolve_node(id: &PackageId, root: &PackageId,
                          graph: &Graph<PackageId>) -> EncodableDependency {
    let deps = graph.edges(id).map(|edge| {
        let mut deps = edge.map(|e| {
            encodable_package_id(e, root)
        }).collect::<Vec<EncodablePackageId>>();
        deps.sort();
        deps
    });

    let source = if id.get_source_id() == root.get_source_id() {
        None
    } else {
        Some(id.get_source_id().clone())
    };

    EncodableDependency {
        name: id.get_name().to_string(),
        version: id.get_version().to_string(),
        source: source,
        dependencies: deps,
    }
}

fn encodable_package_id(id: &PackageId, root: &PackageId) -> EncodablePackageId {
    let source = if id.get_source_id() == root.get_source_id() {
        None
    } else {
        Some(id.get_source_id().with_precise(None))
    };
    EncodablePackageId {
        name: id.get_name().to_string(),
        version: id.get_version().to_string(),
        source: source,
    }
}
