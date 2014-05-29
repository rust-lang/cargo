#![allow(dead_code)]

use url::Url;
use util::{CargoResult,ProcessBuilder,io_error,human_error,process};
use std::fmt;
use std::fmt::{Show,Formatter};
use std::str;
use std::io::{UserDir,AllPermissions};
use std::io::fs::{mkdir_recursive,rmdir_recursive,chmod};
use serialize::{Encodable,Encoder};
use core::source::Source;
use core::{NameVer,Package,Summary};
use ops;

pub struct GitSource {
    config: GitConfig,
    dest: Path
}

impl GitSource {
    pub fn new(config: GitConfig, dest: Path) -> GitSource {
        GitSource { config: config, dest: dest }
    }
}

impl Show for GitSource {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        try!(write!(f, "git repo at {}", self.config.url));

        if self.config.reference.as_slice() != "master" {
            try!(write!(f, " ({})", self.config.reference));
        }

        Ok(())
    }
}

impl Source for GitSource {
    fn update(&self) -> CargoResult<()> {
        let remote = GitRemoteRepo::from_config(self.config.clone());
        let repo = try!(remote.checkout());

        try!(repo.copy_to(&self.dest));

        Ok(())
    }

    fn list(&self) -> CargoResult<Vec<Summary>> {
        let pkg = try!(read_manifest(&self.dest));
        Ok(vec!(pkg.get_summary().clone()))
    }

    fn download(&self, _: &[NameVer]) -> CargoResult<()> {
        Ok(())
    }

    fn get(&self, packages: &[NameVer]) -> CargoResult<Vec<Package>> {
        let pkg = try!(read_manifest(&self.dest));

        if packages.iter().any(|nv| pkg.is_for_name_ver(nv)) {
            Ok(vec!(pkg))
        } else {
            Ok(vec!())
        }
    }
}

macro_rules! git(
    ($config:expr, $verbose:expr, $str:expr, $($rest:expr),*) => (
        try!(git_inherit(&$config, $verbose, format!($str, $($rest),*)))
    );

    ($config:expr, $verbose:expr, $str:expr) => (
        try!(git_inherit(&$config, $verbose, format!($str)))
    );
)

macro_rules! git_output(
    ($config:expr, $verbose:expr, $str:expr, $($rest:expr),*) => (
        try!(git_output(&$config, $verbose, format!($str, $($rest),*)))
    );

    ($config:expr, $verbose:expr, $str:expr) => (
        try!(git_output(&$config, $verbose, format!($str)))
    );
)

macro_rules! errln(
    ($($arg:tt)*) => (let _ = writeln!(::std::io::stdio::stderr(), $($arg)*))
)

/**
 * GitConfig represents the information about a git location for code determined from
 * a Cargo manifest, as well as a location to store the git database for a remote
 * repository.
 */

#[deriving(Eq,Clone)]
pub struct GitConfig {
    path: Path,
    url: Url,
    reference: String,
    verbose: bool
}

#[deriving(Eq,Clone,Encodable)]
struct EncodableGitConfig {
    path: String,
    url: String,
    reference: String
}

/**
 * GitRemoteRepo is responsible for taking a GitConfig and bringing the local database up
 * to date with the remote repository, returning a GitRepo.
 *
 * A GitRemoteRepo has a `reference` in its config, which may not resolve to a valid revision.
 * Its `checkout` method returns a `GitRepo` which is guaranteed to have a resolved
 * revision for the supplied reference.
 */

#[deriving(Eq,Clone)]
pub struct GitRemoteRepo {
    config: GitConfig
}

/**
 * GitRepo is a local clone of a remote repository's database. The supplied reference is
 * guaranteed to resolve to a valid `revision`, so all code run from this point forward
 * can assume that the requested code exists.
 */

#[deriving(Eq,Clone,Encodable)]
pub struct GitRepo {
    config: GitConfig,
    revision: String
}

/**
 * GitCheckout is a local checkout of a particular revision. A single GitRepo can
 * have multiple GitCheckouts.
 */

pub struct GitCheckout<'a> {
    location: Path,
    repo: &'a GitRepo
}

impl<E, S: Encoder<E>> Encodable<S, E> for GitConfig {
    fn encode(&self, s: &mut S) -> Result<(), E> {
        EncodableGitConfig {
            path: self.path.display().to_str(),
            url: self.url.to_str(),
            reference: self.reference.clone()
        }.encode(s)
    }
}

impl GitRemoteRepo {
    pub fn new(path: Path, url: Url, reference: String, verbose: bool) -> GitRemoteRepo {
        GitRemoteRepo { config: GitConfig { path: path, url: url, reference: reference, verbose: verbose } }
    }

