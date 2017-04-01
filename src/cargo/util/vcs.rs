use std::path::Path;

use git2;

use util::{CargoResult, process};

pub struct HgRepo;
pub struct GitRepo;
pub struct PijulRepo;

impl GitRepo {
    pub fn init(path: &Path, _: &Path) -> CargoResult<GitRepo> {
        git2::Repository::init(path)?;
        Ok(GitRepo)
    }
    pub fn discover(path: &Path, _: &Path) -> Result<git2::Repository,git2::Error> {
        git2::Repository::discover(path)
    }
}

impl HgRepo {
    pub fn init(path: &Path, cwd: &Path) -> CargoResult<HgRepo> {
        process("hg").cwd(cwd).arg("init").arg(path).exec()?;
        Ok(HgRepo)
    }
    pub fn discover(path: &Path, cwd: &Path) -> CargoResult<HgRepo> {
        process("hg").cwd(cwd).arg("root").cwd(path).exec_with_output()?;
        Ok(HgRepo)
    }
}

impl PijulRepo {
    pub fn init(path: &Path, cwd: &Path) -> CargoResult<PijulRepo> {
        process("pijul").cwd(cwd).arg("init").arg(path).exec()?;
        Ok(PijulRepo)
    }
}
