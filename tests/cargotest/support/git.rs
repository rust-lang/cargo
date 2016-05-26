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
        t!(fs::create_dir_all(p.parent().unwrap()));
        let repo = t!(git2::Repository::init(p));
        {
            let mut config = t!(repo.config());
            t!(config.set_str("user.name", "name"));
            t!(config.set_str("user.email", "email"));
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
        t!(fs::create_dir_all(dst.parent().unwrap()));
        t!(t!(File::create(&dst)).write_all(contents.as_bytes()));
        self
    }

    pub fn build(&self) {
        let mut index = t!(self.repo.index());
        for file in self.files.iter() {
            t!(index.add_path(file));
        }
        t!(index.write());
        let id = t!(index.write_tree());
        let tree = t!(self.repo.find_tree(id));
        let sig = t!(self.repo.signature());
        t!(self.repo.commit(Some("HEAD"), &sig, &sig,
                            "Initial commit", &tree, &[]));
    }

    pub fn root(&self) -> &Path {
        self.repo.workdir().unwrap()
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

    let repo = t!(git2::Repository::init(&git_project.root()));
    let mut cfg = t!(repo.config());
    t!(cfg.set_str("user.email", "foo@bar.com"));
    t!(cfg.set_str("user.name", "Foo Bar"));
    drop(cfg);
    add(&repo);
    commit(&repo);
    Ok(git_project)
}

pub fn add(repo: &git2::Repository) {
    // FIXME(libgit2/libgit2#2514): apparently add_all will add all submodules
    // as well, and then fail b/c they're a directory. As a stopgap, we just
    // ignore all submodules.
    let mut s = t!(repo.submodules());
    for submodule in s.iter_mut() {
        t!(submodule.add_to_index(false));
    }
    let mut index = t!(repo.index());
    t!(index.add_all(["*"].iter(), git2::ADD_DEFAULT,
                  Some(&mut (|a, _b| {
        if s.iter().any(|s| a.starts_with(s.path())) {1} else {0}
    }))));
    t!(index.write());
}

pub fn add_submodule<'a>(repo: &'a git2::Repository, url: &str,
                         path: &Path) -> git2::Submodule<'a>
{
    let path = path.to_str().unwrap().replace(r"\", "/");
    let mut s = t!(repo.submodule(url, Path::new(&path), false));
    let subrepo = t!(s.open());
    t!(subrepo.remote_add_fetch("origin", "refs/heads/*:refs/heads/*"));
    let mut origin = t!(subrepo.find_remote("origin"));
    t!(origin.fetch(&[], None, None));
    t!(subrepo.checkout_head(None));
    t!(s.add_finalize());
    return s;
}

pub fn commit(repo: &git2::Repository) -> git2::Oid {
    let tree_id = t!(t!(repo.index()).write_tree());
    let sig = t!(repo.signature());
    let mut parents = Vec::new();
    match repo.head().ok().map(|h| h.target().unwrap()) {
        Some(parent) => parents.push(t!(repo.find_commit(parent))),
        None => {}
    }
    let parents = parents.iter().collect::<Vec<_>>();
    t!(repo.commit(Some("HEAD"), &sig, &sig, "test",
                   &t!(repo.find_tree(tree_id)),
                   &parents))
}

pub fn tag(repo: &git2::Repository, name: &str) {
    let head = repo.head().unwrap().target().unwrap();
    t!(repo.tag(name,
                &t!(repo.find_object(head, None)),
                &t!(repo.signature()),
                "make a new tag",
                false));
}
