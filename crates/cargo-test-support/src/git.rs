/*
# Git Testing Support

## Creating a git dependency
`git::new()` is an easy way to create a new git repository containing a
project that you can then use as a dependency. It will automatically add all
the files you specify in the project and commit them to the repository.
Example:

```
let git_project = git::new("dep1", |project| {
    project
        .file("Cargo.toml", &basic_manifest("dep1"))
        .file("src/lib.rs", r#"pub fn f() { println!("hi!"); } "#)
});

// Use the `url()` method to get the file url to the new repository.
let p = project()
    .file("Cargo.toml", &format!(r#"
        [package]
        name = "a"
        version = "1.0.0"

        [dependencies]
        dep1 = {{ git = '{}' }}
    "#, git_project.url()))
    .file("src/lib.rs", "extern crate dep1;")
    .build();
```

## Manually creating repositories
`git::repo()` can be used to create a `RepoBuilder` which provides a way of
adding files to a blank repository and committing them.

If you want to then manipulate the repository (such as adding new files or
tags), you can use `git2::Repository::open()` to open the repository and then
use some of the helper functions in this file to interact with the repository.

*/

use crate::{path2url, project, Project, ProjectBuilder};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Once;
use url::Url;

#[must_use]
pub struct RepoBuilder {
    repo: git2::Repository,
    files: Vec<PathBuf>,
}

pub struct Repository(git2::Repository);

/// Create a `RepoBuilder` to build a new git repository.
///
/// Call `build()` to finalize and create the repository.
pub fn repo(p: &Path) -> RepoBuilder {
    RepoBuilder::init(p)
}

impl RepoBuilder {
    pub fn init(p: &Path) -> RepoBuilder {
        t!(fs::create_dir_all(p.parent().unwrap()));
        let repo = init(p);
        RepoBuilder {
            repo,
            files: Vec::new(),
        }
    }

    /// Add a file to the repository.
    pub fn file(self, path: &str, contents: &str) -> RepoBuilder {
        let mut me = self.nocommit_file(path, contents);
        me.files.push(PathBuf::from(path));
        me
    }

    /// Add a file that will be left in the working directory, but not added
    /// to the repository.
    pub fn nocommit_file(self, path: &str, contents: &str) -> RepoBuilder {
        let dst = self.repo.workdir().unwrap().join(path);
        t!(fs::create_dir_all(dst.parent().unwrap()));
        t!(fs::write(&dst, contents));
        self
    }

    /// Create the repository and commit the new files.
    pub fn build(self) -> Repository {
        {
            let mut index = t!(self.repo.index());
            for file in self.files.iter() {
                t!(index.add_path(file));
            }
            t!(index.write());
            let id = t!(index.write_tree());
            let tree = t!(self.repo.find_tree(id));
            let sig = t!(self.repo.signature());
            t!(self
                .repo
                .commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[]));
        }
        let RepoBuilder { repo, .. } = self;
        Repository(repo)
    }
}

impl Repository {
    pub fn root(&self) -> &Path {
        self.0.workdir().unwrap()
    }

    pub fn url(&self) -> Url {
        path2url(self.0.workdir().unwrap().to_path_buf())
    }

    pub fn revparse_head(&self) -> String {
        self.0
            .revparse_single("HEAD")
            .expect("revparse HEAD")
            .id()
            .to_string()
    }
}

/// Initialize a new repository at the given path.
pub fn init(path: &Path) -> git2::Repository {
    default_search_path();
    let repo = t!(git2::Repository::init(path));
    default_repo_cfg(&repo);
    repo
}

fn default_search_path() {
    use crate::paths::global_root;
    use git2::{opts::set_search_path, ConfigLevel};

    static INIT: Once = Once::new();
    INIT.call_once(|| unsafe {
        let path = global_root().join("blank_git_search_path");
        t!(set_search_path(ConfigLevel::System, &path));
        t!(set_search_path(ConfigLevel::Global, &path));
        t!(set_search_path(ConfigLevel::XDG, &path));
        t!(set_search_path(ConfigLevel::ProgramData, &path));
    })
}

fn default_repo_cfg(repo: &git2::Repository) {
    let mut cfg = t!(repo.config());
    t!(cfg.set_str("user.email", "foo@bar.com"));
    t!(cfg.set_str("user.name", "Foo Bar"));
}

/// Create a new git repository with a project.
pub fn new<F>(name: &str, callback: F) -> Project
where
    F: FnOnce(ProjectBuilder) -> ProjectBuilder,
{
    new_repo(name, callback).0
}

/// Create a new git repository with a project.
/// Returns both the Project and the git Repository.
pub fn new_repo<F>(name: &str, callback: F) -> (Project, git2::Repository)
where
    F: FnOnce(ProjectBuilder) -> ProjectBuilder,
{
    let mut git_project = project().at(name);
    git_project = callback(git_project);
    let git_project = git_project.build();

    let repo = init(&git_project.root());
    add(&repo);
    commit(&repo);
    (git_project, repo)
}

/// Add all files in the working directory to the git index.
pub fn add(repo: &git2::Repository) {
    // FIXME(libgit2/libgit2#2514): apparently, `add_all` will add all submodules
    // as well, and then fail because they're directories. As a stop-gap, we just
    // ignore all submodules.
    let mut s = t!(repo.submodules());
    for submodule in s.iter_mut() {
        t!(submodule.add_to_index(false));
    }
    let mut index = t!(repo.index());
    t!(index.add_all(
        ["*"].iter(),
        git2::IndexAddOption::DEFAULT,
        Some(
            &mut (|a, _b| if s.iter().any(|s| a.starts_with(s.path())) {
                1
            } else {
                0
            })
        )
    ));
    t!(index.write());
}

/// Add a git submodule to the repository.
pub fn add_submodule<'a>(
    repo: &'a git2::Repository,
    url: &str,
    path: &Path,
) -> git2::Submodule<'a> {
    let path = path.to_str().unwrap().replace(r"\", "/");
    let mut s = t!(repo.submodule(url, Path::new(&path), false));
    let subrepo = t!(s.open());
    default_repo_cfg(&subrepo);
    t!(subrepo.remote_add_fetch("origin", "refs/heads/*:refs/heads/*"));
    let mut origin = t!(subrepo.find_remote("origin"));
    t!(origin.fetch(&Vec::<String>::new(), None, None));
    t!(subrepo.checkout_head(None));
    t!(s.add_finalize());
    s
}

/// Commit changes to the git repository.
pub fn commit(repo: &git2::Repository) -> git2::Oid {
    let tree_id = t!(t!(repo.index()).write_tree());
    let sig = t!(repo.signature());
    let mut parents = Vec::new();
    if let Some(parent) = repo.head().ok().map(|h| h.target().unwrap()) {
        parents.push(t!(repo.find_commit(parent)))
    }
    let parents = parents.iter().collect::<Vec<_>>();
    t!(repo.commit(
        Some("HEAD"),
        &sig,
        &sig,
        "test",
        &t!(repo.find_tree(tree_id)),
        &parents
    ))
}

/// Create a new tag in the git repository.
pub fn tag(repo: &git2::Repository, name: &str) {
    let head = repo.head().unwrap().target().unwrap();
    t!(repo.tag(
        name,
        &t!(repo.find_object(head, None)),
        &t!(repo.signature()),
        "make a new tag",
        false
    ));
}
