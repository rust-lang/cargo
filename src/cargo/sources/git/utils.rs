//! Utilities for handling git repositories, mainly around
//! authentication/cloning.

use crate::core::GitReference;
use crate::util::errors::CargoResult;
use crate::util::{network, Config, IntoUrl, MetricsCounter, Progress};
use anyhow::{anyhow, Context as _};
use cargo_util::{paths, ProcessBuilder};
use curl::easy::List;
use git2::{self, ErrorClass, ObjectType};
use log::{debug, info};
use serde::ser;
use serde::Serialize;
use std::env;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};
use url::Url;

fn serialize_str<T, S>(t: &T, s: S) -> Result<S::Ok, S::Error>
where
    T: fmt::Display,
    S: ser::Serializer,
{
    s.collect_str(t)
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
/// and return an `anyhow::Error` if no revision for that reference was found.
#[derive(Serialize)]
pub struct GitCheckout<'a> {
    database: &'a GitDatabase,
    location: PathBuf,
    #[serde(serialize_with = "serialize_str")]
    revision: git2::Oid,
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

    pub fn rev_for(&self, path: &Path, reference: &GitReference) -> CargoResult<git2::Oid> {
        reference.resolve(&self.db_at(path)?.repo)
    }

    pub fn checkout(
        &self,
        into: &Path,
        db: Option<GitDatabase>,
        reference: &GitReference,
        locked_rev: Option<git2::Oid>,
        cargo_config: &Config,
    ) -> CargoResult<(GitDatabase, git2::Oid)> {
        // If we have a previous instance of `GitDatabase` then fetch into that
        // if we can. If that can successfully load our revision then we've
        // populated the database with the latest version of `reference`, so
        // return that database and the rev we resolve to.
        if let Some(mut db) = db {
            fetch(&mut db.repo, self.url.as_str(), reference, cargo_config)
                .context(format!("failed to fetch into: {}", into.display()))?;
            match locked_rev {
                Some(rev) => {
                    if db.contains(rev) {
                        return Ok((db, rev));
                    }
                }
                None => {
                    if let Ok(rev) = reference.resolve(&db.repo) {
                        return Ok((db, rev));
                    }
                }
            }
        }

        // Otherwise start from scratch to handle corrupt git repositories.
        // After our fetch (which is interpreted as a clone now) we do the same
        // resolution to figure out what we cloned.
        if into.exists() {
            paths::remove_dir_all(into)?;
        }
        paths::create_dir_all(into)?;
        let mut repo = init(into, true)?;
        fetch(&mut repo, self.url.as_str(), reference, cargo_config)
            .context(format!("failed to clone into: {}", into.display()))?;
        let rev = match locked_rev {
            Some(rev) => rev,
            None => reference.resolve(&repo)?,
        };

        Ok((
            GitDatabase {
                remote: self.clone(),
                path: into.to_path_buf(),
                repo,
            },
            rev,
        ))
    }

    pub fn db_at(&self, db_path: &Path) -> CargoResult<GitDatabase> {
        let repo = git2::Repository::open(db_path)?;
        Ok(GitDatabase {
            remote: self.clone(),
            path: db_path.to_path_buf(),
            repo,
        })
    }
}

