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

        try!(add_pkg_to_graph(&mut g, &self.root, default));

        match self.package {
            Some(ref packages) => {
                for dep in packages.iter() {
                    try!(add_pkg_to_graph(&mut g, dep, default));
                }
            }
            None => {}
        }

        let root = self.root.to_package_id(default);
        Ok(Resolve {
            graph: g,
            root: try!(root),
            features: HashMap::new(),
            metadata: self.metadata.clone(),
        })
    }
}

fn add_pkg_to_graph(g: &mut Graph<PackageId>,
                    dep: &EncodableDependency,
                    default: &SourceId)
                    -> CargoResult<()>
{
    let package_id = try!(dep.to_package_id(default));
    g.add(package_id.clone(), []);

    match dep.dependencies {
        Some(ref deps) => {
            for edge in deps.iter() {
                g.link(package_id.clone(), try!(edge.to_package_id(default)));
            }
        },
        _ => ()
    };

    Ok(())
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
        Some(id.get_source_id().clone())
    };
    EncodablePackageId {
        name: id.get_name().to_string(),
        version: id.get_version().to_string(),
        source: source,
    }
}
