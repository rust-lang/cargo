use std::fmt::{mod, Show, Formatter};
use std::io::{UserDir};
use std::io::fs::{mkdir_recursive,rmdir_recursive};
use serialize::{Encodable,Encoder};
use url::Url;
use git2;

use util::{CargoResult, ChainError, human, ToUrl, internal, Require};

#[deriving(PartialEq,Clone,Encodable)]
pub enum GitReference {
    Master,
    Other(String)
}

#[deriving(PartialEq,Clone,Encodable)]
pub struct GitRevision(String);

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
    fn as_slice(&self) -> &str {
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

impl Str for GitRevision {
    fn as_slice(&self) -> &str {
        let GitRevision(ref me) = *self;
        me.as_slice()
    }
}

impl Show for GitRevision {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        self.as_slice().fmt(f)
    }
}

/// GitRemote represents a remote repository. It gets cloned into a local
/// GitDatabase.
#[deriving(PartialEq,Clone,Show)]
pub struct GitRemote {
    url: Url,
}

#[deriving(PartialEq,Clone,Encodable)]
struct EncodableGitRemote {
    url: String,
}

impl<E, S: Encoder<E>> Encodable<S, E> for GitRemote {
    fn encode(&self, s: &mut S) -> Result<(), E> {
        EncodableGitRemote {
            url: self.url.to_string()
        }.encode(s)
    }
}

/// GitDatabase is a local clone of a remote repository's database. Multiple
/// GitCheckouts can be cloned from this GitDatabase.
pub struct GitDatabase {
    remote: GitRemote,
    path: Path,
    repo: git2::Repository,
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
pub struct GitCheckout<'a> {
    database: &'a GitDatabase,
    location: Path,
    revision: GitRevision,
    repo: git2::Repository,
}

#[deriving(Encodable)]
pub struct EncodableGitCheckout {
    database: EncodableGitDatabase,
    location: String,
    revision: String,
}

impl<'a, E, S: Encoder<E>> Encodable<S, E> for GitCheckout<'a> {
    fn encode(&self, s: &mut S) -> Result<(), E> {
        EncodableGitCheckout {
            location: self.location.display().to_string(),
            revision: self.revision.to_string(),
            database: EncodableGitDatabase {
                remote: self.database.remote.clone(),
                path: self.database.path.display().to_string(),
            },
        }.encode(s)
    }
}

// Implementations

impl GitRemote {
    pub fn new(url: &Url) -> GitRemote {
        GitRemote { url: url.clone() }
    }

    pub fn get_url(&self) -> &Url {
        &self.url
    }

    pub fn rev_for<S: Str>(&self, path: &Path, reference: S)
                           -> CargoResult<GitRevision> {
        let db = try!(self.db_at(path));
        db.rev_for(reference)
    }

    pub fn checkout(&self, into: &Path) -> CargoResult<GitDatabase> {
        let repo = match git2::Repository::open(into) {
            Ok(repo) => {
                try!(self.fetch_into(&repo).chain_error(|| {
                    internal(format!("failed to fetch into {}", into.display()))
                }));
                repo
            }
            Err(..) => {
                try!(self.clone_into(into).chain_error(|| {
                    internal(format!("failed to clone into: {}", into.display()))
                }))
            }
        };

        Ok(GitDatabase { remote: self.clone(), path: into.clone(), repo: repo })
    }

    pub fn db_at(&self, db_path: &Path) -> CargoResult<GitDatabase> {
        let repo = try!(git2::Repository::open(db_path));
        Ok(GitDatabase {
            remote: self.clone(),
            path: db_path.clone(),
            repo: repo,
        })
    }

    fn fetch_into(&self, dst: &git2::Repository) -> CargoResult<()> {
        let url = self.url.to_string();
        let refspec = "refs/heads/*:refs/heads/*";
        let mut remote = try!(dst.remote_create_anonymous(url.as_slice(),
                                                          refspec));
        try!(remote.add_fetch("refs/tags/*:refs/tags/*"));
        try!(remote.fetch(None, None));
        Ok(())
    }

    fn clone_into(&self, dst: &Path) -> CargoResult<git2::Repository> {
        let url = self.url.to_string();
        if dst.exists() {
            try!(rmdir_recursive(dst));
        }
        try!(mkdir_recursive(dst, UserDir));
        let repo = try!(git2::build::RepoBuilder::new().bare(true)
                                                       .hardlinks(false)
                                                       .clone(url.as_slice(), dst));
        Ok(repo)
    }
}

