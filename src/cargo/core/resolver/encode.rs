use std::collections::{HashMap, BTreeMap};

use regex::Regex;
use rustc_serialize::{Encodable, Encoder, Decodable, Decoder};

use core::{PackageId, SourceId};
use util::{CargoResult, Graph};

use super::Resolve;

#[derive(RustcEncodable, RustcDecodable, Debug)]
pub struct EncodableResolve {
    package: Option<Vec<EncodableDependency>>,
    root: EncodableDependency,
    metadata: Option<Metadata>,
}

pub type Metadata = BTreeMap<String, String>;

impl EncodableResolve {
    pub fn to_resolve(&self, default: &SourceId) -> CargoResult<Resolve> {
        let mut g = Graph::new();
        let mut tmp = HashMap::new();

        let packages = Vec::new();
        let packages = self.package.as_ref().unwrap_or(&packages);

        {
            let mut register_pkg = |&mut: pkg: &EncodableDependency|
                                    -> CargoResult<()> {
                let pkgid = try!(pkg.to_package_id(default));
                let precise = pkgid.source_id().precise()
                                   .map(|s| s.to_string());
                assert!(tmp.insert(pkgid.clone(), precise).is_none(),
                        "a package was referenced twice in the lockfile");
                g.add(try!(pkg.to_package_id(default)), &[]);
                Ok(())
            };

            try!(register_pkg(&self.root));
            for pkg in packages.iter() {
                try!(register_pkg(pkg));
            }
        }

        {
            let mut add_dependencies = |&mut: pkg: &EncodableDependency|
                                        -> CargoResult<()> {
                let package_id = try!(pkg.to_package_id(default));

                let deps = match pkg.dependencies {
                    Some(ref deps) => deps,
                    None => return Ok(()),
                };
                for edge in deps.iter() {
                    let to_depend_on = try!(edge.to_package_id(default));
                    let precise_pkgid =
                        tmp.get(&to_depend_on)
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

#[derive(RustcEncodable, RustcDecodable, Debug, PartialOrd, Ord, PartialEq, Eq)]
pub struct EncodableDependency {
    name: String,
    version: String,
    source: Option<SourceId>,
    dependencies: Option<Vec<EncodablePackageId>>
}

impl EncodableDependency {
    fn to_package_id(&self, default_source: &SourceId) -> CargoResult<PackageId> {
        PackageId::new(
            &self.name,
            &self.version,
            self.source.as_ref().unwrap_or(default_source))
    }
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

impl EncodablePackageId {
    fn to_package_id(&self, default_source: &SourceId) -> CargoResult<PackageId> {
        PackageId::new(
            &self.name,
            &self.version,
            self.source.as_ref().unwrap_or(default_source))
    }
}

impl Encodable for Resolve {
    fn encode<S: Encoder>(&self, s: &mut S) -> Result<(), S::Error> {
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

    let source = if id.source_id() == root.source_id() {
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

fn encodable_package_id(id: &PackageId, root: &PackageId) -> EncodablePackageId {
    let source = if id.source_id() == root.source_id() {
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
