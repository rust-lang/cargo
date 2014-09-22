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
            dependencies: dependencies.to_vec(),
        }
    }

    pub fn get_package_id(&self) -> &PackageId {
        &self.package_id
    }

    pub fn get_name(&self) -> &str {
        self.get_package_id().get_name()
    }

    pub fn get_version(&self) -> &Version {
        self.get_package_id().get_version()
    }

    pub fn get_source_id(&self) -> &SourceId {
        self.package_id.get_source_id()
    }

    pub fn get_dependencies(&self) -> &[Dependency] {
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