impl GitDatabase {
    fn get_path<'a>(&'a self) -> &'a Path {
        &self.path
    }

    pub fn copy_to(&self, rev: GitRevision, dest: &Path)
                   -> CargoResult<GitCheckout> {
        match git2::Repository::open(dest) {
            Ok(repo) => {
                let is_fresh = match repo.revparse_single("HEAD") {
                    Ok(head) => head.id().to_string() == rev.to_string(),
                    _ => false,
                };
                if is_fresh {
                    return Ok(GitCheckout::new(dest, self, rev, repo))
                }
            }
            _ => {}
        }

        GitCheckout::clone_into(dest, self, rev)
    }

    pub fn rev_for<S: Str>(&self, reference: S) -> CargoResult<GitRevision> {
        let rev = try!(self.repo.revparse_single(reference.as_slice()));
        Ok(GitRevision(rev.id().to_string()))
    }

    pub fn has_ref<S: Str>(&self, reference: S) -> CargoResult<()> {
        try!(self.repo.revparse_single(reference.as_slice()));
        Ok(())
    }
}

impl<'a> GitCheckout<'a> {
    fn new<'a>(path: &Path, database: &'a GitDatabase, revision: GitRevision,
               repo: git2::Repository)
               -> GitCheckout<'a>
    {
        GitCheckout {
            location: path.clone(),
            database: database,
            revision: revision,
            repo: repo,
        }
    }

    fn clone_into<'a>(into: &Path, database: &'a GitDatabase,
                      revision: GitRevision)
                      -> CargoResult<GitCheckout<'a>>
    {
        let repo = try!(GitCheckout::clone_repo(database.get_path(), into));
        let checkout = GitCheckout::new(into, database, revision, repo);

        try!(checkout.reset().chain_error(|| {
            internal("failed to reset to the right revision")
        }));
        try!(checkout.update_submodules().chain_error(|| {
            internal("failed to update submodules")
        }));

        Ok(checkout)
    }

    pub fn get_rev(&self) -> &str {
        self.revision.as_slice()
    }

    fn clone_repo(source: &Path, into: &Path) -> CargoResult<git2::Repository> {
        let dirname = into.dir_path();

        try!(mkdir_recursive(&dirname, UserDir).chain_error(|| {
            human(format!("Couldn't mkdir {}", dirname.display()))
        }));

        if into.exists() {
            try!(rmdir_recursive(into).chain_error(|| {
                human(format!("Couldn't rmdir {}", into.display()))
            }));
        }

        let url = try!(source.to_url().map_err(human));
        let url = url.to_string();
        let repo = try!(git2::Repository::clone(url.as_slice(),
                                                into).chain_error(|| {
            internal(format!("failed to clone {} into {}", source.display(),
                             into.display()))
        }));
        Ok(repo)
    }

    fn reset(&self) -> CargoResult<()> {
        info!("reset {} to {}", self.repo.path().display(),
              self.revision.as_slice());
        let oid = try!(git2::Oid::from_str(self.revision.as_slice()));
        let object = try!(git2::Object::lookup(&self.repo, oid, None));
        try!(self.repo.reset(&object, git2::Hard, None, None));
        Ok(())
    }

    fn update_submodules(&self) -> CargoResult<()> {
        return update_submodules(&self.repo);

        fn update_submodules(repo: &git2::Repository) -> CargoResult<()> {
            info!("update submodules for: {}", repo.path().display());

            for mut child in try!(repo.submodules()).move_iter() {
                try!(child.init(false));
                let url = try!(child.url().require(|| {
                    internal("non-utf8 url for submodule")
                }));

                // A submodule which is listed in .gitmodules but not actually
                // checked out will not have a head id, so we should ignore it.
                let head = match child.head_id() {
                    Some(head) => head,
                    None => continue,
                };

                // If the submodule hasn't been checked out yet, we need to
                // clone it. If it has been checked out and the head is the same
                // as the submodule's head, then we can bail out and go to the
                // next submodule.
                let repo = match child.open() {
                    Ok(repo) => {
                        if child.head_id() == try!(repo.head()).target() {
                            continue
                        }
                        repo
                    }
                    Err(..) => {
                        let path = repo.path().dir_path().join(child.path());
                        try!(git2::Repository::clone(url, &path))
                    }
                };

                // Fetch data from origin and reset to the head commit
                let refspec = "refs/heads/*:refs/heads/*";
                let mut remote = try!(repo.remote_create_anonymous(url, refspec));
                try!(remote.fetch(None, None).chain_error(|| {
                    internal(format!("failed to fetch submodule `{}` from {}",
                                     child.name().unwrap_or(""), url))
                }));

                let obj = try!(git2::Object::lookup(&repo, head, None));
                try!(repo.reset(&obj, git2::Hard, None, None));
                try!(update_submodules(&repo));
            }
            Ok(())
        }
    }
}
