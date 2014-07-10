use std::fmt;
use std::fmt::{Show,Formatter};
use std::str;
use std::io::{UserDir,AllPermissions};
use std::io::fs::{mkdir_recursive,rmdir_recursive,chmod};
use serialize::{Encodable,Encoder};

use core::source::{Location, Local, Remote};
use util::{CargoResult, ChainError, ProcessBuilder, process, human};

#[deriving(PartialEq,Clone,Encodable)]
pub enum GitReference {
    Master,
    Other(String)
}

impl GitReference {
    pub fn for_str<S: Str>(string: S) -> GitReference {
        if string.as_slice() == "master" {
            Master
        } else {
            Other(string.as_slice().to_string())
        }
    }
}

impl Str for GitReference {
    fn as_slice<'a>(&'a self) -> &'a str {
        match *self {
            Master => "master",
            Other(ref string) => string.as_slice()
        }
    }
}

impl Show for GitReference {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        self.as_slice().fmt(f)
    }
}


macro_rules! git(
    ($config:expr, $str:expr, $($rest:expr),*) => (
        try!(git_inherit(&$config, format!($str, $($rest),*)))
    );

    ($config:expr, $str:expr) => (
        try!(git_inherit(&$config, format!($str)))
    );
)

macro_rules! git_output(
    ($config:expr, $str:expr, $($rest:expr),*) => ({
        try!(git_output(&$config, format!($str, $($rest),*)))
    });

    ($config:expr, $str:expr) => ({
        try!(git_output(&$config, format!($str)))
    });
)

macro_rules! errln(
    ($($arg:tt)*) => (let _ = writeln!(::std::io::stdio::stderr(), $($arg)*))
)

/// GitRemote represents a remote repository. It gets cloned into a local
/// GitDatabase.
#[deriving(PartialEq,Clone,Show)]
pub struct GitRemote {
    location: Location,
}

#[deriving(PartialEq,Clone,Encodable)]
struct EncodableGitRemote {
    location: String,
}

impl<E, S: Encoder<E>> Encodable<S, E> for GitRemote {
    fn encode(&self, s: &mut S) -> Result<(), E> {
        EncodableGitRemote {
            location: self.location.to_string()
        }.encode(s)
    }
}

/// GitDatabase is a local clone of a remote repository's database. Multiple
/// GitCheckouts can be cloned from this GitDatabase.
#[deriving(PartialEq,Clone)]
pub struct GitDatabase {
    remote: GitRemote,
    path: Path,
}

#[deriving(Encodable)]
pub struct EncodableGitDatabase {
    remote: GitRemote,
    path: String,
}

impl<E, S: Encoder<E>> Encodable<S, E> for GitDatabase {
    fn encode(&self, s: &mut S) -> Result<(), E> {
        EncodableGitDatabase {
            remote: self.remote.clone(),
            path: self.path.display().to_string()
        }.encode(s)
    }
}

/// GitCheckout is a local checkout of a particular revision. Calling
/// `clone_into` with a reference will resolve the reference into a revision,
/// and return a CargoError if no revision for that reference was found.
pub struct GitCheckout {
    database: GitDatabase,
    location: Path,
    reference: GitReference,
    revision: String,
}

#[deriving(Encodable)]
pub struct EncodableGitCheckout {
    database: GitDatabase,
    location: String,
    reference: String,
    revision: String,
}

impl<E, S: Encoder<E>> Encodable<S, E> for GitCheckout {
    fn encode(&self, s: &mut S) -> Result<(), E> {
        EncodableGitCheckout {
            database: self.database.clone(),
            location: self.location.display().to_string(),
            reference: self.reference.to_string(),
            revision: self.revision.to_string()
        }.encode(s)
    }
}

// Implementations

impl GitRemote {
    pub fn new(location: &Location) -> GitRemote {
        GitRemote { location: location.clone() }
    }

    pub fn get_location<'a>(&'a self) -> &'a Location {
        &self.location
    }

    pub fn has_ref<S: Str>(&self, path: &Path, reference: S) -> CargoResult<()> {
        git_output!(*path, "rev-parse {}", reference.as_slice());
        Ok(())
    }

    pub fn checkout(&self, into: &Path) -> CargoResult<GitDatabase> {
        if into.exists() {
            try!(self.fetch_into(into));
        } else {
            try!(self.clone_into(into));
        }

        Ok(GitDatabase { remote: self.clone(), path: into.clone() })
    }

    pub fn db_at(&self, db_path: &Path) -> GitDatabase {
        GitDatabase { remote: self.clone(), path: db_path.clone() }
    }

    fn fetch_into(&self, path: &Path) -> CargoResult<()> {
        Ok(git!(*path, "fetch --force --quiet --tags {} \
                        refs/heads/*:refs/heads/*", self.fetch_location()))
    }

    fn clone_into(&self, path: &Path) -> CargoResult<()> {
        let dirname = Path::new(path.dirname());

        try!(mkdir_recursive(path, UserDir));

        Ok(git!(dirname, "clone {} {} --bare --no-hardlinks --quiet",
                self.fetch_location(), path.display()))
    }

    fn fetch_location(&self) -> String {
        match self.location {
            Local(ref p) => p.display().to_string(),
            Remote(ref u) => u.to_string(),
        }
    }
}

