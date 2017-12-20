use std::env;
use std::fmt;
use std::fs::{self, File};
use std::mem;
use std::path::{Path, PathBuf};
use std::process::Command;

use curl::easy::{Easy, List};
use git2::{self, ObjectType};
use serde::ser::{self, Serialize};
use url::Url;

use core::GitReference;
use util::{ToUrl, internal, Config, network, Progress};
use util::errors::{CargoResult, CargoResultExt, CargoError, Internal};

#[derive(PartialEq, Clone, Debug)]
pub struct GitRevision(git2::Oid);

impl ser::Serialize for GitRevision {
    fn serialize<S: ser::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        serialize_str(self, s)
    }
}

fn serialize_str<T, S>(t: &T, s: S) -> Result<S::Ok, S::Error>
    where T: fmt::Display,
          S: ser::Serializer,
{
    t.to_string().serialize(s)
}

impl fmt::Display for GitRevision {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

pub struct GitShortID(git2::Buf);

impl GitShortID {
    pub fn as_str(&self) -> &str {
        self.0.as_str().unwrap()
    }
}

/// `GitRemote` represents a remote repository. It gets cloned into a local
/// `GitDatabase`.
#[derive(PartialEq, Clone, Debug, Serialize)]
pub struct GitRemote {
    #[serde(serialize_with = "serialize_str")]
    url: Url,
}

/// `GitDatabase` is a local clone of a remote repository's database. Multiple
/// `GitCheckouts` can be cloned from this `GitDatabase`.
#[derive(Serialize)]
pub struct GitDatabase {
    remote: GitRemote,
    path: PathBuf,
    #[serde(skip_serializing)]
    repo: git2::Repository,
}

/// `GitCheckout` is a local checkout of a particular revision. Calling
/// `clone_into` with a reference will resolve the reference into a revision,
/// and return a `CargoError` if no revision for that reference was found.
#[derive(Serialize)]
pub struct GitCheckout<'a> {
    database: &'a GitDatabase,
    location: PathBuf,
    revision: GitRevision,
    #[serde(skip_serializing)]
    repo: git2::Repository,
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
        let db = self.db_at(path)?;
        db.rev_for(reference)
    }

    pub fn checkout(&self, into: &Path, cargo_config: &Config) -> CargoResult<GitDatabase> {
        let repo = match git2::Repository::open(into) {
            Ok(mut repo) => {
                self.fetch_into(&mut repo, cargo_config).chain_err(|| {
                    format!("failed to fetch into {}", into.display())
                })?;
                repo
            }
            Err(..) => {
                self.clone_into(into, cargo_config).chain_err(|| {
                    format!("failed to clone into: {}", into.display())
                })?
            }
        };

        Ok(GitDatabase {
            remote: self.clone(),
            path: into.to_path_buf(),
            repo: repo,
        })
    }

    pub fn db_at(&self, db_path: &Path) -> CargoResult<GitDatabase> {
        let repo = git2::Repository::open(db_path)?;
        Ok(GitDatabase {
            remote: self.clone(),
            path: db_path.to_path_buf(),
            repo: repo,
        })
    }

    fn fetch_into(&self, dst: &mut git2::Repository, cargo_config: &Config) -> CargoResult<()> {
        // Create a local anonymous remote in the repository to fetch the url
        let refspec = "refs/heads/*:refs/heads/*";
        fetch(dst, &self.url, refspec, cargo_config)
    }

    fn clone_into(&self, dst: &Path, cargo_config: &Config) -> CargoResult<git2::Repository> {
        if fs::metadata(&dst).is_ok() {
            fs::remove_dir_all(dst)?;
        }
        fs::create_dir_all(dst)?;
        let mut repo = git2::Repository::init_bare(dst)?;
        fetch(&mut repo, &self.url, "refs/heads/*:refs/heads/*", cargo_config)?;
        Ok(repo)
    }
}

impl GitDatabase {
    pub fn copy_to(&self, rev: GitRevision, dest: &Path, cargo_config: &Config)
                   -> CargoResult<GitCheckout> {
        let checkout = match git2::Repository::open(dest) {
            Ok(repo) => {
                let mut checkout = GitCheckout::new(dest, self, rev, repo);
                if !checkout.is_fresh() {
                    checkout.fetch(cargo_config)?;
                    checkout.reset(cargo_config)?;
                    assert!(checkout.is_fresh());
                }
                checkout
            }
            Err(..) => GitCheckout::clone_into(dest, self, rev, cargo_config)?,
        };
        checkout.update_submodules(cargo_config)?;
        Ok(checkout)
    }

