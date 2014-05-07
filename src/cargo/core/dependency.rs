use core::NameVer;

#[deriving(Eq,Clone,Show,Encodable,Decodable)]
pub struct Dependency {
    name: NameVer
}

impl Dependency {
    pub fn new(name: &str) -> Dependency {
        Dependency { name: NameVer::new(name.to_owned(), "1.0.0") }
    }

    pub fn with_namever(name: &NameVer) -> Dependency {
        Dependency { name: name.clone() }
    }

    pub fn with_name_and_version(name: &str, version: &str) -> Dependency {
        Dependency { name: NameVer::new(name, version) }
    }

    pub fn get_namever<'a>(&'a self) -> &'a NameVer {
        &self.name
    }

    pub fn get_name<'a>(&'a self) -> &'a str {
        self.name.get_name()
    }
}

pub trait DependencyNameVers {
    fn namevers(&self) -> Vec<NameVer>;
}

impl DependencyNameVers for Vec<Dependency> {
    fn namevers(&self) -> Vec<NameVer> {
        self.iter().map(|dep| dep.get_namever().clone()).collect()
    }
}
