use crate::util::CargoResult;
use cargo_util::paths;
use cargo_util::ProcessBuilder;
use std::path::Path;

// Check if we are in an existing repo. We define that to be true if either:
//
// 1. We are in a git repo and the path to the new package is not an ignored
//    path in that repo.
// 2. We are in an HG repo.
pub fn existing_vcs_repo(path: &Path, cwd: &Path) -> bool {
    fn in_git_repo(path: &Path, cwd: &Path) -> bool {
        if let Ok(repo) = GitRepo::discover(path, cwd) {
            // Don't check if the working directory itself is ignored.
            if repo.workdir().map_or(false, |workdir| workdir == path) {
                true
            } else {
                !repo.is_path_ignored(path).unwrap_or(false)
            }
        } else {
            false
        }
    }

    in_git_repo(path, cwd)
        || HgRepo::discover(path, cwd).is_ok()
        || RcsRepo::discover(path, cwd).is_ok()
}

pub struct HgRepo;
pub struct GitRepo;
pub struct PijulRepo;
pub struct FossilRepo;
pub struct RcsRepo;

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
        ProcessBuilder::new("hg")
            .cwd(cwd)
            .arg("init")
            .arg("--")
            .arg(path)
            .exec()?;
        Ok(HgRepo)
    }
    pub fn discover(path: &Path, cwd: &Path) -> CargoResult<HgRepo> {
        ProcessBuilder::new("hg")
            .cwd(cwd)
            .arg("--cwd")
            .arg(path)
            .arg("root")
            .exec_with_output()?;
        Ok(HgRepo)
    }
}

impl PijulRepo {
    pub fn init(path: &Path, cwd: &Path) -> CargoResult<PijulRepo> {
        ProcessBuilder::new("pijul")
            .cwd(cwd)
            .arg("init")
            .arg("--")
            .arg(path)
            .exec()?;
        Ok(PijulRepo)
    }
}

impl FossilRepo {
    pub fn init(path: &Path, cwd: &Path) -> CargoResult<FossilRepo> {
        // fossil doesn't create the directory so we'll do that first
        paths::create_dir_all(path)?;

        // set up the paths we'll use
        let db_fname = ".fossil";
        let mut db_path = path.to_owned();
        db_path.push(db_fname);

        // then create the fossil DB in that location
        ProcessBuilder::new("fossil")
            .cwd(cwd)
            .arg("init")
            .arg("--")
            .arg(&db_path)
            .exec()?;

        // open it in that new directory
        ProcessBuilder::new("fossil")
            .cwd(&path)
            .arg("open")
            .arg("--")
            .arg(db_fname)
            .exec()?;

        Ok(FossilRepo)
    }
}

impl RcsRepo {
    pub fn init(path: &Path, _cwd: &Path) -> CargoResult<RcsRepo> {
        paths::create_dir_all(path.join("RCS"))?;
        Ok(RcsRepo)
    }

    pub fn late_init(path: &Path) -> CargoResult<()> {
        for entry in walkdir::WalkDir::new(path)
            .into_iter()
            .filter_entry(|e| e.file_name() != "RCS")
            .filter_map(|e| e.ok())
        {
            let p = entry.path();
            if p.is_file() {
                if let Some(parent) = p.parent() {
                    ProcessBuilder::new("ci")
                        .cwd(parent)
                        .arg("-i")
                        .arg("-l")
                        .arg("-q")
                        .arg("-t-''")
                        .arg(entry.file_name())
                        .exec()?;
                }
            } else if p.is_dir() {
                paths::create_dir_all(p.join("RCS"))?;
            }
        }

        Ok(())
    }

    pub fn discover(path: &Path, _cwd: &Path) -> CargoResult<RcsRepo> {
        ProcessBuilder::new("rlog")
            .cwd(&path)
            .arg("Cargo.toml")
            .exec_with_output()?;
        Ok(RcsRepo {})
    }
}
