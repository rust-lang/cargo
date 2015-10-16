use std::path::Path;

use git2;

use util::{CargoResult, process};

pub struct HgRepo;
pub struct GitRepo;

impl GitRepo {
    pub fn init(path: &Path, _: &Path) -> CargoResult<GitRepo> {
        try!(git2::Repository::init(path));
        return Ok(GitRepo)
    }
    pub fn discover(path: &Path, _: &Path) -> Result<git2::Repository,git2::Error> {
        git2::Repository::discover(path)
    }
}

impl HgRepo {
    pub fn init(path: &Path, cwd: &Path) -> CargoResult<HgRepo> {
        try!(try!(process("hg", cwd)).arg("init").arg(path).exec());
        return Ok(HgRepo)
    }
    pub fn discover(path: &Path, cwd: &Path) -> CargoResult<HgRepo> {
        try!(try!(process("hg", cwd)).arg("root").cwd(path).exec_with_output());
        return Ok(HgRepo)
    }
}