impl GitDatabase {
    fn get_path<'a>(&'a self) -> &'a Path {
        &self.path
    }

    pub fn copy_to<S: Str>(&self, reference: S,
                           dest: &Path) -> CargoResult<GitCheckout> {
        let checkout = try!(GitCheckout::clone_into(dest, self.clone(),
                                  GitReference::for_str(reference.as_slice())));

        try!(checkout.fetch());
        try!(checkout.update_submodules());

        Ok(checkout)
    }

    pub fn rev_for<S: Str>(&self, reference: S) -> CargoResult<String> {
        Ok(git_output!(self.path, "rev-parse {}", reference.as_slice()))
    }

}

impl GitCheckout {
    fn clone_into(into: &Path, database: GitDatabase,
                  reference: GitReference) -> CargoResult<GitCheckout> {
        let revision = try!(database.rev_for(reference.as_slice()));
        let checkout = GitCheckout {
            location: into.clone(),
            database: database,
            reference: reference,
            revision: revision,
        };

        // If the git checkout already exists, we don't need to clone it again
        if !checkout.location.join(".git").exists() {
            try!(checkout.clone_repo());
        }

        Ok(checkout)
    }

    fn get_source<'a>(&'a self) -> &'a Path {
        self.database.get_path()
    }

    fn clone_repo(&self) -> CargoResult<()> {
        let dirname = Path::new(self.location.dirname());

        try!(mkdir_recursive(&dirname, UserDir).chain_error(|| {
            human(format!("Couldn't mkdir {}",
                          Path::new(self.location.dirname()).display()))
        }));

        if self.location.exists() {
            try!(rmdir_recursive(&self.location).chain_error(|| {
                human(format!("Couldn't rmdir {}",
                              Path::new(&self.location).display()))
            }));
        }

        git!(dirname, "clone --no-checkout --quiet {} {}",
             self.get_source().display(), self.location.display());
        try!(chmod(&self.location, AllPermissions));

        Ok(())
    }

    fn fetch(&self) -> CargoResult<()> {
        // In git 1.8, apparently --tags explicitly *only* fetches tags, it does
        // not fetch anything else. In git 1.9, however, git apparently fetches
        // everything when --tags is passed.
        //
        // This means that if we want to fetch everything we need to execute
        // both with and without --tags on 1.8 (apparently), and only with
        // --tags on 1.9. For simplicity, we execute with and without --tags for
        // all gits.
        //
        // FIXME: This is suspicious. I have been informed that, for example,
        //        bundler does not do this, yet bundler appears to work!
        //
        // And to continue the fun, git before 1.7.3 had the fun bug that if a
        // branch was tracking a remote, then `git fetch $url` doesn't work!
        //
        // For details, see
        // https://www.kernel.org/pub/software/scm/git/docs/RelNotes-1.7.3.txt
        //
        // In this case we just use `origin` here instead of the database path.
        git!(self.location, "fetch --force --quiet origin");
        git!(self.location, "fetch --force --quiet --tags origin");
        try!(self.reset(self.revision.as_slice()));
        Ok(())
    }

    fn reset<T: Show>(&self, revision: T) -> CargoResult<()> {
        Ok(git!(self.location, "reset -q --hard {}", revision))
    }

    fn update_submodules(&self) -> CargoResult<()> {
        Ok(git!(self.location, "submodule update --init --recursive --quiet"))
    }
}

fn git(path: &Path, str: &str) -> ProcessBuilder {
    debug!("Executing git {} @ {}", str, path.display());

    process("git").args(str.split(' ').collect::<Vec<&str>>().as_slice())
                  .cwd(path.clone())
}

fn git_inherit(path: &Path, str: String) -> CargoResult<()> {
    git(path, str.as_slice()).exec().chain_error(|| {
        human(format!("Executing `git {}` failed", str))
    })
}

fn git_output(path: &Path, str: String) -> CargoResult<String> {
    let output = try!(git(path, str.as_slice()).exec_with_output()
                                                     .chain_error(||
        human(format!("Executing `git {}` failed", str))));

    Ok(to_str(output.output.as_slice()).as_slice().trim_right().to_string())
}

fn to_str(vec: &[u8]) -> String {
    str::from_utf8_lossy(vec).to_string()
}

