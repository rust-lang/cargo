use core::{Summary,NameVer,Package};
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
    fn download(&self, packages: &[NameVer]) -> CargoResult<()>;

    /**
     * The get method returns the Path of each specified package on the
     * local file system. It assumes that `download` was already called,
     * and that the packages are already locally available on the file
     * system.
     */
    fn get(&self, packages: &[NameVer]) -> CargoResult<Vec<Package>>;
}