    pub fn from_config(config: GitConfig) -> GitRemoteRepo {
        GitRemoteRepo { config: config }
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
        Ok(git!(self.config.path, self.config.verbose, "fetch --force --quiet --tags {} refs/heads/*:refs/heads/*", self.config.url))
    }

    fn clone(&self) -> CargoResult<()> {
        let dirname = Path::new(self.config.path.dirname());

        try!(mkdir_recursive(&self.config.path, UserDir).map_err(|err|
            human_error(format!("Couldn't recursively create `{}`", dirname.display()), format!("path={}", dirname.display()), io_error(err))));

        Ok(git!(dirname, self.config.verbose, "clone {} {} --bare --no-hardlinks --quiet", self.config.url, self.config.path.display()))
    }
}

impl GitRepo {
    fn get_path<'a>(&'a self) -> &'a Path {
        &self.config.path
    }

    pub fn copy_to<'a>(&'a self, dest: &Path) -> CargoResult<GitCheckout<'a>> {
        let checkout = try!(GitCheckout::clone(dest, self));

        try!(checkout.fetch());
        try!(checkout.reset(self.revision.as_slice()));
        try!(checkout.update_submodules());

        Ok(checkout)
    }
}

impl<'a> GitCheckout<'a> {
    fn clone<'a>(into: &Path, repo: &'a GitRepo) -> CargoResult<GitCheckout<'a>> {
        let checkout = GitCheckout { location: into.clone(), repo: repo };

        // If the git checkout already exists, we don't need to clone it again
        if !checkout.location.join(".git").exists() {
            try!(checkout.clone_repo());
        }

        Ok(checkout)
    }

    fn get_source<'a>(&'a self) -> &'a Path {
        self.repo.get_path()
    }

    fn get_verbose(&self) -> bool {
        self.repo.config.verbose
    }

    fn clone_repo(&self) -> CargoResult<()> {
        let dirname = Path::new(self.location.dirname());

        try!(mkdir_recursive(&dirname, UserDir).map_err(|e|
            human_error(format!("Couldn't mkdir {}", Path::new(self.location.dirname()).display()), None::<&str>, io_error(e))));

        if self.location.exists() {
            try!(rmdir_recursive(&self.location).map_err(|e|
                human_error(format!("Couldn't rmdir {}", Path::new(&self.location).display()), None::<&str>, io_error(e))));
        }

        git!(dirname, self.get_verbose(), "clone --no-checkout --quiet {} {}", self.get_source().display(), self.location.display());
        try!(chmod(&self.location, AllPermissions).map_err(io_error));

        Ok(())
    }

    fn fetch(&self) -> CargoResult<()> {
        Ok(git!(self.location, self.get_verbose(), "fetch --force --quiet --tags {}", self.get_source().display()))
    }

    fn reset<T: Show>(&self, revision: T) -> CargoResult<()> {
        Ok(git!(self.location, self.get_verbose(), "reset -q --hard {}", revision))
    }

    fn update_submodules(&self) -> CargoResult<()> {
        Ok(git!(self.location, self.get_verbose(), "submodule update --init --recursive --quiet"))
    }
}

fn rev_for(config: &GitConfig) -> CargoResult<String> {
    Ok(git_output!(config.path, config.verbose, "rev-parse {}", config.reference))
}

#[allow(dead_code)]
fn has_rev<T: Show>(path: &Path, rev: T) -> bool {
    git_output(path, false, format!("cat-file -e {}", rev)).is_ok()
}

fn git(path: &Path, verbose: bool, str: &str) -> ProcessBuilder {
    if verbose {
        errln!("Executing git {} @ {}", str, path.display());
    }

    process("git").args(str.split(' ').collect::<Vec<&str>>().as_slice()).cwd(path.clone())
}

fn git_inherit(path: &Path, verbose: bool, str: String) -> CargoResult<()> {
    git(path, verbose, str.as_slice()).exec().map_err(|err|
        human_error(format!("Couldn't execute `git {}`: {}", str, err), None::<&str>, err))
}

fn git_output(path: &Path, verbose: bool, str: String) -> CargoResult<String> {
    let output = try!(git(path, verbose, str.as_slice()).exec_with_output().map_err(|err|
        human_error(format!("Couldn't execute `git {}`", str), None::<&str>, err)));

    Ok(to_str(output.output.as_slice()).as_slice().trim_right().to_str())
}

fn to_str(vec: &[u8]) -> String {
    str::from_utf8_lossy(vec).to_str()
}

fn read_manifest(path: &Path) -> CargoResult<Package> {
    let joined = path.join("Cargo.toml");
    ops::read_manifest(joined.as_str().unwrap())
}
