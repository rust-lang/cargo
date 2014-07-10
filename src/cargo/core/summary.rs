use semver::Version;
use core::{
    Dependency,
    PackageId,
    SourceId
};

/// Summaries are cloned, and should not be mutated after creation
#[deriving(Show,Clone,PartialEq)]
pub struct Summary {
    package_id: PackageId,
    dependencies: Vec<Dependency>
}

impl Summary {
    pub fn new(pkg_id: &PackageId, dependencies: &[Dependency]) -> Summary {
        Summary {
            package_id: pkg_id.clone(),
            dependencies: Vec::from_slice(dependencies),
        }
    }

    pub fn get_package_id<'a>(&'a self) -> &'a PackageId {
        &self.package_id
    }

    pub fn get_name<'a>(&'a self) -> &'a str {
        self.get_package_id().get_name()
    }

    pub fn get_version<'a>(&'a self) -> &'a Version {
        self.get_package_id().get_version()
    }

    pub fn get_source_id<'a>(&'a self) -> &'a SourceId {
        self.package_id.get_source_id()
    }

    pub fn get_dependencies<'a>(&'a self) -> &'a [Dependency] {
        self.dependencies.as_slice()
    }
}

pub trait SummaryVec {
    fn names(&self) -> Vec<String>;
}

impl SummaryVec for Vec<Summary> {
    // TODO: Move to Registry
    fn names(&self) -> Vec<String> {
        self.iter().map(|summary| summary.get_name().to_string()).collect()
    }

}
