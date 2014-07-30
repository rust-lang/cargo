use semver::Version;

use core::{Source, SourceId, PackageId, Package, Summary, Registry};
use core::Dependency;
use util::CargoResult;

pub struct DummyRegistrySource {
    id: SourceId,
}

impl DummyRegistrySource {
    pub fn new(id: &SourceId) -> DummyRegistrySource {
        DummyRegistrySource { id: id.clone() }
    }
}

impl Registry for DummyRegistrySource {
    // This is a hack to get tests to pass, this is just a dummy registry.
    fn query(&mut self, dep: &Dependency) -> CargoResult<Vec<Summary>> {
        let mut version = Version {
            major: 0, minor: 0, patch: 0,
            pre: Vec::new(), build: Vec::new(),
        };
        for i in range(0, 10) {
            version.minor = i;
            if dep.get_version_req().matches(&version) { break }
        }
        let pkgid = PackageId::new(dep.get_name().as_slice(),
                                   version,
                                   &self.id).unwrap();
        Ok(vec![Summary::new(&pkgid, [])])
    }
}

impl Source for DummyRegistrySource {
    fn update(&mut self) -> CargoResult<()> { Ok(()) }
    fn download(&self, _packages: &[PackageId]) -> CargoResult<()> { Ok(()) }
    fn get(&self, _packages: &[PackageId]) -> CargoResult<Vec<Package>> {
        Ok(Vec::new())
    }
    fn fingerprint(&self, _pkg: &Package) -> CargoResult<String> {
        unimplemented!()
    }
}