impl GitDatabase {
    pub fn copy_to(
        &self,
        rev: git2::Oid,
        dest: &Path,
        cargo_config: &Config,
    ) -> CargoResult<GitCheckout<'_>> {
        let mut checkout = None;
        if let Ok(repo) = git2::Repository::open(dest) {
            let mut co = GitCheckout::new(dest, self, rev, repo);
            if !co.is_fresh() {
                // After a successful fetch operation the subsequent reset can
                // fail sometimes for corrupt repositories where the fetch
                // operation succeeds but the object isn't actually there in one
                // way or another. In these situations just skip the error and
                // try blowing away the whole repository and trying with a
                // clone.
                co.fetch(cargo_config)?;
                match co.reset(cargo_config) {
                    Ok(()) => {
                        assert!(co.is_fresh());
                        checkout = Some(co);
                    }
                    Err(e) => debug!("failed reset after fetch {:?}", e),
                }
            } else {
                checkout = Some(co);
            }
        };
        let checkout = match checkout {
            Some(c) => c,
            None => GitCheckout::clone_into(dest, self, rev, cargo_config)?,
        };
        checkout.update_submodules(cargo_config)?;
        Ok(checkout)
    }

    pub fn to_short_id(&self, revision: git2::Oid) -> CargoResult<GitShortID> {
        let obj = self.repo.find_object(revision, None)?;
        Ok(GitShortID(obj.short_id()?))
    }

    pub fn contains(&self, oid: git2::Oid) -> bool {
        self.repo.revparse_single(&oid.to_string()).is_ok()
    }

    pub fn resolve(&self, r: &GitReference) -> CargoResult<git2::Oid> {
        r.resolve(&self.repo)
    }
}

impl GitReference {
    pub fn resolve(&self, repo: &git2::Repository) -> CargoResult<git2::Oid> {
        let id = match self {
            // Note that we resolve the named tag here in sync with where it's
            // fetched into via `fetch` below.
            GitReference::Tag(s) => (|| -> CargoResult<git2::Oid> {
                let refname = format!("refs/remotes/origin/tags/{}", s);
                let id = repo.refname_to_id(&refname)?;
                let obj = repo.find_object(id, None)?;
                let obj = obj.peel(ObjectType::Commit)?;
                Ok(obj.id())
            })()
            .with_context(|| format!("failed to find tag `{}`", s))?,

            // Resolve the remote name since that's all we're configuring in
            // `fetch` below.
            GitReference::Branch(s) => {
                let name = format!("origin/{}", s);
                let b = repo
                    .find_branch(&name, git2::BranchType::Remote)
                    .with_context(|| format!("failed to find branch `{}`", s))?;
                b.get()
                    .target()
                    .ok_or_else(|| anyhow::format_err!("branch `{}` did not have a target", s))?
            }

            // We'll be using the HEAD commit
            GitReference::DefaultBranch => {
                let head_id = repo.refname_to_id("refs/remotes/origin/HEAD")?;
                let head = repo.find_object(head_id, None)?;
                head.peel(ObjectType::Commit)?.id()
            }

            GitReference::Rev(s) => {
                let obj = repo.revparse_single(s)?;
                match obj.as_tag() {
                    Some(tag) => tag.target_id(),
                    None => obj.id(),
                }
            }
        };
        Ok(id)
    }
}

