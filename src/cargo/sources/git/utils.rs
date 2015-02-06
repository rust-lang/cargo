use std::fmt::{self, Formatter};
use std::old_io::{USER_DIR};
use std::old_io::fs::{mkdir_recursive, rmdir_recursive, PathExtensions};
use rustc_serialize::{Encodable, Encoder};
use url::Url;
use git2::{self, ObjectType};

use core::GitReference;
use util::{CargoResult, ChainError, human, ToUrl, internal};

#[derive(PartialEq, Clone, Debug)]
#[allow(missing_copy_implementations)]
pub struct GitRevision(git2::Oid);

impl fmt::Display for GitRevision {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

/// GitRemote represents a remote repository. It gets cloned into a local
/// GitDatabase.
#[derive(PartialEq,Clone,Debug)]
pub struct GitRemote {
    url: Url,
}

#[derive(PartialEq,Clone,RustcEncodable)]
struct EncodableGitRemote {
    url: String,
}

impl Encodable for GitRemote {
    fn encode<S: Encoder>(&self, s: &mut S) -> Result<(), S::Error> {
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

#[derive(RustcEncodable)]
pub struct EncodableGitDatabase {
    remote: GitRemote,
    path: String,
}

impl Encodable for GitDatabase {
    fn encode<S: Encoder>(&self, s: &mut S) -> Result<(), S::Error> {
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

#[derive(RustcEncodable)]
pub struct EncodableGitCheckout {
    database: EncodableGitDatabase,
    location: String,
    revision: String,
}

impl<'a> Encodable for GitCheckout<'a> {
    fn encode<S: Encoder>(&self, s: &mut S) -> Result<(), S::Error> {
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

    pub fn url(&self) -> &Url {
        &self.url
    }

    pub fn rev_for(&self, path: &Path, reference: &GitReference)
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
        // Create a local anonymous remote in the repository to fetch the url
        let url = self.url.to_string();
        let refspec = "refs/heads/*:refs/heads/*";
        fetch(dst, &url, refspec)
    }

    fn clone_into(&self, dst: &Path) -> CargoResult<git2::Repository> {
        let url = self.url.to_string();
        if dst.exists() {
            try!(rmdir_recursive(dst));
        }
        try!(mkdir_recursive(dst, USER_DIR));
        let repo = try!(git2::Repository::init_bare(dst));
        try!(fetch(&repo, &url, "refs/heads/*:refs/heads/*"));
        Ok(repo)
    }
}

impl GitDatabase {
    fn path<'a>(&'a self) -> &'a Path {
        &self.path
    }

    pub fn copy_to(&self, rev: GitRevision, dest: &Path)
                   -> CargoResult<GitCheckout> {
        let checkout = match git2::Repository::open(dest) {
            Ok(repo) => {
                let checkout = GitCheckout::new(dest, self, rev, repo);
                if !checkout.is_fresh() {
                    try!(checkout.fetch());
                    try!(checkout.reset());
                    assert!(checkout.is_fresh());
                }
                checkout
            }
            Err(..) => try!(GitCheckout::clone_into(dest, self, rev)),
        };
        try!(checkout.update_submodules().chain_error(|| {
            internal("failed to update submodules")
        }));
        Ok(checkout)
    }

    pub fn rev_for(&self, reference: &GitReference) -> CargoResult<GitRevision> {
        let id = match *reference {
            GitReference::Tag(ref s) => {
                try!((|:| {
                    let refname = format!("refs/tags/{}", s);
                    let id = try!(self.repo.refname_to_id(&refname));
                    let obj = try!(self.repo.find_object(id, None));
                    let obj = try!(obj.peel(ObjectType::Commit));
                    Ok(obj.id())
                }).chain_error(|| {
                    human(format!("failed to find tag `{}`", s))
                }))
            }
            GitReference::Branch(ref s) => {
                try!((|:| {
                    let b = try!(self.repo.find_branch(s, git2::BranchType::Local));
                    b.get().target().chain_error(|| {
                        human(format!("branch `{}` did not have a target", s))
                    })
                }).chain_error(|| {
                    human(format!("failed to find branch `{}`", s))
                }))
            }
            GitReference::Rev(ref s) => {
                let obj = try!(self.repo.revparse_single(s));
                obj.id()
            }
        };
        Ok(GitRevision(id))
    }

    pub fn has_ref(&self, reference: &str) -> CargoResult<()> {
        try!(self.repo.revparse_single(reference));
        Ok(())
    }
}

impl<'a> GitCheckout<'a> {
    fn new(path: &Path, database: &'a GitDatabase, revision: GitRevision,
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

    fn clone_into(into: &Path, database: &'a GitDatabase,
                  revision: GitRevision)
                  -> CargoResult<GitCheckout<'a>>
    {
        let repo = try!(GitCheckout::clone_repo(database.path(), into));
        let checkout = GitCheckout::new(into, database, revision, repo);
        try!(checkout.reset());
        Ok(checkout)
    }

    fn clone_repo(source: &Path, into: &Path) -> CargoResult<git2::Repository> {
        let dirname = into.dir_path();

        try!(mkdir_recursive(&dirname, USER_DIR).chain_error(|| {
            human(format!("Couldn't mkdir {}", dirname.display()))
        }));

        if into.exists() {
            try!(rmdir_recursive(into).chain_error(|| {
                human(format!("Couldn't rmdir {}", into.display()))
            }));
        }

        let url = try!(source.to_url().map_err(human));
        let url = url.to_string();
        let repo = try!(git2::Repository::clone(&url, into).chain_error(|| {
            internal(format!("failed to clone {} into {}", source.display(),
                             into.display()))
        }));
        Ok(repo)
    }

    fn is_fresh(&self) -> bool {
        match self.repo.revparse_single("HEAD") {
            Ok(head) => head.id().to_string() == self.revision.to_string(),
            _ => false,
        }
    }

    fn fetch(&self) -> CargoResult<()> {
        info!("fetch {}", self.repo.path().display());
        let url = try!(self.database.path.to_url().map_err(human));
        let url = url.to_string();
        let refspec = "refs/heads/*:refs/heads/*";
        try!(fetch(&self.repo, &url, refspec));
        Ok(())
    }

    fn reset(&self) -> CargoResult<()> {
        info!("reset {} to {}", self.repo.path().display(), self.revision);
        let object = try!(self.repo.find_object(self.revision.0, None));
        try!(self.repo.reset(&object, git2::ResetType::Hard, None, None, None));
        Ok(())
    }

    fn update_submodules(&self) -> CargoResult<()> {
        return update_submodules(&self.repo);

        fn update_submodules(repo: &git2::Repository) -> CargoResult<()> {
            info!("update submodules for: {}", repo.path().display());

            for mut child in try!(repo.submodules()).into_iter() {
                try!(child.init(false));
                let url = try!(child.url().chain_error(|| {
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
                let head_and_repo = child.open().and_then(|repo| {
                    Ok((try!(repo.head()).target(), repo))
                });
                let repo = match head_and_repo {
                    Ok((head, repo)) => {
                        if child.head_id() == head {
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
                try!(fetch(&repo, url, refspec).chain_error(|| {
                    internal(format!("failed to fetch submodule `{}` from {}",
                                     child.name().unwrap_or(""), url))
                }));

                let obj = try!(repo.find_object(head, None));
                try!(repo.reset(&obj, git2::ResetType::Hard, None, None, None));
                try!(update_submodules(&repo));
            }
            Ok(())
        }
    }
}

fn with_authentication<T, F>(url: &str, cfg: &git2::Config, mut f: F)
                             -> CargoResult<T>
    where F: FnMut(&mut git2::Credentials) -> CargoResult<T>
{
    // Prepare the authentication callbacks.
    //
    // We check the `allowed` types of credentials, and we try to do as much as
    // possible based on that:
    //
    // * Prioritize SSH keys from the local ssh agent as they're likely the most
    //   reliable. The username here is prioritized from the credential
    //   callback, then from whatever is configured in git itself, and finally
    //   we fall back to the generic user of `git`.
    //
    // * If a username/password is allowed, then we fallback to git2-rs's
    //   implementation of the credential helper. This is what is configured
    //   with `credential.helper` in git, and is the interface for the OSX
    //   keychain, for example.
    //
    // * After the above two have failed, we just kinda grapple attempting to
    //   return *something*.
    let mut cred_helper = git2::CredentialHelper::new(url);
    cred_helper.config(cfg);
    let mut cred_error = false;
    let ret = f(&mut |url, username, allowed| {
        let creds = if allowed.contains(git2::SSH_KEY) {
            let user = username.map(|s| s.to_string())
                               .or_else(|| cred_helper.username.clone())
                               .unwrap_or("git".to_string());
            git2::Cred::ssh_key_from_agent(&user)
        } else if allowed.contains(git2::USER_PASS_PLAINTEXT) {
            git2::Cred::credential_helper(cfg, url, username)
        } else if allowed.contains(git2::DEFAULT) {
            git2::Cred::default()
        } else {
            Err(git2::Error::from_str("no authentication available"))
        };
        cred_error = creds.is_err();
        creds
    });
    if cred_error {
        ret.chain_error(|| {
            human("Failed to authenticate when downloading repository")
        })
    } else {
        ret
    }
}

pub fn fetch(repo: &git2::Repository, url: &str,
             refspec: &str) -> CargoResult<()> {
    // Create a local anonymous remote in the repository to fetch the url

    with_authentication(url, &try!(repo.config()), |f| {
        let mut cb = git2::RemoteCallbacks::new();
        cb.credentials(|a, b, c| f(a, b, c));
        let mut remote = try!(repo.remote_anonymous(&url, Some(refspec)));
        try!(remote.add_fetch("refs/tags/*:refs/tags/*"));
        remote.set_callbacks(&mut cb);
        try!(remote.fetch(&["refs/tags/*:refs/tags/*", refspec], None, None));
        Ok(())
    })
}
