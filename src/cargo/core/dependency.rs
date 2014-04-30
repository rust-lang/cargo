use core;

#[deriving(Eq,Clone,Show,Encodable,Decodable)]
pub struct Dependency {
    name: core::NameVer
}

impl Dependency {
    pub fn new(name: &str) -> Dependency {
        Dependency { name: core::NameVer::new(name.to_owned(), "1.0.0") }
    }

    pub fn with_namever(name: &core::NameVer) -> Dependency {
        Dependency { name: name.clone() }
    }

    pub fn with_name_and_version(name: &str, version: &str) -> Dependency {
        Dependency { name: core::NameVer::new(name, version) }
    }

    pub fn get_name<'a>(&'a self) -> &'a str {
        self.name.get_name()
    }
}
