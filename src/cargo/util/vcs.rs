use std::path::Path;
use std::fs::create_dir;

use git2;

use util::{process, CargoResult};

pub struct HgRepo;
pub struct GitRepo;
pub struct PijulRepo;
pub struct FossilRepo;

impl GitRepo {
    pub fn init(path: &Path, _: &Path) -> CargoResult<GitRepo> {
        git2::Repository::init(path)?;
        Ok(GitRepo)
    }
    pub fn discover(path: &Path, _: &Path) -> Result<git2::Repository, git2::Error> {
        git2::Repository::discover(path)
    }
}

impl HgRepo {
    pub fn init(path: &Path, cwd: &Path) -> CargoResult<HgRepo> {
        process("hg").cwd(cwd).arg("init").arg(path).exec()?;
        Ok(HgRepo)
    }
    pub fn discover(path: &Path, cwd: &Path) -> CargoResult<HgRepo> {
        process("hg")
            .cwd(cwd)
            .arg("root")
            .cwd(path)
            .exec_with_output()?;
        Ok(HgRepo)
    }
}

impl PijulRepo {
    pub fn init(path: &Path, cwd: &Path) -> CargoResult<PijulRepo> {
        process("pijul").cwd(cwd).arg("init").arg(path).exec()?;
        Ok(PijulRepo)
    }
}

impl FossilRepo {
    pub fn init(path: &Path, cwd: &Path) -> CargoResult<FossilRepo> {
        // fossil doesn't create the directory so we'll do that first
        create_dir(path)?;

        // set up the paths we'll use
        let db_fname = ".fossil";
        let mut db_path = path.to_owned();
        db_path.push(db_fname);

        // then create the fossil DB in that location
        process("fossil").cwd(cwd).arg("init").arg(&db_path).exec()?;

        // open it in that new directory
        process("fossil")
            .cwd(&path)
            .arg("open")
            .arg(db_fname)
            .exec()?;

        // set `target` as ignoreable and cleanable
        process("fossil")
            .cwd(cwd)
            .arg("settings")
            .arg("ignore-glob")
            .arg("target");
        process("fossil")
            .cwd(cwd)
            .arg("settings")
            .arg("clean-glob")
            .arg("target");
        Ok(FossilRepo)
    }
}
