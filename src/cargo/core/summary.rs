use core::{
    Dependency,
    NameVer
};

#[deriving(Show,Clone,Eq,Encodable)]
pub struct Summary {
    name_ver: NameVer,
    dependencies: Vec<Dependency>
}

impl Summary {
    pub fn new(name_ver: &NameVer, dependencies: &[Dependency]) -> Summary {
        Summary {
            name_ver: name_ver.clone(),
            dependencies: Vec::from_slice(dependencies)
        }
    }

    pub fn get_name_ver<'a>(&'a self) -> &'a NameVer {
        &self.name_ver
    }

    pub fn get_name<'a>(&'a self) -> &'a str {
        self.get_name_ver().get_name()
    }

    pub fn get_dependencies<'a>(&'a self) -> &'a [Dependency] {
        self.dependencies.as_slice()
    }
}

pub trait SummaryVec {
    fn names(&self) -> Vec<~str>;
    fn deps(&self) -> Vec<Dependency>;
}

impl SummaryVec for Vec<Summary> {
    fn names(&self) -> Vec<~str> {
        self.iter().map(|summary| summary.name_ver.get_name().to_owned()).collect()
    }

    fn deps(&self) -> Vec<Dependency> {
        self.iter().map(|summary| Dependency::with_namever(summary.get_name_ver())).collect()
    }
}
