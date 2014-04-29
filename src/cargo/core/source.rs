use std::fmt;
use core::package::NameVer;
use CargoResult;

#[deriving(Clone,Eq)]
pub struct PackagePath {
    name: NameVer,
    path: Path
}

impl fmt::Show for PackagePath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f.buf, "{} at {}", self.name, self.path.display())
    }
}

impl PackagePath {
    pub fn new(name: NameVer, path: Path) -> PackagePath {
        PackagePath { name: name, path: path }
    }
}

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
    fn list(&self) -> CargoResult<Vec<NameVer>>;

    /**
     * The download method fetches the full package for each name and
     * version specified.
     */
    fn download(&self, packages: Vec<NameVer>) -> CargoResult<()>;

    /**
     * The get method returns the Path of each specified package on the
     * local file system. It assumes that `download` was already called,
     * and that the packages are already locally available on the file
     * system.
     */
    fn get(&self, packages: Vec<NameVer>) -> CargoResult<Vec<PackagePath>>;
}
