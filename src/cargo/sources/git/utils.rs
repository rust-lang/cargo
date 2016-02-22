use std::env;
use std::fmt;
use std::fs::{self, File};
use std::path::{Path, PathBuf};

use rustc_serialize::{Encodable, Encoder};
use url::Url;
use git2::{self, ObjectType};

use core::GitReference;
use util::{CargoResult, ChainError, human, ToUrl, internal};

#[derive(PartialEq, Clone, Debug)]
pub struct GitRevision(git2::Oid);

impl fmt::Display for GitRevision {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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
    path: PathBuf,
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
    location: PathBuf,
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
                    human(format!("failed to fetch into {}", into.display()))
                }));
                repo
            }
            Err(..) => {
                try!(self.clone_into(into).chain_error(|| {
                    human(format!("failed to clone into: {}", into.display()))
                }))
            }
        };

        Ok(GitDatabase {
            remote: self.clone(),
            path: into.to_path_buf(),
            repo: repo,
        })
    }

    pub fn db_at(&self, db_path: &Path) -> CargoResult<GitDatabase> {
        let repo = try!(git2::Repository::open(db_path));
        Ok(GitDatabase {
            remote: self.clone(),
            path: db_path.to_path_buf(),
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
        if fs::metadata(&dst).is_ok() {
            try!(fs::remove_dir_all(dst));
        }
        try!(fs::create_dir_all(dst));
        let repo = try!(git2::Repository::init_bare(dst));
        try!(fetch(&repo, &url, "refs/heads/*:refs/heads/*"));
        Ok(repo)
    }
}

impl GitDatabase {
    fn path(&self) -> &Path {
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
                try!((|| {
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
                try!((|| {
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
            location: path.to_path_buf(),
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
        let dirname = into.parent().unwrap();

        try!(fs::create_dir_all(&dirname).chain_error(|| {
            human(format!("Couldn't mkdir {}", dirname.display()))
        }));

        if fs::metadata(&into).is_ok() {
            try!(fs::remove_dir_all(into).chain_error(|| {
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
            Ok(ref head) if head.id() == self.revision.0 => {
                // See comments in reset() for why we check this
                fs::metadata(self.location.join(".cargo-ok")).is_ok()
            }
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
        // If we're interrupted while performing this reset (e.g. we die because
        // of a signal) Cargo needs to be sure to try to check out this repo
        // again on the next go-round.
        //
        // To enable this we have a dummy file in our checkout, .cargo-ok, which
        // if present means that the repo has been successfully reset and is
        // ready to go. Hence if we start to do a reset, we make sure this file
        // *doesn't* exist, and then once we're done we create the file.
        let ok_file = self.location.join(".cargo-ok");
        let _ = fs::remove_file(&ok_file);
        info!("reset {} to {}", self.repo.path().display(), self.revision);
        let object = try!(self.repo.find_object(self.revision.0, None));
        try!(self.repo.reset(&object, git2::ResetType::Hard, None));
        try!(File::create(ok_file));
        Ok(())
    }

    fn update_submodules(&self) -> CargoResult<()> {
        return update_submodules(&self.repo);

        fn update_submodules(repo: &git2::Repository) -> CargoResult<()> {
            info!("update submodules for: {:?}", repo.workdir().unwrap());

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
                    let target = try!(repo.head()).target();
                    Ok((target, repo))
                });
                let repo = match head_and_repo {
                    Ok((head, repo)) => {
                        if child.head_id() == head {
                            continue
                        }
                        repo
                    }
                    Err(..) => {
                        let path = repo.workdir().unwrap().join(child.path());
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
                try!(repo.reset(&obj, git2::ResetType::Hard, None));
                try!(update_submodules(&repo));
            }
            Ok(())
        }
    }
}

/// Prepare the authentication callbacks for cloning a git repository.
///
/// The main purpose of this function is to construct the "authentication
/// callback" which is used to clone a repository. This callback will attempt to
/// find the right authentication on the system (without user input) and will
/// guide libgit2 in doing so.
///
/// The callback is provided `allowed` types of credentials, and we try to do as
/// much as possible based on that:
///
/// * Prioritize SSH keys from the local ssh agent as they're likely the most
///   reliable. The username here is prioritized from the credential
///   callback, then from whatever is configured in git itself, and finally
///   we fall back to the generic user of `git`.
///
/// * If a username/password is allowed, then we fallback to git2-rs's
///   implementation of the credential helper. This is what is configured
///   with `credential.helper` in git, and is the interface for the OSX
///   keychain, for example.
///
/// * After the above two have failed, we just kinda grapple attempting to
///   return *something*.
///
/// If any form of authentication fails, libgit2 will repeatedly ask us for
/// credentials until we give it a reason to not do so. To ensure we don't
/// just sit here looping forever we keep track of authentications we've
/// attempted and we don't try the same ones again.
fn with_authentication<T, F>(url: &str, cfg: &git2::Config, mut f: F)
                             -> CargoResult<T>
    where F: FnMut(&mut git2::Credentials) -> CargoResult<T>
{
    let mut cred_helper = git2::CredentialHelper::new(url);
    cred_helper.config(cfg);

    let mut attempted = git2::CredentialType::empty();
    let mut failed_cred_helper = false;

    // We try a couple of different user names when cloning via ssh as there's a
    // few possibilities if one isn't mentioned, and these are used to keep
    // track of that.
    enum UsernameAttempt {
        Arg,
        CredHelper,
        Local,
        Git,
    }
    let mut username_attempt = UsernameAttempt::Arg;
    let mut username_attempts = Vec::new();

    let res = f(&mut |url, username, allowed| {
        let allowed = allowed & !attempted;

        // libgit2's "USERNAME" authentication actually means that it's just
        // asking us for a username to keep going. This is currently only really
        // used for SSH authentication and isn't really an authentication type.
        // The logic currently looks like:
        //
        //      let user = ...;
        //      if (user.is_null())
        //          user = callback(USERNAME, null, ...);
        //
        //      callback(SSH_KEY, user, ...)
        //
        // So if we have a USERNAME request we just pass it either `username` or
        // a fallback of "git". We'll do some more principled attempts later on.
        if allowed.contains(git2::USERNAME) {
            attempted = attempted | git2::USERNAME;
            return git2::Cred::username(username.unwrap_or("git"))
        }

        // An "SSH_KEY" authentication indicates that we need some sort of SSH
        // authentication. This can currently either come from the ssh-agent
        // process or from a raw in-memory SSH key. Cargo only supports using
        // ssh-agent currently.
        //
        // We try a few different usernames here, including:
        //
        //  1. The `username` argument, if provided. This will cover cases where
        //     the user was passed in the URL, for example.
        //  2. The global credential helper's username, if any is configured
        //  3. The local account's username (if present)
        //  4. Finally, "git" as it's a common fallback (e.g. with github)
        if allowed.contains(git2::SSH_KEY) {
            loop {
                let name = match username_attempt {
                    UsernameAttempt::Arg => {
                        username_attempt = UsernameAttempt::CredHelper;
                        username.map(|s| s.to_string())
                    }
                    UsernameAttempt::CredHelper => {
                        username_attempt = UsernameAttempt::Local;
                        cred_helper.username.clone()
                    }
                    UsernameAttempt::Local => {
                        username_attempt = UsernameAttempt::Git;
                        env::var("USER").or_else(|_| env::var("USERNAME")).ok()
                    }
                    UsernameAttempt::Git => {
                        attempted = attempted | git2::SSH_KEY;
                        Some("git".to_string())
                    }
                };
                if let Some(name) = name {
                    let ret = git2::Cred::ssh_key_from_agent(&name);
                    username_attempts.push(name);
                    return ret
                }
            }
        }

        // Sometimes libgit2 will ask for a username/password in plaintext. This
        // is where Cargo would have an interactive prompt if we supported it,
        // but we currently don't! Right now the only way we support fetching a
        // plaintext password is through the `credential.helper` support, so
        // fetch that here.
        if allowed.contains(git2::USER_PASS_PLAINTEXT) {
            attempted = attempted | git2::USER_PASS_PLAINTEXT;
            let r = git2::Cred::credential_helper(cfg, url, username);
            failed_cred_helper = r.is_err();
            return r
        }

        // I'm... not sure what the DEFAULT kind of authentication is, but seems
        // easy to support?
        if allowed.contains(git2::DEFAULT) {
            attempted = attempted | git2::DEFAULT;
            return git2::Cred::default()
        }

        // Whelp, we tried our best
        Err(git2::Error::from_str("no authentication available"))
    });

    if attempted.bits() == 0 || res.is_ok() {
        return res
    }

    // In the case of an authentication failure (where we tried something) then
    // we try to give a more helpful error message about precisely what we
    // tried.
    res.chain_error(|| {
        let mut msg = "failed to authenticate when downloading \
                       repository".to_string();
        if attempted.contains(git2::SSH_KEY) {
            let names = username_attempts.iter()
                                         .map(|s| format!("`{}`", s))
                                         .collect::<Vec<_>>()
                                         .join(", ");
            msg.push_str(&format!("\nattempted ssh-agent authentication, but \
                                   none of the usernames {} succeeded", names));
        }
        if attempted.contains(git2::USER_PASS_PLAINTEXT) {
            if failed_cred_helper {
                msg.push_str("\nattempted to find username/password via \
                              git's `credential.helper` support, but failed");
            } else {
                msg.push_str("\nattempted to find username/password via \
                              `credential.helper`, but maybe the found \
                              credentials were incorrect");
            }
        }
        human(msg)
    })
}

pub fn fetch(repo: &git2::Repository, url: &str,
             refspec: &str) -> CargoResult<()> {
    // Create a local anonymous remote in the repository to fetch the url

    with_authentication(url, &try!(repo.config()), |f| {
        let mut cb = git2::RemoteCallbacks::new();
        cb.credentials(f);
        let mut remote = try!(repo.remote_anonymous(&url));
        let mut opts = git2::FetchOptions::new();
        opts.remote_callbacks(cb)
            .download_tags(git2::AutotagOption::All);
        try!(remote.fetch(&[refspec], Some(&mut opts), None));
        Ok(())
    })
}