impl<'a> GitCheckout<'a> {
    fn new(
        path: &Path,
        database: &'a GitDatabase,
        revision: git2::Oid,
        repo: git2::Repository,
    ) -> GitCheckout<'a> {
        GitCheckout {
            location: path.to_path_buf(),
            database,
            revision,
            repo,
        }
    }

    fn clone_into(
        into: &Path,
        database: &'a GitDatabase,
        revision: git2::Oid,
        config: &Config,
    ) -> CargoResult<GitCheckout<'a>> {
        let dirname = into.parent().unwrap();
        paths::create_dir_all(&dirname)?;
        if into.exists() {
            paths::remove_dir_all(into)?;
        }

        // we're doing a local filesystem-to-filesystem clone so there should
        // be no need to respect global configuration options, so pass in
        // an empty instance of `git2::Config` below.
        let git_config = git2::Config::new()?;

        // Clone the repository, but make sure we use the "local" option in
        // libgit2 which will attempt to use hardlinks to set up the database.
        // This should speed up the clone operation quite a bit if it works.
        //
        // Note that we still use the same fetch options because while we don't
        // need authentication information we may want progress bars and such.
        let url = database.path.into_url()?;
        let mut repo = None;
        with_fetch_options(&git_config, url.as_str(), config, &mut |fopts| {
            let mut checkout = git2::build::CheckoutBuilder::new();
            checkout.dry_run(); // we'll do this below during a `reset`

            let r = git2::build::RepoBuilder::new()
                // use hard links and/or copy the database, we're doing a
                // filesystem clone so this'll speed things up quite a bit.
                .clone_local(git2::build::CloneLocal::Local)
                .with_checkout(checkout)
                .fetch_options(fopts)
                .clone(url.as_str(), into)?;
            repo = Some(r);
            Ok(())
        })?;
        let repo = repo.unwrap();

        let checkout = GitCheckout::new(into, database, revision, repo);
        checkout.reset(config)?;
        Ok(checkout)
    }

    fn is_fresh(&self) -> bool {
        match self.repo.revparse_single("HEAD") {
            Ok(ref head) if head.id() == self.revision => {
                // See comments in reset() for why we check this
                self.location.join(".cargo-ok").exists()
            }
            _ => false,
        }
    }

    fn fetch(&mut self, cargo_config: &Config) -> CargoResult<()> {
        info!("fetch {}", self.repo.path().display());
        let url = self.database.path.into_url()?;
        let reference = GitReference::Rev(self.revision.to_string());
        fetch(&mut self.repo, url.as_str(), &reference, cargo_config)?;
        Ok(())
    }

    fn reset(&self, config: &Config) -> CargoResult<()> {
        // If we're interrupted while performing this reset (e.g., we die because
        // of a signal) Cargo needs to be sure to try to check out this repo
        // again on the next go-round.
        //
        // To enable this we have a dummy file in our checkout, .cargo-ok, which
        // if present means that the repo has been successfully reset and is
        // ready to go. Hence if we start to do a reset, we make sure this file
        // *doesn't* exist, and then once we're done we create the file.
        let ok_file = self.location.join(".cargo-ok");
        let _ = paths::remove_file(&ok_file);
        info!("reset {} to {}", self.repo.path().display(), self.revision);

        // Ensure libgit2 won't mess with newlines when we vendor.
        if let Ok(mut git_config) = self.repo.config() {
            git_config.set_bool("core.autocrlf", false)?;
        }

        let object = self.repo.find_object(self.revision, None)?;
        reset(&self.repo, &object, config)?;
        paths::create(ok_file)?;
        Ok(())
    }

    fn update_submodules(&self, cargo_config: &Config) -> CargoResult<()> {
        return update_submodules(&self.repo, cargo_config);

        fn update_submodules(repo: &git2::Repository, cargo_config: &Config) -> CargoResult<()> {
            debug!("update submodules for: {:?}", repo.workdir().unwrap());

            for mut child in repo.submodules()? {
                update_submodule(repo, &mut child, cargo_config).with_context(|| {
                    format!(
                        "failed to update submodule `{}`",
                        child.name().unwrap_or("")
                    )
                })?;
            }
            Ok(())
        }

        fn update_submodule(
            parent: &git2::Repository,
            child: &mut git2::Submodule<'_>,
            cargo_config: &Config,
        ) -> CargoResult<()> {
            child.init(false)?;
            let url = child.url().ok_or_else(|| {
                anyhow::format_err!("non-utf8 url for submodule {:?}?", child.path())
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
                        return update_submodules(&repo, cargo_config);
                    }
                    repo
                }
                Err(..) => {
                    let path = parent.workdir().unwrap().join(child.path());
                    let _ = paths::remove_dir_all(&path);
                    init(&path, false)?
                }
            };
            // Fetch data from origin and reset to the head commit
            let reference = GitReference::Rev(head.to_string());
            cargo_config
                .shell()
                .status("Updating", format!("git submodule `{}`", url))?;
            fetch(&mut repo, url, &reference, cargo_config).with_context(|| {
                format!(
                    "failed to fetch submodule `{}` from {}",
                    child.name().unwrap_or(""),
                    url
                )
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
///   with `credential.helper` in git, and is the interface for the macOS
///   keychain, for example.
///
/// * After the above two have failed, we just kinda grapple attempting to
///   return *something*.
///
/// If any form of authentication fails, libgit2 will repeatedly ask us for
/// credentials until we give it a reason to not do so. To ensure we don't
/// just sit here looping forever we keep track of authentications we've
/// attempted and we don't try the same ones again.
fn with_authentication<T, F>(url: &str, cfg: &git2::Config, mut f: F) -> CargoResult<T>
where
    F: FnMut(&mut git2::Credentials<'_>) -> CargoResult<T>,
{
    let mut cred_helper = git2::CredentialHelper::new(url);
    cred_helper.config(cfg);

    let mut ssh_username_requested = false;
    let mut cred_helper_bad = None;
    let mut ssh_agent_attempts = Vec::new();
    let mut any_attempts = false;
    let mut tried_sshkey = false;
    let mut url_attempt = None;

    let orig_url = url;
    let mut res = f(&mut |url, username, allowed| {
        any_attempts = true;
        if url != orig_url {
            url_attempt = Some(url.to_string());
        }
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
        if allowed.contains(git2::CredentialType::USERNAME) {
            debug_assert!(username.is_none());
            ssh_username_requested = true;
            return Err(git2::Error::from_str("gonna try usernames later"));
        }

        // An "SSH_KEY" authentication indicates that we need some sort of SSH
        // authentication. This can currently either come from the ssh-agent
        // process or from a raw in-memory SSH key. Cargo only supports using
        // ssh-agent currently.
        //
        // If we get called with this then the only way that should be possible
        // is if a username is specified in the URL itself (e.g., `username` is
        // Some), hence the unwrap() here. We try custom usernames down below.
        if allowed.contains(git2::CredentialType::SSH_KEY) && !tried_sshkey {
            // If ssh-agent authentication fails, libgit2 will keep
            // calling this callback asking for other authentication
            // methods to try. Make sure we only try ssh-agent once,
            // to avoid looping forever.
            tried_sshkey = true;
            let username = username.unwrap();
            debug_assert!(!ssh_username_requested);
            ssh_agent_attempts.push(username.to_string());
            return git2::Cred::ssh_key_from_agent(username);
        }

        // Sometimes libgit2 will ask for a username/password in plaintext. This
        // is where Cargo would have an interactive prompt if we supported it,
        // but we currently don't! Right now the only way we support fetching a
        // plaintext password is through the `credential.helper` support, so
        // fetch that here.
        //
        // If ssh-agent authentication fails, libgit2 will keep calling this
        // callback asking for other authentication methods to try. Check
        // cred_helper_bad to make sure we only try the git credentail helper
        // once, to avoid looping forever.
        if allowed.contains(git2::CredentialType::USER_PASS_PLAINTEXT) && cred_helper_bad.is_none()
        {
            let r = git2::Cred::credential_helper(cfg, url, username);
            cred_helper_bad = Some(r.is_err());
            return r;
        }

        // I'm... not sure what the DEFAULT kind of authentication is, but seems
        // easy to support?
        if allowed.contains(git2::CredentialType::DEFAULT) {
            return git2::Cred::default();
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
        let mut attempts = vec![String::from("git")];
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
                if allowed.contains(git2::CredentialType::USERNAME) {
                    return git2::Cred::username(&s);
                }
                if allowed.contains(git2::CredentialType::SSH_KEY) {
                    debug_assert_eq!(Some(&s[..]), username);
                    attempts += 1;
                    if attempts == 1 {
                        ssh_agent_attempts.push(s.to_string());
                        return git2::Cred::ssh_key_from_agent(&s);
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
            // that this username failed to authenticate (e.g., no other network
            // errors happened). Otherwise something else is funny so we bail
            // out.
            if attempts != 2 {
                break;
            }
        }
    }
    let mut err = match res {
        Ok(e) => return Ok(e),
        Err(e) => e,
    };

    // In the case of an authentication failure (where we tried something) then
    // we try to give a more helpful error message about precisely what we
    // tried.
    if any_attempts {
        let mut msg = "failed to authenticate when downloading \
                       repository"
            .to_string();

        if let Some(attempt) = &url_attempt {
            if url != attempt {
                msg.push_str(": ");
                msg.push_str(attempt);
            }
        }
        msg.push('\n');
        if !ssh_agent_attempts.is_empty() {
            let names = ssh_agent_attempts
                .iter()
                .map(|s| format!("`{}`", s))
                .collect::<Vec<_>>()
                .join(", ");
            msg.push_str(&format!(
                "\n* attempted ssh-agent authentication, but \
                 no usernames succeeded: {}",
                names
            ));
        }
        if let Some(failed_cred_helper) = cred_helper_bad {
            if failed_cred_helper {
                msg.push_str(
                    "\n* attempted to find username/password via \
                     git's `credential.helper` support, but failed",
                );
            } else {
                msg.push_str(
                    "\n* attempted to find username/password via \
                     `credential.helper`, but maybe the found \
                     credentials were incorrect",
                );
            }
        }
        msg.push_str("\n\n");
        msg.push_str("if the git CLI succeeds then `net.git-fetch-with-cli` may help here\n");
        msg.push_str("https://doc.rust-lang.org/cargo/reference/config.html#netgit-fetch-with-cli");
        err = err.context(msg);

    // Otherwise if we didn't even get to the authentication phase them we may
    // have failed to set up a connection, in these cases hint on the
    // `net.git-fetch-with-cli` configuration option.
    } else if let Some(e) = err.downcast_ref::<git2::Error>() {
        match e.class() {
            ErrorClass::Net
            | ErrorClass::Ssl
            | ErrorClass::Submodule
            | ErrorClass::FetchHead
            | ErrorClass::Ssh
            | ErrorClass::Callback
            | ErrorClass::Http => {
                let mut msg = "network failure seems to have happened\n".to_string();
                msg.push_str(
                    "if a proxy or similar is necessary `net.git-fetch-with-cli` may help here\n",
                );
                msg.push_str(
                    "https://doc.rust-lang.org/cargo/reference/config.html#netgit-fetch-with-cli",
                );
                err = err.context(msg);
            }
            _ => {}
        }
    }

    Err(err)
}

fn reset(repo: &git2::Repository, obj: &git2::Object<'_>, config: &Config) -> CargoResult<()> {
    let mut pb = Progress::new("Checkout", config);
    let mut opts = git2::build::CheckoutBuilder::new();
    opts.progress(|_, cur, max| {
        drop(pb.tick(cur, max, ""));
    });
    debug!("doing reset");
    repo.reset(obj, git2::ResetType::Hard, Some(&mut opts))?;
    debug!("reset done");
    Ok(())
}

pub fn with_fetch_options(
    git_config: &git2::Config,
    url: &str,
    config: &Config,
    cb: &mut dyn FnMut(git2::FetchOptions<'_>) -> CargoResult<()>,
) -> CargoResult<()> {
    let mut progress = Progress::new("Fetch", config);
    network::with_retry(config, || {
        with_authentication(url, git_config, |f| {
            let mut last_update = Instant::now();
            let mut rcb = git2::RemoteCallbacks::new();
            // We choose `N=10` here to make a `300ms * 10slots ~= 3000ms`
            // sliding window for tracking the data transfer rate (in bytes/s).
            let mut counter = MetricsCounter::<10>::new(0, last_update);
            rcb.credentials(f);
            rcb.transfer_progress(|stats| {
                let indexed_deltas = stats.indexed_deltas();
                let msg = if indexed_deltas > 0 {
                    // Resolving deltas.
                    format!(
                        ", ({}/{}) resolving deltas",
                        indexed_deltas,
                        stats.total_deltas()
                    )
                } else {
                    // Receiving objects.
                    //
                    // # Caveat
                    //
                    // Progress bar relies on git2 calling `transfer_progress`
                    // to update its transfer rate, but we cannot guarantee a
                    // periodic call of that callback. Thus if we don't receive
                    // any data for, say, 10 seconds, the rate will get stuck
                    // and never go down to 0B/s.
                    // In the future, we need to find away to update the rate
                    // even when the callback is not called.
                    let now = Instant::now();
                    // Scrape a `received_bytes` to the counter every 300ms.
                    if now - last_update > Duration::from_millis(300) {
                        counter.add(stats.received_bytes(), now);
                        last_update = now;
                    }
                    fn format_bytes(bytes: f32) -> (&'static str, f32) {
                        static UNITS: [&str; 5] = ["", "Ki", "Mi", "Gi", "Ti"];
                        let i = (bytes.log2() / 10.0).min(4.0) as usize;
                        (UNITS[i], bytes / 1024_f32.powi(i as i32))
                    }
                    let (unit, rate) = format_bytes(counter.rate());
                    format!(", {:.2}{}B/s", rate, unit)
                };
                progress
                    .tick(stats.indexed_objects(), stats.total_objects(), &msg)
                    .is_ok()
            });

            // Create a local anonymous remote in the repository to fetch the
            // url
            let mut opts = git2::FetchOptions::new();
            opts.remote_callbacks(rcb);
            cb(opts)
        })?;
        Ok(())
    })
}

pub fn fetch(
    repo: &mut git2::Repository,
    url: &str,
    reference: &GitReference,
    config: &Config,
) -> CargoResult<()> {
    if config.frozen() {
        anyhow::bail!(
            "attempting to update a git repository, but --frozen \
             was specified"
        )
    }
    if !config.network_allowed() {
        anyhow::bail!("can't update a git repository in the offline mode")
    }

    // If we're fetching from GitHub, attempt GitHub's special fast path for
    // testing if we've already got an up-to-date copy of the repository
    match github_up_to_date(repo, url, reference, config) {
        Ok(true) => return Ok(()),
        Ok(false) => {}
        Err(e) => debug!("failed to check github {:?}", e),
    }

    // We reuse repositories quite a lot, so before we go through and update the
    // repo check to see if it's a little too old and could benefit from a gc.
    // In theory this shouldn't be too too expensive compared to the network
    // request we're about to issue.
    maybe_gc_repo(repo)?;

    // Translate the reference desired here into an actual list of refspecs
    // which need to get fetched. Additionally record if we're fetching tags.
    let mut refspecs = Vec::new();
    let mut tags = false;
    // The `+` symbol on the refspec means to allow a forced (fast-forward)
    // update which is needed if there is ever a force push that requires a
    // fast-forward.
    match reference {
        // For branches and tags we can fetch simply one reference and copy it
        // locally, no need to fetch other branches/tags.
        GitReference::Branch(b) => {
            refspecs.push(format!("+refs/heads/{0}:refs/remotes/origin/{0}", b));
        }
        GitReference::Tag(t) => {
            refspecs.push(format!("+refs/tags/{0}:refs/remotes/origin/tags/{0}", t));
        }

        GitReference::DefaultBranch => {
            refspecs.push(String::from("+HEAD:refs/remotes/origin/HEAD"));
        }

        GitReference::Rev(rev) => {
            if rev.starts_with("refs/") {
                refspecs.push(format!("+{0}:{0}", rev));
            } else {
                // We don't know what the rev will point to. To handle this
                // situation we fetch all branches and tags, and then we pray
                // it's somewhere in there.
                refspecs.push(String::from("+refs/heads/*:refs/remotes/origin/*"));
                refspecs.push(String::from("+HEAD:refs/remotes/origin/HEAD"));
                tags = true;
            }
        }
    }

    // Unfortunately `libgit2` is notably lacking in the realm of authentication
    // when compared to the `git` command line. As a result, allow an escape
    // hatch for users that would prefer to use `git`-the-CLI for fetching
    // repositories instead of `libgit2`-the-library. This should make more
    // flavors of authentication possible while also still giving us all the
    // speed and portability of using `libgit2`.
    if let Some(true) = config.net_config()?.git_fetch_with_cli {
        return fetch_with_cli(repo, url, &refspecs, tags, config);
    }

    debug!("doing a fetch for {}", url);
    let git_config = git2::Config::open_default()?;
    with_fetch_options(&git_config, url, config, &mut |mut opts| {
        if tags {
            opts.download_tags(git2::AutotagOption::All);
        }
        // The `fetch` operation here may fail spuriously due to a corrupt
        // repository. It could also fail, however, for a whole slew of other
        // reasons (aka network related reasons). We want Cargo to automatically
        // recover from corrupt repositories, but we don't want Cargo to stomp
        // over other legitimate errors.
        //
        // Consequently we save off the error of the `fetch` operation and if it
        // looks like a "corrupt repo" error then we blow away the repo and try
        // again. If it looks like any other kind of error, or if we've already
        // blown away the repository, then we want to return the error as-is.
        let mut repo_reinitialized = false;
        loop {
            debug!("initiating fetch of {:?} from {}", refspecs, url);
            let res = repo
                .remote_anonymous(url)?
                .fetch(&refspecs, Some(&mut opts), None);
            let err = match res {
                Ok(()) => break,
                Err(e) => e,
            };
            debug!("fetch failed: {}", err);

            if !repo_reinitialized && matches!(err.class(), ErrorClass::Reference | ErrorClass::Odb)
            {
                repo_reinitialized = true;
                debug!(
                    "looks like this is a corrupt repository, reinitializing \
                     and trying again"
                );
                if reinitialize(repo).is_ok() {
                    continue;
                }
            }

            return Err(err.into());
        }
        Ok(())
    })
}

fn fetch_with_cli(
    repo: &mut git2::Repository,
    url: &str,
    refspecs: &[String],
    tags: bool,
    config: &Config,
) -> CargoResult<()> {
    let mut cmd = ProcessBuilder::new("git");
    cmd.arg("fetch");
    if tags {
        cmd.arg("--tags");
    }
    cmd.arg("--force") // handle force pushes
        .arg("--update-head-ok") // see discussion in #2078
        .arg(url)
        .args(refspecs)
        // If cargo is run by git (for example, the `exec` command in `git
        // rebase`), the GIT_DIR is set by git and will point to the wrong
        // location (this takes precedence over the cwd). Make sure this is
        // unset so git will look at cwd for the repo.
        .env_remove("GIT_DIR")
        // The reset of these may not be necessary, but I'm including them
        // just to be extra paranoid and avoid any issues.
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_OBJECT_DIRECTORY")
        .env_remove("GIT_ALTERNATE_OBJECT_DIRECTORIES")
        .cwd(repo.path());
    config
        .shell()
        .verbose(|s| s.status("Running", &cmd.to_string()))?;
    cmd.exec_with_output()?;
    Ok(())
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
            return Ok(());
        }
    };
    let max = env::var("__CARGO_PACKFILE_LIMIT")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(100);
    if entries < max {
        debug!("skipping gc as there's only {} pack files", entries);
        return Ok(());
    }

    // First up, try a literal `git gc` by shelling out to git. This is pretty
    // likely to fail though as we may not have `git` installed. Note that
    // libgit2 doesn't currently implement the gc operation, so there's no
    // equivalent there.
    match Command::new("git")
        .arg("gc")
        .current_dir(repo.path())
        .output()
    {
        Ok(out) => {
            debug!(
                "git-gc status: {}\n\nstdout ---\n{}\nstderr ---\n{}",
                out.status,
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
            if out.status.success() {
                let new = git2::Repository::open(repo.path())?;
                *repo = new;
                return Ok(());
            }
        }
        Err(e) => debug!("git-gc failed to spawn: {}", e),
    }

    // Alright all else failed, let's start over.
    reinitialize(repo)
}

fn reinitialize(repo: &mut git2::Repository) -> CargoResult<()> {
    // Here we want to drop the current repository object pointed to by `repo`,
    // so we initialize temporary repository in a sub-folder, blow away the
    // existing git folder, and then recreate the git repo. Finally we blow away
    // the `tmp` folder we allocated.
    let path = repo.path().to_path_buf();
    debug!("reinitializing git repo at {:?}", path);
    let tmp = path.join("tmp");
    let bare = !repo.path().ends_with(".git");
    *repo = init(&tmp, false)?;
    for entry in path.read_dir()? {
        let entry = entry?;
        if entry.file_name().to_str() == Some("tmp") {
            continue;
        }
        let path = entry.path();
        drop(paths::remove_file(&path).or_else(|_| paths::remove_dir_all(&path)));
    }
    *repo = init(&path, bare)?;
    paths::remove_dir_all(&tmp)?;
    Ok(())
}

fn init(path: &Path, bare: bool) -> CargoResult<git2::Repository> {
    let mut opts = git2::RepositoryInitOptions::new();
    // Skip anything related to templates, they just call all sorts of issues as
    // we really don't want to use them yet they insist on being used. See #6240
    // for an example issue that comes up.
    opts.external_template(false);
    opts.bare(bare);
    Ok(git2::Repository::init_opts(&path, &opts)?)
}

/// Updating the index is done pretty regularly so we want it to be as fast as
/// possible. For registries hosted on GitHub (like the crates.io index) there's
/// a fast path available to use [1] to tell us that there's no updates to be
/// made.
///
/// This function will attempt to hit that fast path and verify that the `oid`
/// is actually the current branch of the repository. If `true` is returned then
/// no update needs to be performed, but if `false` is returned then the
/// standard update logic still needs to happen.
///
/// [1]: https://developer.github.com/v3/repos/commits/#get-the-sha-1-of-a-commit-reference
///
/// Note that this function should never cause an actual failure because it's
/// just a fast path. As a result all errors are ignored in this function and we
/// just return a `bool`. Any real errors will be reported through the normal
/// update path above.
fn github_up_to_date(
    repo: &mut git2::Repository,
    url: &str,
    reference: &GitReference,
    config: &Config,
) -> CargoResult<bool> {
    let url = Url::parse(url)?;
    if url.host_str() != Some("github.com") {
        return Ok(false);
    }

    let github_branch_name = match reference {
        GitReference::Branch(branch) => branch,
        GitReference::Tag(tag) => tag,
        GitReference::DefaultBranch => "HEAD",
        GitReference::Rev(rev) => {
            if rev.starts_with("refs/") {
                rev
            } else {
                debug!("can't use github fast path with `rev = \"{}\"`", rev);
                return Ok(false);
            }
        }
    };

    // This expects GitHub urls in the form `github.com/user/repo` and nothing
    // else
    let mut pieces = url
        .path_segments()
        .ok_or_else(|| anyhow!("no path segments on url"))?;
    let username = pieces
        .next()
        .ok_or_else(|| anyhow!("couldn't find username"))?;
    let repository = pieces
        .next()
        .ok_or_else(|| anyhow!("couldn't find repository name"))?;
    if pieces.next().is_some() {
        anyhow::bail!("too many segments on URL");
    }

    // Trim off the `.git` from the repository, if present, since that's
    // optional for GitHub and won't work when we try to use the API as well.
    let repository = repository.strip_suffix(".git").unwrap_or(repository);

    let url = format!(
        "https://api.github.com/repos/{}/{}/commits/{}",
        username, repository, github_branch_name,
    );
    let mut handle = config.http()?.borrow_mut();
    debug!("attempting GitHub fast path for {}", url);
    handle.get(true)?;
    handle.url(&url)?;
    handle.useragent("cargo")?;
    let mut headers = List::new();
    headers.append("Accept: application/vnd.github.3.sha")?;
    headers.append(&format!("If-None-Match: \"{}\"", reference.resolve(repo)?))?;
    handle.http_headers(headers)?;
    handle.perform()?;
    Ok(handle.response_code()? == 304)
}
