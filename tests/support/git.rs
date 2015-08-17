use std::fs::{self, File};
use std::io::prelude::*;
use std::path::{Path, PathBuf};

use url::Url;
use git2;

use cargo::util::ProcessError;
use support::{ProjectBuilder, project, path2url};

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
        me.files.push(PathBuf::from(path));
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

pub fn new<F>(name: &str, callback: F) -> Result<ProjectBuilder, ProcessError>
    where F: FnOnce(ProjectBuilder) -> ProjectBuilder
{
    let mut git_project = project(name);
    git_project = callback(git_project);
    git_project.build();

    let repo = git2::Repository::init(&git_project.root()).unwrap();
    let mut cfg = repo.config().unwrap();
    cfg.set_str("user.email", "foo@bar.com").unwrap();
    cfg.set_str("user.name", "Foo Bar").unwrap();
    drop(cfg);
    add(&repo);
    commit(&repo);
    Ok(git_project)
}

pub fn add(repo: &git2::Repository) {
    // FIXME(libgit2/libgit2#2514): apparently add_all will add all submodules
    // as well, and then fail b/c they're a directory. As a stopgap, we just
    // ignore all submodules.
    let mut s = repo.submodules().unwrap();
    for submodule in s.iter_mut() {
        submodule.add_to_index(false).unwrap();
    }
    let mut index = repo.index().unwrap();
    index.add_all(["*"].iter(), git2::ADD_DEFAULT,
                  Some(&mut (|a, _b| {
        if s.iter().any(|s| a.starts_with(s.path())) {1} else {0}
    }))).unwrap();
    index.write().unwrap();
}

pub fn add_submodule<'a>(repo: &'a git2::Repository, url: &str,
                         path: &Path) -> git2::Submodule<'a>
{
    let path = path.to_str().unwrap().replace(r"\", "/");
    let mut s = repo.submodule(url, Path::new(&path), false).unwrap();
    let subrepo = s.open().unwrap();
    let mut origin = subrepo.find_remote("origin").unwrap();
    origin.add_fetch("refs/heads/*:refs/heads/*").unwrap();
    origin.fetch(&[], None).unwrap();
    origin.save().unwrap();
    subrepo.checkout_head(None).unwrap();
    s.add_finalize().unwrap();
    return s;
}

pub fn commit(repo: &git2::Repository) -> git2::Oid {
    let tree_id = repo.index().unwrap().write_tree().unwrap();
    let sig = repo.signature().unwrap();
    let mut parents = Vec::new();
    match repo.head().ok().map(|h| h.target().unwrap()) {
        Some(parent) => parents.push(repo.find_commit(parent).unwrap()),
        None => {}
    }
    let parents = parents.iter().collect::<Vec<_>>();
    repo.commit(Some("HEAD"), &sig, &sig, "test",
                &repo.find_tree(tree_id).unwrap(),
                &parents).unwrap()
}
