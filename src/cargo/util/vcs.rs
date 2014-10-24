use git2;

use util::{CargoResult, process};

pub struct HgRepo;
pub struct GitRepo;

impl GitRepo {
    pub fn init(path: &Path) -> CargoResult<GitRepo> {
        try!(git2::Repository::init(path));
        return Ok(GitRepo)
    }
    pub fn discover(path: &Path) -> Result<git2::Repository,git2::Error> {
        git2::Repository::discover(path)
    }
}

impl HgRepo {
    pub fn init(path: &Path) -> CargoResult<HgRepo> {
        let path_str = path.as_str().unwrap();
        try!(process("hg").arg("init").arg(path_str).exec());
        return Ok(HgRepo)
    }
}