    pub fn rev_for(&self, reference: &GitReference) -> CargoResult<GitRevision> {
        let id = match *reference {
            GitReference::Tag(ref s) => {
                (|| -> CargoResult<git2::Oid> {
                    let refname = format!("refs/tags/{}", s);
                    let id = self.repo.refname_to_id(&refname)?;
                    let obj = self.repo.find_object(id, None)?;
                    let obj = obj.peel(ObjectType::Commit)?;
                    Ok(obj.id())
                })().chain_err(|| {
                    format!("failed to find tag `{}`", s)
                })?
            }
            GitReference::Branch(ref s) => {
                (|| {
                    let b = self.repo.find_branch(s, git2::BranchType::Local)?;
                    b.get().target().ok_or_else(|| {
                        format_err!("branch `{}` did not have a target", s)
                    })
                })().chain_err(|| {
                    format!("failed to find branch `{}`", s)
                })?
            }
            GitReference::Rev(ref s) => {
                let obj = self.repo.revparse_single(s)?;
                match obj.as_tag() {
                    Some(tag) => tag.target_id(),
                    None => obj.id(),
                }
            }
        };
        Ok(GitRevision(id))
    }

    pub fn to_short_id(&self, revision: GitRevision) -> CargoResult<GitShortID> {
        let obj = self.repo.find_object(revision.0, None)?;
        Ok(GitShortID(obj.short_id()?))
    }

