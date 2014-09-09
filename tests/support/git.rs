use std::io::{mod, fs, File};

use git2;

pub struct RepoBuilder {
    repo: git2::Repository,
    files: Vec<Path>,
}

pub fn repo(p: &Path) -> RepoBuilder { RepoBuilder::init(p) }

impl RepoBuilder {
    pub fn init(p: &Path) -> RepoBuilder {
        fs::mkdir_recursive(&p.dir_path(), io::UserDir).unwrap();
        let repo = git2::Repository::init(p).unwrap();
        {
            let mut config = repo.config().unwrap();
            config.set_str("user.name", "name").unwrap();
            config.set_str("user.email", "email").unwrap();
        }
        RepoBuilder { repo: repo, files: Vec::new() }
    }

    pub fn file<T: Str>(self, path: &str, contents: T) -> RepoBuilder {
        let mut me = self.nocommit_file(path, contents);
        me.files.push(Path::new(path));
        me
    }

    pub fn nocommit_file<T: Str>(self, path: &str,
                                 contents: T) -> RepoBuilder {
        let dst = self.repo.path().dir_path().join(path);
        fs::mkdir_recursive(&dst.dir_path(), io::UserDir).unwrap();
        File::create(&dst).write_str(contents.as_slice()).unwrap();
        self
    }

    pub fn build(&self) {
        let mut index = self.repo.index().unwrap();
        for file in self.files.iter() {
            index.add_path(file).unwrap();
        }
        index.write().unwrap();
        let id = index.write_tree().unwrap();
        let tree = git2::Tree::lookup(&self.repo, id).unwrap();
        let sig = git2::Signature::default(&self.repo).unwrap();
        git2::Commit::new(&self.repo, Some("HEAD"), &sig, &sig,
                          "Initial commit", &tree, []).unwrap();
    }
}
