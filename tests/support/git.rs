use std::fs::{self, File};
use std::io::prelude::*;
use std::path::{Path, PathBuf};

use url::Url;
use git2;

use support::path2url;

pub struct RepoBuilder {
    repo: git2::Repository,
    files: Vec<PathBuf>,
}

pub fn repo(p: &Path) -> RepoBuilder { RepoBuilder::init(p) }

impl RepoBuilder {
    pub fn init(p: &Path) -> RepoBuilder {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        let repo = git2::Repository::init(p).unwrap();
        {
            let mut config = repo.config().unwrap();
            config.set_str("user.name", "name").unwrap();
            config.set_str("user.email", "email").unwrap();
        }
        RepoBuilder { repo: repo, files: Vec::new() }
    }

    pub fn file(self, path: &str, contents: &str) -> RepoBuilder {
        let mut me = self.nocommit_file(path, contents);
        me.files.push(PathBuf::new(path));
        me
    }

    pub fn nocommit_file(self, path: &str, contents: &str) -> RepoBuilder {
        let dst = self.repo.workdir().unwrap().join(path);
        fs::create_dir_all(dst.parent().unwrap()).unwrap();
        File::create(&dst).unwrap().write_all(contents.as_bytes()).unwrap();
        self
    }

    pub fn build(&self) {
        let mut index = self.repo.index().unwrap();
        for file in self.files.iter() {
            index.add_path(file).unwrap();
        }
        index.write().unwrap();
        let id = index.write_tree().unwrap();
        let tree = self.repo.find_tree(id).unwrap();
        let sig = self.repo.signature().unwrap();
        self.repo.commit(Some("HEAD"), &sig, &sig,
                         "Initial commit", &tree, &[]).unwrap();
    }

    pub fn url(&self) -> Url {
        path2url(self.repo.workdir().unwrap().to_path_buf())
    }
}
