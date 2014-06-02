use semver::Version;
use core::{
    Dependency,
    NameVer
};

#[deriving(Show,Clone,PartialEq)]
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

    pub fn get_version<'a>(&'a self) -> &'a Version {
        self.get_name_ver().get_version()
    }

    pub fn get_dependencies<'a>(&'a self) -> &'a [Dependency] {
        self.dependencies.as_slice()
    }
}

pub trait SummaryVec {
    fn names(&self) -> Vec<String>;
    fn deps(&self) -> Vec<Dependency>;
}

impl SummaryVec for Vec<Summary> {
    // TODO: Move to Registry
    fn names(&self) -> Vec<String> {
        self.iter().map(|summary| summary.name_ver.get_name().to_str()).collect()
    }

    // TODO: Delete
    fn deps(&self) -> Vec<Dependency> {
        self.iter().map(|summary| Dependency::exact(summary.get_name(), summary.get_version())).collect()
    }
}
