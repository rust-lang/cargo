use url::Url;
use core::{Summary,Package,PackageId};
use util::CargoResult;

/**
 * A Source finds and downloads remote packages based on names and
 * versions.
 */
pub trait Source {
    /**
     * The update method performs any network operations required to
     * get the entire list of all names, versions and dependencies of
     * packages managed by the Source.
     */
    fn update(&self) -> CargoResult<()>;

    /**
     * The list method lists all names, versions and dependencies of
     * packages managed by the source. It assumes that `update` has
     * already been called and no additional network operations are
     * required.
     */
    fn list(&self) -> CargoResult<Vec<Summary>>;

    /**
     * The download method fetches the full package for each name and
     * version specified.
     */
    fn download(&self, packages: &[PackageId]) -> CargoResult<()>;

    /**
     * The get method returns the Path of each specified package on the
     * local file system. It assumes that `download` was already called,
     * and that the packages are already locally available on the file
     * system.
     */
    fn get(&self, packages: &[PackageId]) -> CargoResult<Vec<Package>>;
}

#[deriving(Clone,PartialEq)]
pub enum SourceKind {
    GitKind(String)
}

#[deriving(Clone,PartialEq)]
pub struct SourceId {
    pub kind: SourceKind,
    pub url: Url
}

impl SourceId {
    pub fn new(kind: SourceKind, url: Url) -> SourceId {
        SourceId { kind: kind, url: url }
    }
}

pub struct SourceSet {
    sources: Vec<Box<Source>>
}

impl SourceSet {
    pub fn new(sources: Vec<Box<Source>>) -> SourceSet {
        SourceSet { sources: sources }
    }
}

impl Source for SourceSet {
    fn update(&self) -> CargoResult<()> {
        for source in self.sources.iter() {
            try!(source.update());
        }

        Ok(())
    }

    fn list(&self) -> CargoResult<Vec<Summary>> {
        let mut ret = Vec::new();

        for source in self.sources.iter() {
            ret.push_all(try!(source.list()).as_slice());
        }

        Ok(ret)
    }

    fn download(&self, packages: &[PackageId]) -> CargoResult<()> {
        for source in self.sources.iter() {
            try!(source.download(packages));
        }

        Ok(())
    }

    fn get(&self, packages: &[PackageId]) -> CargoResult<Vec<Package>> {
        let mut ret = Vec::new();

        for source in self.sources.iter() {
            ret.push_all(try!(source.get(packages)).as_slice());
        }

        Ok(ret)
    }
}