    pub fn has_ref(&self, reference: &str) -> CargoResult<()> {
        self.repo.revparse_single(reference)?;
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

    fn clone_into(into: &Path,
                  database: &'a GitDatabase,
                  revision: GitRevision,
                  config: &Config)
                  -> CargoResult<GitCheckout<'a>>
    {
        let dirname = into.parent().unwrap();
        fs::create_dir_all(&dirname).chain_err(|| {
            format!("Couldn't mkdir {}", dirname.display())
        })?;
        if fs::metadata(&into).is_ok() {
            fs::remove_dir_all(into).chain_err(|| {
                format!("Couldn't rmdir {}", into.display())
            })?;
        }
        let repo = git2::Repository::init(into)?;
        let mut checkout = GitCheckout::new(into, database, revision, repo);
        checkout.fetch(config)?;
        checkout.reset(config)?;
        Ok(checkout)
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

    fn fetch(&mut self, cargo_config: &Config) -> CargoResult<()> {
        info!("fetch {}", self.repo.path().display());
        let url = self.database.path.to_url()?;
        let refspec = "refs/heads/*:refs/heads/*";
        fetch(&mut self.repo, &url, refspec, cargo_config)?;
        Ok(())
    }

    fn reset(&self, config: &Config) -> CargoResult<()> {
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
        let object = self.repo.find_object(self.revision.0, None)?;
        reset(&self.repo, &object, config)?;
        File::create(ok_file)?;
        Ok(())
    }

    fn update_submodules(&self, cargo_config: &Config) -> CargoResult<()> {
        return update_submodules(&self.repo, cargo_config);

        fn update_submodules(repo: &git2::Repository, cargo_config: &Config) -> CargoResult<()> {
            info!("update submodules for: {:?}", repo.workdir().unwrap());

            for mut child in repo.submodules()? {
                update_submodule(repo, &mut child, cargo_config)
                    .chain_err(|| {
                        format!("failed to update submodule `{}`",
                                child.name().unwrap_or(""))
                    })?;
            }
            Ok(())
        }

        fn update_submodule(parent: &git2::Repository,
                            child: &mut git2::Submodule,
                            cargo_config: &Config) -> CargoResult<()> {
            child.init(false)?;
            let url = child.url().ok_or_else(|| {
                internal("non-utf8 url for submodule")
            })?;

            // A submodule which is listed in .gitmodules but not actually
            // checked out will not have a head id, so we should ignore it.
            let head = match child.head_id() {
                Some(head) => head,
                None => return Ok(()),
            };

            // If the submodule hasn't been checked out yet, we need to
            // clone it. If it has been checked out and the head is the same
            // as the submodule's head, then we can skip an update and keep
            // recursing.
            let head_and_repo = child.open().and_then(|repo| {
                let target = repo.head()?.target();
                Ok((target, repo))
            });
            let mut repo = match head_and_repo {
                Ok((head, repo)) => {
                    if child.head_id() == head {
                        return update_submodules(&repo, cargo_config)
                    }
                    repo
                }
                Err(..) => {
                    let path = parent.workdir().unwrap().join(child.path());
                    let _ = fs::remove_dir_all(&path);
                    git2::Repository::init(&path)?
                }
            };

            // Fetch data from origin and reset to the head commit
            let refspec = "refs/heads/*:refs/heads/*";
            let url = url.to_url()?;
            fetch(&mut repo, &url, refspec, cargo_config).chain_err(|| {
                internal(format!("failed to fetch submodule `{}` from {}",
                                 child.name().unwrap_or(""), url))
            })?;

            let obj = repo.find_object(head, None)?;
            reset(&repo, &obj, cargo_config)?;
            update_submodules(&repo, cargo_config)
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

    let mut ssh_username_requested = false;
    let mut cred_helper_bad = None;
    let mut ssh_agent_attempts = Vec::new();
    let mut any_attempts = false;
    let mut tried_sshkey = false;

    let mut res = f(&mut |url, username, allowed| {
        any_attempts = true;
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
        // So if we're being called here then we know that (a) we're using ssh
        // authentication and (b) no username was specified in the URL that
        // we're trying to clone. We need to guess an appropriate username here,
        // but that may involve a few attempts. Unfortunately we can't switch
        // usernames during one authentication session with libgit2, so to
        // handle this we bail out of this authentication session after setting
        // the flag `ssh_username_requested`, and then we handle this below.
        if allowed.contains(git2::USERNAME) {
            debug_assert!(username.is_none());
            ssh_username_requested = true;
            return Err(git2::Error::from_str("gonna try usernames later"))
        }

        // An "SSH_KEY" authentication indicates that we need some sort of SSH
        // authentication. This can currently either come from the ssh-agent
        // process or from a raw in-memory SSH key. Cargo only supports using
        // ssh-agent currently.
        //
        // If we get called with this then the only way that should be possible
        // is if a username is specified in the URL itself (e.g. `username` is
        // Some), hence the unwrap() here. We try custom usernames down below.
        if allowed.contains(git2::SSH_KEY) && !tried_sshkey {
            // If ssh-agent authentication fails, libgit2 will keep
            // calling this callback asking for other authentication
            // methods to try. Make sure we only try ssh-agent once,
            // to avoid looping forever.
            tried_sshkey = true;
            let username = username.unwrap();
            debug_assert!(!ssh_username_requested);
            ssh_agent_attempts.push(username.to_string());
            return git2::Cred::ssh_key_from_agent(username)
        }

        // Sometimes libgit2 will ask for a username/password in plaintext. This
        // is where Cargo would have an interactive prompt if we supported it,
        // but we currently don't! Right now the only way we support fetching a
        // plaintext password is through the `credential.helper` support, so
        // fetch that here.
        if allowed.contains(git2::USER_PASS_PLAINTEXT) {
            let r = git2::Cred::credential_helper(cfg, url, username);
            cred_helper_bad = Some(r.is_err());
            return r
        }

        // I'm... not sure what the DEFAULT kind of authentication is, but seems
        // easy to support?
        if allowed.contains(git2::DEFAULT) {
            return git2::Cred::default()
        }

        // Whelp, we tried our best
        Err(git2::Error::from_str("no authentication available"))
    });

    // Ok, so if it looks like we're going to be doing ssh authentication, we
    // want to try a few different usernames as one wasn't specified in the URL
    // for us to use. In order, we'll try:
    //
    // * A credential helper's username for this URL, if available.
    // * This account's username.
    // * "git"
    //
    // We have to restart the authentication session each time (due to
    // constraints in libssh2 I guess? maybe this is inherent to ssh?), so we
    // call our callback, `f`, in a loop here.
    if ssh_username_requested {
        debug_assert!(res.is_err());
        let mut attempts = Vec::new();
        attempts.push("git".to_string());
        if let Ok(s) = env::var("USER").or_else(|_| env::var("USERNAME")) {
            attempts.push(s);
        }
        if let Some(ref s) = cred_helper.username {
            attempts.push(s.clone());
        }

        while let Some(s) = attempts.pop() {
            // We should get `USERNAME` first, where we just return our attempt,
            // and then after that we should get `SSH_KEY`. If the first attempt
            // fails we'll get called again, but we don't have another option so
            // we bail out.
            let mut attempts = 0;
            res = f(&mut |_url, username, allowed| {
                if allowed.contains(git2::USERNAME) {
                    return git2::Cred::username(&s);
                }
                if allowed.contains(git2::SSH_KEY) {
                    debug_assert_eq!(Some(&s[..]), username);
                    attempts += 1;
                    if attempts == 1 {
                        ssh_agent_attempts.push(s.to_string());
                        return git2::Cred::ssh_key_from_agent(&s)
                    }
                }
                Err(git2::Error::from_str("no authentication available"))
            });

            // If we made two attempts then that means:
            //
            // 1. A username was requested, we returned `s`.
            // 2. An ssh key was requested, we returned to look up `s` in the
            //    ssh agent.
            // 3. For whatever reason that lookup failed, so we were asked again
            //    for another mode of authentication.
            //
            // Essentially, if `attempts == 2` then in theory the only error was
            // that this username failed to authenticate (e.g. no other network
            // errors happened). Otherwise something else is funny so we bail
            // out.
            if attempts != 2 {
                break
            }
        }
    }

    if res.is_ok() || !any_attempts {
        return res.map_err(From::from)
    }

    // In the case of an authentication failure (where we tried something) then
    // we try to give a more helpful error message about precisely what we
    // tried.
    let res = res.map_err(Internal::new).chain_err(|| {
        let mut msg = "failed to authenticate when downloading \
                       repository".to_string();
        if !ssh_agent_attempts.is_empty() {
            let names = ssh_agent_attempts.iter()
                                          .map(|s| format!("`{}`", s))
                                          .collect::<Vec<_>>()
                                          .join(", ");
            msg.push_str(&format!("\nattempted ssh-agent authentication, but \
                                   none of the usernames {} succeeded", names));
        }
        if let Some(failed_cred_helper) = cred_helper_bad {
            if failed_cred_helper {
                msg.push_str("\nattempted to find username/password via \
                              git's `credential.helper` support, but failed");
            } else {
                msg.push_str("\nattempted to find username/password via \
                              `credential.helper`, but maybe the found \
                              credentials were incorrect");
            }
        }
        msg
    })?;
    Ok(res)
}

fn reset(repo: &git2::Repository,
         obj: &git2::Object,
         config: &Config) -> CargoResult<()> {
    let mut pb = Progress::new("Checkout", config);
    let mut opts = git2::build::CheckoutBuilder::new();
    opts.progress(|_, cur, max| {
        drop(pb.tick(cur, max));
    });
    repo.reset(obj, git2::ResetType::Hard, Some(&mut opts))?;
    Ok(())
}

pub fn fetch(repo: &mut git2::Repository,
             url: &Url,
             refspec: &str,
             config: &Config) -> CargoResult<()> {
    if !config.network_allowed() {
        bail!("attempting to update a git repository, but --frozen \
               was specified")
    }

    // If we're fetching from github, attempt github's special fast path for
    // testing if we've already got an up-to-date copy of the repository
    if url.host_str() == Some("github.com") {
        if let Ok(oid) = repo.refname_to_id("refs/remotes/origin/master") {
            let mut handle = config.http()?.borrow_mut();
            debug!("attempting github fast path for {}", url);
            if github_up_to_date(&mut handle, url, &oid) {
                return Ok(())
            } else {
                debug!("fast path failed, falling back to a git fetch");
            }
        }
    }

    // We reuse repositories quite a lot, so before we go through and update the
    // repo check to see if it's a little too old and could benefit from a gc.
    // In theory this shouldn't be too too expensive compared to the network
    // request we're about to issue.
    maybe_gc_repo(repo)?;

    debug!("doing a fetch for {}", url);
    let mut progress = Progress::new("Fetch", config);
    with_authentication(url.as_str(), &repo.config()?, |f| {
        let mut cb = git2::RemoteCallbacks::new();
        cb.credentials(f);

        cb.transfer_progress(|stats| {
            progress.tick(stats.indexed_objects(), stats.total_objects()).is_ok()
        });

        // Create a local anonymous remote in the repository to fetch the url
        let mut remote = repo.remote_anonymous(url.as_str())?;
        let mut opts = git2::FetchOptions::new();
        opts.remote_callbacks(cb)
            .download_tags(git2::AutotagOption::All);

        network::with_retry(config, || {
            debug!("initiating fetch of {} from {}", refspec, url);
            remote.fetch(&[refspec], Some(&mut opts), None)
                .map_err(CargoError::from)
        })?;
        Ok(())
    })
}

/// Cargo has a bunch of long-lived git repositories in its global cache and
/// some, like the index, are updated very frequently. Right now each update
/// creates a new "pack file" inside the git database, and over time this can
/// cause bad performance and bad current behavior in libgit2.
///
/// One pathological use case today is where libgit2 opens hundreds of file
/// descriptors, getting us dangerously close to blowing out the OS limits of
/// how many fds we can have open. This is detailed in #4403.
///
/// To try to combat this problem we attempt a `git gc` here. Note, though, that
/// we may not even have `git` installed on the system! As a result we
/// opportunistically try a `git gc` when the pack directory looks too big, and
/// failing that we just blow away the repository and start over.
fn maybe_gc_repo(repo: &mut git2::Repository) -> CargoResult<()> {
    // Here we arbitrarily declare that if you have more than 100 files in your
    // `pack` folder that we need to do a gc.
    let entries = match repo.path().join("objects/pack").read_dir() {
        Ok(e) => e.count(),
        Err(_) => {
            debug!("skipping gc as pack dir appears gone");
            return Ok(())
        }
    };
    let max = env::var("__CARGO_PACKFILE_LIMIT").ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(100);
    if entries < max {
        debug!("skipping gc as there's only {} pack files", entries);
        return Ok(())
    }

    // First up, try a literal `git gc` by shelling out to git. This is pretty
    // likely to fail though as we may not have `git` installed. Note that
    // libgit2 doesn't currently implement the gc operation, so there's no
    // equivalent there.
    match Command::new("git").arg("gc").current_dir(repo.path()).output() {
        Ok(out) => {
            debug!("git-gc status: {}\n\nstdout ---\n{}\nstderr ---\n{}",
                   out.status,
                   String::from_utf8_lossy(&out.stdout),
                   String::from_utf8_lossy(&out.stderr));
            if out.status.success() {
                let new = git2::Repository::open(repo.path())?;
                mem::replace(repo, new);
                return Ok(())
            }
        }
        Err(e) => debug!("git-gc failed to spawn: {}", e),
    }

    // Alright all else failed, let's start over.
    //
    // Here we want to drop the current repository object pointed to by `repo`,
    // so we initialize temporary repository in a sub-folder, blow away the
    // existing git folder, and then recreate the git repo. Finally we blow away
    // the `tmp` folder we allocated.
    let path = repo.path().to_path_buf();
    let tmp = path.join("tmp");
    mem::replace(repo, git2::Repository::init(&tmp)?);
    for entry in path.read_dir()? {
        let entry = entry?;
        if entry.file_name().to_str() == Some("tmp") {
            continue
        }
        let path = entry.path();
        drop(fs::remove_file(&path).or_else(|_| fs::remove_dir_all(&path)));
    }
    if repo.is_bare() {
        mem::replace(repo, git2::Repository::init_bare(path)?);
    } else {
        mem::replace(repo, git2::Repository::init(path)?);
    }
    fs::remove_dir_all(&tmp).chain_err(|| {
        format!("failed to remove {:?}", tmp)
    })?;
    Ok(())
}

/// Updating the index is done pretty regularly so we want it to be as fast as
/// possible. For registries hosted on github (like the crates.io index) there's
/// a fast path available to use [1] to tell us that there's no updates to be
/// made.
///
/// This function will attempt to hit that fast path and verify that the `oid`
/// is actually the current `master` branch of the repository. If `true` is
/// returned then no update needs to be performed, but if `false` is returned
/// then the standard update logic still needs to happen.
///
/// [1]: https://developer.github.com/v3/repos/commits/#get-the-sha-1-of-a-commit-reference
///
/// Note that this function should never cause an actual failure because it's
/// just a fast path. As a result all errors are ignored in this function and we
/// just return a `bool`. Any real errors will be reported through the normal
/// update path above.
fn github_up_to_date(handle: &mut Easy, url: &Url, oid: &git2::Oid) -> bool {
    macro_rules! try {
        ($e:expr) => (match $e {
            Some(e) => e,
            None => return false,
        })
    }

    // This expects github urls in the form `github.com/user/repo` and nothing
    // else
    let mut pieces = try!(url.path_segments());
    let username = try!(pieces.next());
    let repo = try!(pieces.next());
    if pieces.next().is_some() {
        return false
    }

    let url = format!("https://api.github.com/repos/{}/{}/commits/master",
                      username, repo);
    try!(handle.get(true).ok());
    try!(handle.url(&url).ok());
    try!(handle.useragent("cargo").ok());
    let mut headers = List::new();
    try!(headers.append("Accept: application/vnd.github.3.sha").ok());
    try!(headers.append(&format!("If-None-Match: \"{}\"", oid)).ok());
    try!(handle.http_headers(headers).ok());
    try!(handle.perform().ok());

    try!(handle.response_code().ok()) == 304
}
