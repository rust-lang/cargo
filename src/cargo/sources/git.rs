#![allow(dead_code)]

use url::Url;
use util::{CargoResult,ProcessBuilder,io_error,human_error,process};
use std::fmt::Show;
use std::str;
use std::io::{UserDir,AllPermissions};
use std::io::fs::{mkdir_recursive,rmdir_recursive,chmod};
use serialize::{Encodable,Encoder};

macro_rules! git(
    ($config:expr, $str:expr, $($rest:expr),*) => (
        try!(git_inherit(&$config, format!($str, $($rest),*)))
    );

    ($config:expr, $str:expr) => (
        try!(git_inherit(&$config, format!($str)))
    );
)

macro_rules! git_output(
    ($config:expr, $str:expr, $($rest:expr),*) => (
        try!(git_output(&$config, format!($str, $($rest),*)))
    );

    ($config:expr, $str:expr) => (
        try!(git_output(&$config, format!($str)))
    );
)

#[deriving(Eq,Clone)]
struct GitConfig {
    path: Path,
    uri: Url,
    reference: String
}

#[deriving(Eq,Clone,Encodable)]
struct EncodableGitConfig {
    path: String,
    uri: String,
    reference: String
}

impl<E, S: Encoder<E>> Encodable<S, E> for GitConfig {
    fn encode(&self, s: &mut S) -> Result<(), E> {
        EncodableGitConfig {
            path: self.path.display().to_str(),
            uri: self.uri.to_str(),
            reference: self.reference.clone()
        }.encode(s)
    }
}

#[deriving(Eq,Clone)]
pub struct GitCommand {
    config: GitConfig
}

#[deriving(Eq,Clone,Encodable)]
pub struct GitRepo {
    config: GitConfig,
    revision: String
}

impl GitCommand {
    pub fn new(path: Path, uri: Url, reference: String) -> GitCommand {
        GitCommand { config: GitConfig { path: path, uri: uri, reference: reference } }
    }

    pub fn get_cwd<'a>(&'a self) -> &'a Path {
        &self.config.path
    }

    pub fn checkout(&self) -> CargoResult<GitRepo> {
        if self.config.path.exists() {
            // TODO: If the revision we have is a rev, avoid unnecessarily fetching if we have the rev already
            try!(self.fetch());
        } else {
            try!(self.clone());
        }

        Ok(GitRepo { config: self.config.clone(), revision: try!(rev_for(&self.config)) })
    }

    fn fetch(&self) -> CargoResult<()> {
        Ok(git!(self.config.path, "fetch --force --quiet --tags {} refs/heads/*:refs/heads/*", self.config.uri))
    }

    fn clone(&self) -> CargoResult<()> {
        let dirname = Path::new(self.config.path.dirname());

        try!(mkdir_recursive(&self.config.path, UserDir).map_err(|err|
            human_error(format!("Couldn't recursively create `{}`", dirname.display()), format!("path={}", dirname.display()), io_error(err))));

        Ok(git!(dirname, "clone {} {} --bare --no-hardlinks --quiet", self.config.uri, self.config.path.display()))
    }
}

struct GitCheckout<'a> {
    location: Path,
    repo: &'a GitRepo
}

impl<'a> GitCheckout<'a> {
    fn clone<'a>(into: Path, repo: &'a GitRepo) -> CargoResult<GitCheckout<'a>> {
        let checkout = GitCheckout { location: into, repo: repo };

        // If the git checkout already exists, we don't need to clone it again
        if !checkout.location.join(".git").exists() {
            try!(checkout.clone_repo());
        }

        Ok(checkout)
    }

    fn get_source<'a>(&'a self) -> &'a Path {
        self.repo.get_path()
    }

    fn clone_repo(&self) -> CargoResult<()> {
        try!(mkdir_recursive(&Path::new(self.location.dirname()), UserDir).map_err(io_error));
        try!(rmdir_recursive(&self.location).map_err(io_error));

        git!(self.location, "clone --no-checkout --quiet {} {}", self.get_source().display(), self.location.display());
        try!(chmod(&self.location, AllPermissions).map_err(io_error));

        Ok(())
    }

    fn fetch(&self) -> CargoResult<()> {
        Ok(git!(self.location, "fetch --force --quiet --tags {}", self.get_source().display()))
    }

    fn reset<T: Show>(&self, revision: T) -> CargoResult<()> {
        Ok(git!(self.location, "reset --hard {}", revision))
    }

    fn update_submodules(&self) -> CargoResult<()> {
        Ok(git!(self.location, "submodule update --init --recursive"))
    }
}

impl GitRepo {
    fn get_path<'a>(&'a self) -> &'a Path {
        &self.config.path
    }

    #[allow(unused_variable)]
    fn copy_to<'a>(&'a self, dest: Path) -> CargoResult<GitCheckout<'a>> {
        let checkout = try!(GitCheckout::clone(dest, self));

        try!(checkout.fetch());
        try!(checkout.reset(self.revision.as_slice()));
        try!(checkout.update_submodules());

        Ok(checkout)
    }

    fn clone_to(&self, destination: &Path) -> CargoResult<()> {
        try!(mkdir_recursive(&Path::new(destination.dirname()), UserDir).map_err(io_error));
        try!(rmdir_recursive(destination).map_err(io_error));
        git!(self.config.path, "clone --no-checkout --quiet {} {}", self.config.path.display(), destination.display());
        try!(chmod(destination, AllPermissions).map_err(io_error));

        git!(*destination, "fetch --force --quiet --tags {}", self.config.path.display());
        git!(*destination, "reset --hard {}", self.revision);
        git!(*destination, "submodule update --init --recursive");

        Ok(())
    }
}

fn rev_for(config: &GitConfig) -> CargoResult<String> {
    Ok(git_output!(config.path, "rev-parse {}", config.reference))
}

#[allow(dead_code)]
fn has_rev<T: Show>(path: &Path, rev: T) -> bool {
    git_output(path, format!("cat-file -e {}", rev)).is_ok()
}

fn git(path: &Path, str: &str) -> ProcessBuilder {
    println!("Executing git {} @ {}", str, path.display());
    process("git").args(str.split(' ').collect::<Vec<&str>>().as_slice()).cwd(path.clone())
}

fn git_inherit(path: &Path, str: String) -> CargoResult<()> {
    git(path, str.as_slice()).exec().map_err(|err|
        human_error(format!("Couldn't execute `git {}`: {}", str, err), None::<&str>, err))
}

fn git_output(path: &Path, str: String) -> CargoResult<String> {
    let output = try!(git(path, str.as_slice()).exec_with_output().map_err(|err|
        human_error(format!("Couldn't execute `git {}`", str), None::<&str>, err)));

    Ok(to_str(output.output.as_slice()))
}

fn to_str(vec: &[u8]) -> String {
    str::from_utf8_lossy(vec).to_str()
}
