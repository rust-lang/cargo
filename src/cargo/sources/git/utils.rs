//! Utilities for handling git repositories, mainly around
//! authentication/cloning.

use crate::core::{GitReference, Verbosity};
use crate::sources::git::fetch::RemoteKind;
use crate::sources::git::oxide;
use crate::sources::git::oxide::cargo_config_to_gitoxide_overrides;
use crate::util::HumanBytes;
use crate::util::errors::CargoResult;
use crate::util::{GlobalContext, IntoUrl, MetricsCounter, Progress, network};
use anyhow::{Context as _, anyhow};
use cargo_util::{ProcessBuilder, paths};
use curl::easy::List;
use git2::{ErrorClass, ObjectType, Oid};
use serde::Serialize;
use serde::ser;
use std::borrow::Cow;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tracing::{debug, info};
use url::Url;

/// A file indicates that if present, `git reset` has been done and a repo
/// checkout is ready to go. See [`GitCheckout::reset`] for why we need this.
const CHECKOUT_READY_LOCK: &str = ".cargo-ok";

fn serialize_str<T, S>(t: &T, s: S) -> Result<S::Ok, S::Error>
where
    T: fmt::Display,
    S: ser::Serializer,
{
    s.collect_str(t)
}

/// A short abbreviated OID.
///
/// Exists for avoiding extra allocations in [`GitDatabase::to_short_id`].
pub struct GitShortID(git2::Buf);

impl GitShortID {
    /// Views the short ID as a `str`.
    pub fn as_str(&self) -> &str {
        self.0.as_str().unwrap()
    }
}

/// A remote repository. It gets cloned into a local [`GitDatabase`].
#[derive(PartialEq, Clone, Debug, Serialize)]
pub struct GitRemote {
    /// URL to a remote repository.
    #[serde(serialize_with = "serialize_str")]
    url: Url,
}

/// A local clone of a remote repository's database. Multiple [`GitCheckout`]s
/// can be cloned from a single [`GitDatabase`].
pub struct GitDatabase {
    /// The remote repository where this database is fetched from.
    remote: GitRemote,
    /// Path to the root of the underlying Git repository on the local filesystem.
    path: PathBuf,
    /// Underlying Git repository instance for this database.
    repo: git2::Repository,
}

/// A local checkout of a particular revision from a [`GitDatabase`].
pub struct GitCheckout<'a> {
    /// The git database where this checkout is cloned from.
    database: &'a GitDatabase,
    /// Path to the root of the underlying Git repository on the local filesystem.
    path: PathBuf,
    /// The git revision this checkout is for.
    revision: git2::Oid,
    /// Underlying Git repository instance for this checkout.
    repo: git2::Repository,
}

impl GitRemote {
    /// Creates an instance for a remote repository URL.
    pub fn new(url: &Url) -> GitRemote {
        GitRemote { url: url.clone() }
    }

    /// Gets the remote repository URL.
    pub fn url(&self) -> &Url {
        &self.url
    }

    /// Fetches and checkouts to a reference or a revision from this remote
    /// into a local path.
    ///
    /// This ensures that it gets the up-to-date commit when a named reference
    /// is given (tag, branch, refs/*). Thus, network connection is involved.
    ///
    /// If we have a previous instance of [`GitDatabase`] then fetch into that
    /// if we can. If that can successfully load our revision then we've
    /// populated the database with the latest version of `reference`, so
    /// return that database and the rev we resolve to.
    pub fn checkout(
        &self,
        into: &Path,
        db: Option<GitDatabase>,
        reference: &GitReference,
        gctx: &GlobalContext,
    ) -> CargoResult<(GitDatabase, git2::Oid)> {
        if let Some(mut db) = db {
            fetch(
                &mut db.repo,
                self.url.as_str(),
                reference,
                gctx,
                RemoteKind::GitDependency,
            )
            .with_context(|| format!("failed to fetch into: {}", into.display()))?;

            if let Some(rev) = resolve_ref(reference, &db.repo).ok() {
                return Ok((db, rev));
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
        fetch(
            &mut repo,
            self.url.as_str(),
            reference,
            gctx,
            RemoteKind::GitDependency,
        )
        .with_context(|| format!("failed to clone into: {}", into.display()))?;
        let rev = resolve_ref(reference, &repo)?;

        Ok((
            GitDatabase {
                remote: self.clone(),
                path: into.to_path_buf(),
                repo,
            },
            rev,
        ))
    }

    /// Creates a [`GitDatabase`] of this remote at `db_path`.
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
    /// Checkouts to a revision at `dest`ination from this database.
    #[tracing::instrument(skip(self, gctx))]
    pub fn copy_to(
        &self,
        rev: git2::Oid,
        dest: &Path,
        gctx: &GlobalContext,
    ) -> CargoResult<GitCheckout<'_>> {
        // If the existing checkout exists, and it is fresh, use it.
        // A non-fresh checkout can happen if the checkout operation was
        // interrupted. In that case, the checkout gets deleted and a new
        // clone is created.
        let checkout = match git2::Repository::open(dest)
            .ok()
            .map(|repo| GitCheckout::new(self, rev, repo))
            .filter(|co| co.is_fresh())
        {
            Some(co) => co,
            None => {
                let (checkout, guard) = GitCheckout::clone_into(dest, self, rev, gctx)?;
                checkout.update_submodules(gctx)?;
                guard.mark_ok()?;
                checkout
            }
        };

        Ok(checkout)
    }

    /// Get a short OID for a `revision`, usually 7 chars or more if ambiguous.
    pub fn to_short_id(&self, revision: git2::Oid) -> CargoResult<GitShortID> {
        let obj = self.repo.find_object(revision, None)?;
        Ok(GitShortID(obj.short_id()?))
    }

    /// Checks if the database contains the object of this `oid`..
    pub fn contains(&self, oid: git2::Oid) -> bool {
        self.repo.revparse_single(&oid.to_string()).is_ok()
    }

    /// [`resolve_ref`]s this reference with this database.
    pub fn resolve(&self, r: &GitReference) -> CargoResult<git2::Oid> {
        resolve_ref(r, &self.repo)
    }
}

/// Resolves [`GitReference`] to an object ID with objects the `repo` currently has.
pub fn resolve_ref(gitref: &GitReference, repo: &git2::Repository) -> CargoResult<git2::Oid> {
    let id = match gitref {
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

impl<'a> GitCheckout<'a> {
    /// Creates an instance of [`GitCheckout`]. This doesn't imply the checkout
    /// is done. Use [`GitCheckout::is_fresh`] to check.
    ///
    /// * The `database` is where this checkout is from.
    /// * The `repo` will be the checked out Git repository.
    fn new(
        database: &'a GitDatabase,
        revision: git2::Oid,
        repo: git2::Repository,
    ) -> GitCheckout<'a> {
        let path = repo.workdir().unwrap_or_else(|| repo.path());
        GitCheckout {
            path: path.to_path_buf(),
            database,
            revision,
            repo,
        }
    }

    /// Gets the remote repository URL.
    fn remote_url(&self) -> &Url {
        &self.database.remote.url()
    }

    /// Clone a repo for a `revision` into a local path from a `datatabase`.
    /// This is a filesystem-to-filesystem clone.
    fn clone_into(
        into: &Path,
        database: &'a GitDatabase,
        revision: git2::Oid,
        gctx: &GlobalContext,
    ) -> CargoResult<(GitCheckout<'a>, CheckoutGuard)> {
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
        with_fetch_options(&git_config, url.as_str(), gctx, &mut |fopts| {
            let mut checkout = git2::build::CheckoutBuilder::new();
            checkout.dry_run(); // we'll do this below during a `reset`

            let r = git2::build::RepoBuilder::new()
                // use hard links and/or copy the database, we're doing a
                // filesystem clone so this'll speed things up quite a bit.
                .clone_local(git2::build::CloneLocal::Local)
                .with_checkout(checkout)
                .fetch_options(fopts)
                .clone(url.as_str(), into)?;
            // `git2` doesn't seem to handle shallow repos correctly when doing
            // a local clone. Fortunately all that's needed is the copy of the
            // one file that defines the shallow boundary, the commits which
            // have their parents omitted as part of the shallow clone.
            //
            // TODO(git2): remove this when git2 supports shallow clone correctly
            if database.repo.is_shallow() {
                std::fs::copy(
                    database.repo.path().join("shallow"),
                    r.path().join("shallow"),
                )?;
            }
            repo = Some(r);
            Ok(())
        })?;
        let repo = repo.unwrap();

        let checkout = GitCheckout::new(database, revision, repo);
        let guard = checkout.reset(gctx)?;
        Ok((checkout, guard))
    }

    /// Checks if the `HEAD` of this checkout points to the expected revision.
    fn is_fresh(&self) -> bool {
        match self.repo.revparse_single("HEAD") {
            Ok(ref head) if head.id() == self.revision => {
                // See comments in reset() for why we check this
                self.path.join(CHECKOUT_READY_LOCK).exists()
            }
            _ => false,
        }
    }

    /// Similar to [`reset()`]. This roughly performs `git reset --hard` to the
    /// revision of this checkout, with additional interrupt protection by a
    /// dummy file [`CHECKOUT_READY_LOCK`].
    ///
    /// If we're interrupted while performing a `git reset` (e.g., we die
    /// because of a signal) Cargo needs to be sure to try to check out this
    /// repo again on the next go-round.
    ///
    /// To enable this we have a dummy file in our checkout, [`.cargo-ok`],
    /// which if present means that the repo has been successfully reset and is
    /// ready to go. Hence if we start to do a reset, we make sure this file
    /// *doesn't* exist. The caller of [`reset`] has an option to perform additional operations
    /// (e.g. submodule update) before marking the check-out as ready.
    ///
    /// [`.cargo-ok`]: CHECKOUT_READY_LOCK
    fn reset(&self, gctx: &GlobalContext) -> CargoResult<CheckoutGuard> {
        let guard = CheckoutGuard::guard(&self.path);
        info!("reset {} to {}", self.repo.path().display(), self.revision);

        // Ensure libgit2 won't mess with newlines when we vendor.
        if let Ok(mut git_config) = self.repo.config() {
            git_config.set_bool("core.autocrlf", false)?;
        }

        let object = self.repo.find_object(self.revision, None)?;
        reset(&self.repo, &object, gctx)?;

        Ok(guard)
    }

    /// Like `git submodule update --recursive` but for this git checkout.
    ///
    /// This function respects `submodule.<name>.update = none`[^1] git config.
    /// Submodules set to `none` won't be fetched.
    ///
    /// [^1]: <https://git-scm.com/docs/git-submodule#Documentation/git-submodule.txt-none>
    fn update_submodules(&self, gctx: &GlobalContext) -> CargoResult<()> {
        return update_submodules(&self.repo, gctx, self.remote_url().as_str());

        /// Recursive helper for [`GitCheckout::update_submodules`].
        fn update_submodules(
            repo: &git2::Repository,
            gctx: &GlobalContext,
            parent_remote_url: &str,
        ) -> CargoResult<()> {
            debug!("update submodules for: {:?}", repo.workdir().unwrap());

            for mut child in repo.submodules()? {
                update_submodule(repo, &mut child, gctx, parent_remote_url).with_context(|| {
                    format!(
                        "failed to update submodule `{}`",
                        child.name().unwrap_or("")
                    )
                })?;
            }
            Ok(())
        }

        /// Update a single Git submodule, and recurse into its submodules.
        fn update_submodule(
            parent: &git2::Repository,
            child: &mut git2::Submodule<'_>,
            gctx: &GlobalContext,
            parent_remote_url: &str,
        ) -> CargoResult<()> {
            child.init(false)?;

            let child_url_str = child.url().ok_or_else(|| {
                anyhow::format_err!("non-utf8 url for submodule {:?}?", child.path())
            })?;

            // Skip the submodule if the config says not to update it.
            if child.update_strategy() == git2::SubmoduleUpdate::None {
                gctx.shell().status(
                    "Skipping",
                    format!(
                        "git submodule `{}` due to update strategy in .gitmodules",
                        child_url_str
                    ),
                )?;
                return Ok(());
            }

            let child_remote_url = absolute_submodule_url(parent_remote_url, child_url_str)?;

            // A submodule which is listed in .gitmodules but not actually
            // checked out will not have a head id, so we should ignore it.
            let Some(head) = child.head_id() else {
                return Ok(());
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
                        return update_submodules(&repo, gctx, &child_remote_url);
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
            gctx.shell()
                .status("Updating", format!("git submodule `{child_remote_url}`"))?;
            fetch(
                &mut repo,
                &child_remote_url,
                &reference,
                gctx,
                RemoteKind::GitDependency,
            )
            .with_context(|| {
                let name = child.name().unwrap_or("");
                format!("failed to fetch submodule `{name}` from {child_remote_url}",)
            })?;

            let obj = repo.find_object(head, None)?;
            reset(&repo, &obj, gctx)?;
            update_submodules(&repo, gctx, &child_remote_url)
        }
    }
}

/// See [`GitCheckout::reset`] for rationale on this type.
#[must_use]
struct CheckoutGuard {
    ok_file: PathBuf,
}

impl CheckoutGuard {
    fn guard(path: &Path) -> Self {
        let ok_file = path.join(CHECKOUT_READY_LOCK);
        let _ = paths::remove_file(&ok_file);
        Self { ok_file }
    }

    fn mark_ok(self) -> CargoResult<()> {
        let _ = paths::create(self.ok_file)?;
        Ok(())
    }
}

/// Constructs an absolute URL for a child submodule URL with its parent base URL.
///
/// Git only assumes a submodule URL is a relative path if it starts with `./`
/// or `../` [^1]. To fetch the correct repo, we need to construct an absolute
/// submodule URL.
///
/// At this moment it comes with some limitations:
///
/// * GitHub doesn't accept non-normalized URLs with relative paths.
///   (`ssh://git@github.com/rust-lang/cargo.git/relative/..` is invalid)
/// * `url` crate cannot parse SCP-like URLs.
///   (`git@github.com:rust-lang/cargo.git` is not a valid WHATWG URL)
///
/// To overcome these, this patch always tries [`Url::parse`] first to normalize
/// the path. If it couldn't, append the relative path as the last resort and
/// pray the remote git service supports non-normalized URLs.
///
/// See also rust-lang/cargo#12404 and rust-lang/cargo#12295.
///
/// [^1]: <https://git-scm.com/docs/git-submodule>
fn absolute_submodule_url<'s>(base_url: &str, submodule_url: &'s str) -> CargoResult<Cow<'s, str>> {
    let absolute_url = if ["./", "../"].iter().any(|p| submodule_url.starts_with(p)) {
        match Url::parse(base_url) {
            Ok(mut base_url) => {
                let path = base_url.path();
                if !path.ends_with('/') {
                    base_url.set_path(&format!("{path}/"));
                }
                let absolute_url = base_url.join(submodule_url).with_context(|| {
                    format!(
                        "failed to parse relative child submodule url `{submodule_url}` \
                        using parent base url `{base_url}`"
                    )
                })?;
                Cow::from(absolute_url.to_string())
            }
            Err(_) => {
                let mut absolute_url = base_url.to_string();
                if !absolute_url.ends_with('/') {
                    absolute_url.push('/');
                }
                absolute_url.push_str(submodule_url);
                Cow::from(absolute_url)
            }
        }
    } else {
        Cow::from(submodule_url)
    };

    Ok(absolute_url)
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
fn with_authentication<T, F>(
    gctx: &GlobalContext,
    url: &str,
    cfg: &git2::Config,
    mut f: F,
) -> CargoResult<T>
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
        // cred_helper_bad to make sure we only try the git credential helper
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
        Err(git2::Error::from_str("no authentication methods succeeded"))
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
        if let Ok(s) = gctx.get_env("USER").or_else(|_| gctx.get_env("USERNAME")) {
            attempts.push(s.to_string());
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
                Err(git2::Error::from_str("no authentication methods succeeded"))
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
            ErrorClass::Callback => {
                // This unwraps the git2 error. We're using the callback error
                // specifically to convey errors from Rust land through the C
                // callback interface. We don't need the `; class=Callback
                // (26)` that gets tacked on to the git2 error message.
                err = anyhow::format_err!("{}", e.message());
            }
            _ => {}
        }
    }

    Err(err)
}

/// `git reset --hard` to the given `obj` for the `repo`.
///
/// The `obj` is a commit-ish to which the head should be moved.
fn reset(repo: &git2::Repository, obj: &git2::Object<'_>, gctx: &GlobalContext) -> CargoResult<()> {
    let mut pb = Progress::new("Checkout", gctx);
    let mut opts = git2::build::CheckoutBuilder::new();
    opts.progress(|_, cur, max| {
        drop(pb.tick(cur, max, ""));
    });
    debug!("doing reset");
    repo.reset(obj, git2::ResetType::Hard, Some(&mut opts))?;
    debug!("reset done");
    Ok(())
}

/// Prepares the callbacks for fetching a git repository.
///
/// The main purpose of this function is to construct everything before a fetch.
/// This will attempt to setup a progress bar, the authentication for git,
/// ssh known hosts check, and the network retry mechanism.
///
/// The callback is provided a fetch options, which can be used by the actual
/// git fetch.
pub fn with_fetch_options(
    git_config: &git2::Config,
    url: &str,
    gctx: &GlobalContext,
    cb: &mut dyn FnMut(git2::FetchOptions<'_>) -> CargoResult<()>,
) -> CargoResult<()> {
    let mut progress = Progress::new("Fetch", gctx);
    let ssh_config = gctx.net_config()?.ssh.as_ref();
    let config_known_hosts = ssh_config.and_then(|ssh| ssh.known_hosts.as_ref());
    let diagnostic_home_config = gctx.diagnostic_home_config();
    network::retry::with_retry(gctx, || {
        // Hack: libgit2 disallows overriding the error from check_cb since v1.8.0,
        // so we store the error additionally and unwrap it later
        let mut check_cb_result = Ok(());
        let auth_result = with_authentication(gctx, url, git_config, |f| {
            let port = Url::parse(url).ok().and_then(|url| url.port());
            let mut last_update = Instant::now();
            let mut rcb = git2::RemoteCallbacks::new();
            // We choose `N=10` here to make a `300ms * 10slots ~= 3000ms`
            // sliding window for tracking the data transfer rate (in bytes/s).
            let mut counter = MetricsCounter::<10>::new(0, last_update);
            rcb.credentials(f);
            rcb.certificate_check(|cert, host| {
                match super::known_hosts::certificate_check(
                    gctx,
                    cert,
                    host,
                    port,
                    config_known_hosts,
                    &diagnostic_home_config,
                ) {
                    Ok(status) => Ok(status),
                    Err(e) => {
                        check_cb_result = Err(e);
                        // This is not really used because it'll be overridden by libgit2
                        // See https://github.com/libgit2/libgit2/commit/9a9f220119d9647a352867b24b0556195cb26548
                        Err(git2::Error::from_str(
                            "invalid or unknown remote ssh hostkey",
                        ))
                    }
                }
            });
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
                    let rate = HumanBytes(counter.rate() as u64);
                    format!(", {rate:.2}/s")
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
        });
        if auth_result.is_err() {
            check_cb_result?;
        }
        auth_result?;
        Ok(())
    })
}

/// Attempts to fetch the given git `reference` for a Git repository.
///
/// This is the main entry for git clone/fetch. It does the followings:
///
/// * Turns [`GitReference`] into refspecs accordingly.
/// * Dispatches `git fetch` using libgit2, gitoxide, or git CLI.
///
/// The `remote_url` argument is the git remote URL where we want to fetch from.
///
/// The `remote_kind` argument is a thing for [`-Zgitoxide`] shallow clones
/// at this time. It could be extended when libgit2 supports shallow clones.
///
/// [`-Zgitoxide`]: https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#gitoxide
pub fn fetch(
    repo: &mut git2::Repository,
    remote_url: &str,
    reference: &GitReference,
    gctx: &GlobalContext,
    remote_kind: RemoteKind,
) -> CargoResult<()> {
    if let Some(offline_flag) = gctx.offline_flag() {
        anyhow::bail!(
            "attempting to update a git repository, but {offline_flag} \
             was specified"
        )
    }

    let shallow = remote_kind.to_shallow_setting(repo.is_shallow(), gctx);

    // Flag to keep track if the rev is a full commit hash
    let mut fast_path_rev: bool = false;

    let oid_to_fetch = match github_fast_path(repo, remote_url, reference, gctx) {
        Ok(FastPathRev::UpToDate) => return Ok(()),
        Ok(FastPathRev::NeedsFetch(rev)) => Some(rev),
        Ok(FastPathRev::Indeterminate) => None,
        Err(e) => {
            debug!("failed to check github {:?}", e);
            None
        }
    };

    maybe_gc_repo(repo, gctx)?;

    clean_repo_temp_files(repo);

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
            } else if let Some(oid_to_fetch) = oid_to_fetch {
                fast_path_rev = true;
                refspecs.push(format!("+{0}:refs/commit/{0}", oid_to_fetch));
            } else if !matches!(shallow, gix::remote::fetch::Shallow::NoChange)
                && rev.parse::<Oid>().is_ok()
            {
                // There is a specific commit to fetch and we will do so in shallow-mode only
                // to not disturb the previous logic.
                // Note that with typical settings for shallowing, we will just fetch a single `rev`
                // as single commit.
                // The reason we write to `refs/remotes/origin/HEAD` is that it's of special significance
                // when during `GitReference::resolve()`, but otherwise it shouldn't matter.
                refspecs.push(format!("+{0}:refs/remotes/origin/HEAD", rev));
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

    let result = if let Some(true) = gctx.net_config()?.git_fetch_with_cli {
        fetch_with_cli(repo, remote_url, &refspecs, tags, gctx)
    } else if gctx.cli_unstable().gitoxide.map_or(false, |git| git.fetch) {
        fetch_with_gitoxide(repo, remote_url, refspecs, tags, shallow, gctx)
    } else {
        fetch_with_libgit2(repo, remote_url, refspecs, tags, shallow, gctx)
    };

    if fast_path_rev {
        if let Some(oid) = oid_to_fetch {
            return result.with_context(|| format!("revision {} not found", oid));
        }
    }
    result
}

/// `gitoxide` uses shallow locks to assure consistency when fetching to and to avoid races, and to write
/// files atomically.
/// Cargo has its own lock files and doesn't need that mechanism for race protection, so a stray lock means
/// a signal interrupted a previous shallow fetch and doesn't mean a race is happening.
fn has_shallow_lock_file(err: &crate::sources::git::fetch::Error) -> bool {
    matches!(
        err,
        gix::env::collate::fetch::Error::Fetch(gix::remote::fetch::Error::Fetch(
            gix::protocol::fetch::Error::LockShallowFile(_)
        ))
    )
}

/// Attempts to use `git` CLI installed on the system to fetch a repository,
/// when the config value [`net.git-fetch-with-cli`][1] is set.
///
/// Unfortunately `libgit2` is notably lacking in the realm of authentication
/// when compared to the `git` command line. As a result, allow an escape
/// hatch for users that would prefer to use `git`-the-CLI for fetching
/// repositories instead of `libgit2`-the-library. This should make more
/// flavors of authentication possible while also still giving us all the
/// speed and portability of using `libgit2`.
///
/// [1]: https://doc.rust-lang.org/nightly/cargo/reference/config.html#netgit-fetch-with-cli
fn fetch_with_cli(
    repo: &mut git2::Repository,
    url: &str,
    refspecs: &[String],
    tags: bool,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let mut cmd = ProcessBuilder::new("git");
    cmd.arg("fetch");
    if tags {
        cmd.arg("--tags");
    } else {
        cmd.arg("--no-tags");
    }
    match gctx.shell().verbosity() {
        Verbosity::Normal => {}
        Verbosity::Verbose => {
            cmd.arg("--verbose");
        }
        Verbosity::Quiet => {
            cmd.arg("--quiet");
        }
    }
    cmd.arg("--force") // handle force pushes
        .arg("--update-head-ok") // see discussion in #2078
        .arg(url)
        .args(refspecs)
        // If cargo is run by git (for example, the `exec` command in `git
        // rebase`), the GIT_DIR is set by git and will point to the wrong
        // location. This makes sure GIT_DIR is always the repository path.
        .env("GIT_DIR", repo.path())
        // The reset of these may not be necessary, but I'm including them
        // just to be extra paranoid and avoid any issues.
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_OBJECT_DIRECTORY")
        .env_remove("GIT_ALTERNATE_OBJECT_DIRECTORIES")
        .cwd(repo.path());
    gctx.shell()
        .verbose(|s| s.status("Running", &cmd.to_string()))?;
    cmd.exec()?;
    Ok(())
}

fn fetch_with_gitoxide(
    repo: &mut git2::Repository,
    remote_url: &str,
    refspecs: Vec<String>,
    tags: bool,
    shallow: gix::remote::fetch::Shallow,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let git2_repo = repo;
    let config_overrides = cargo_config_to_gitoxide_overrides(gctx)?;
    let repo_reinitialized = AtomicBool::default();
    let res = oxide::with_retry_and_progress(
        git2_repo.path(),
        gctx,
        &|repo_path,
          should_interrupt,
          mut progress,
          url_for_authentication: &mut dyn FnMut(&gix::bstr::BStr)| {
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
            loop {
                let res = oxide::open_repo(
                    repo_path,
                    config_overrides.clone(),
                    oxide::OpenMode::ForFetch,
                )
                .map_err(crate::sources::git::fetch::Error::from)
                .and_then(|repo| {
                    debug!("initiating fetch of {refspecs:?} from {remote_url}");
                    let url_for_authentication = &mut *url_for_authentication;
                    let remote = repo
                        .remote_at(remote_url)?
                        .with_fetch_tags(if tags {
                            gix::remote::fetch::Tags::All
                        } else {
                            gix::remote::fetch::Tags::Included
                        })
                        .with_refspecs(
                            refspecs.iter().map(|s| s.as_str()),
                            gix::remote::Direction::Fetch,
                        )
                        .map_err(crate::sources::git::fetch::Error::Other)?;
                    let url = remote
                        .url(gix::remote::Direction::Fetch)
                        .expect("set at init")
                        .to_owned();
                    let connection = remote.connect(gix::remote::Direction::Fetch)?;
                    let mut authenticate = connection.configured_credentials(url)?;
                    let connection = connection.with_credentials(
                        move |action: gix::protocol::credentials::helper::Action| {
                            if let Some(url) = action
                                .context()
                                .and_then(|gctx| gctx.url.as_ref().filter(|url| *url != remote_url))
                            {
                                url_for_authentication(url.as_ref());
                            }
                            authenticate(action)
                        },
                    );
                    let outcome = connection
                        .prepare_fetch(&mut progress, gix::remote::ref_map::Options::default())?
                        .with_shallow(shallow.clone())
                        .receive(&mut progress, should_interrupt)?;
                    Ok(outcome)
                });
                let err = match res {
                    Ok(_) => break,
                    Err(e) => e,
                };
                debug!("fetch failed: {}", err);

                if !repo_reinitialized.load(Ordering::Relaxed)
                        // We check for errors that could occur if the configuration, refs or odb files are corrupted.
                        // We don't check for errors related to writing as `gitoxide` is expected to create missing leading
                        // folder before writing files into it, or else not even open a directory as git repository (which is
                        // also handled here).
                        && err.is_corrupted()
                    || has_shallow_lock_file(&err)
                {
                    repo_reinitialized.store(true, Ordering::Relaxed);
                    debug!(
                        "looks like this is a corrupt repository, reinitializing \
                     and trying again"
                    );
                    if oxide::reinitialize(repo_path).is_ok() {
                        continue;
                    }
                }

                return Err(err.into());
            }
            Ok(())
        },
    );
    if repo_reinitialized.load(Ordering::Relaxed) {
        *git2_repo = git2::Repository::open(git2_repo.path())?;
    }
    res
}

fn fetch_with_libgit2(
    repo: &mut git2::Repository,
    remote_url: &str,
    refspecs: Vec<String>,
    tags: bool,
    shallow: gix::remote::fetch::Shallow,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    debug!("doing a fetch for {remote_url}");
    let git_config = git2::Config::open_default()?;
    with_fetch_options(&git_config, remote_url, gctx, &mut |mut opts| {
        if tags {
            opts.download_tags(git2::AutotagOption::All);
        }
        if let gix::remote::fetch::Shallow::DepthAtRemote(depth) = shallow {
            opts.depth(0i32.saturating_add_unsigned(depth.get()));
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
            debug!("initiating fetch of {refspecs:?} from {remote_url}");
            let res = repo
                .remote_anonymous(remote_url)?
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

/// Attempts to `git gc` a repository.
///
/// Cargo has a bunch of long-lived git repositories in its global cache and
/// some, like the index, are updated very frequently. Right now each update
/// creates a new "pack file" inside the git database, and over time this can
/// cause bad performance and bad current behavior in libgit2.
///
/// One pathological use case today is where libgit2 opens hundreds of file
/// descriptors, getting us dangerously close to blowing out the OS limits of
/// how many fds we can have open. This is detailed in [#4403].
///
/// To try to combat this problem we attempt a `git gc` here. Note, though, that
/// we may not even have `git` installed on the system! As a result we
/// opportunistically try a `git gc` when the pack directory looks too big, and
/// failing that we just blow away the repository and start over.
///
/// In theory this shouldn't be too expensive compared to the network request
/// we're about to issue.
///
/// [#4403]: https://github.com/rust-lang/cargo/issues/4403
fn maybe_gc_repo(repo: &mut git2::Repository, gctx: &GlobalContext) -> CargoResult<()> {
    // Here we arbitrarily declare that if you have more than 100 files in your
    // `pack` folder that we need to do a gc.
    let entries = match repo.path().join("objects/pack").read_dir() {
        Ok(e) => e.count(),
        Err(_) => {
            debug!("skipping gc as pack dir appears gone");
            return Ok(());
        }
    };
    let max = gctx
        .get_env("__CARGO_PACKFILE_LIMIT")
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

/// Removes temporary files left from previous activity.
///
/// If libgit2 is interrupted while indexing pack files, it will leave behind
/// some temporary files that it doesn't clean up. These can be quite large in
/// size, so this tries to clean things up.
///
/// This intentionally ignores errors. This is only an opportunistic cleaning,
/// and we don't really care if there are issues (there's unlikely anything
/// that can be done).
///
/// The git CLI has similar behavior (its temp files look like
/// `objects/pack/tmp_pack_9kUSA8`). Those files are normally deleted via `git
/// prune` which is run by `git gc`. However, it doesn't know about libgit2's
/// filenames, so they never get cleaned up.
fn clean_repo_temp_files(repo: &git2::Repository) {
    let path = repo.path().join("objects/pack/pack_git2_*");
    let Some(pattern) = path.to_str() else {
        tracing::warn!("cannot convert {path:?} to a string");
        return;
    };
    let Ok(paths) = glob::glob(pattern) else {
        return;
    };
    for path in paths {
        if let Ok(path) = path {
            match paths::remove_file(&path) {
                Ok(_) => tracing::debug!("removed stale temp git file {path:?}"),
                Err(e) => {
                    tracing::warn!("failed to remove {path:?} while cleaning temp files: {e}")
                }
            }
        }
    }
}

/// Reinitializes a given Git repository. This is useful when a Git repository
/// seems corrupted and we want to start over.
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

/// Initializes a Git repository at `path`.
fn init(path: &Path, bare: bool) -> CargoResult<git2::Repository> {
    let mut opts = git2::RepositoryInitOptions::new();
    // Skip anything related to templates, they just call all sorts of issues as
    // we really don't want to use them yet they insist on being used. See #6240
    // for an example issue that comes up.
    opts.external_template(false);
    opts.bare(bare);
    Ok(git2::Repository::init_opts(&path, &opts)?)
}

/// The result of GitHub fast path check. See [`github_fast_path`] for more.
enum FastPathRev {
    /// The local rev (determined by `reference.resolve(repo)`) is already up to
    /// date with what this rev resolves to on GitHub's server.
    UpToDate,
    /// The following SHA must be fetched in order for the local rev to become
    /// up to date.
    NeedsFetch(Oid),
    /// Don't know whether local rev is up to date. We'll fetch _all_ branches
    /// and tags from the server and see what happens.
    Indeterminate,
}

/// Attempts GitHub's special fast path for testing if we've already got an
/// up-to-date copy of the repository.
///
/// Updating the index is done pretty regularly so we want it to be as fast as
/// possible. For registries hosted on GitHub (like the crates.io index) there's
/// a fast path available to use[^1] to tell us that there's no updates to be
/// made.
///
/// Note that this function should never cause an actual failure because it's
/// just a fast path. As a result, a caller should ignore `Err` returned from
/// this function and move forward on the normal path.
///
/// [^1]: <https://developer.github.com/v3/repos/commits/#get-the-sha-1-of-a-commit-reference>
fn github_fast_path(
    repo: &mut git2::Repository,
    url: &str,
    reference: &GitReference,
    gctx: &GlobalContext,
) -> CargoResult<FastPathRev> {
    let url = Url::parse(url)?;
    if !is_github(&url) {
        return Ok(FastPathRev::Indeterminate);
    }

    let local_object = resolve_ref(reference, repo).ok();

    let github_branch_name = match reference {
        GitReference::Branch(branch) => branch,
        GitReference::Tag(tag) => tag,
        GitReference::DefaultBranch => "HEAD",
        GitReference::Rev(rev) => {
            if rev.starts_with("refs/") {
                rev
            } else if looks_like_commit_hash(rev) {
                // `revparse_single` (used by `resolve`) is the only way to turn
                // short hash -> long hash, but it also parses other things,
                // like branch and tag names, which might coincidentally be
                // valid hex.
                //
                // We only return early if `rev` is a prefix of the object found
                // by `revparse_single`. Don't bother talking to GitHub in that
                // case, since commit hashes are permanent. If a commit with the
                // requested hash is already present in the local clone, its
                // contents must be the same as what is on the server for that
                // hash.
                //
                // If `rev` is not found locally by `revparse_single`, we'll
                // need GitHub to resolve it and get a hash. If `rev` is found
                // but is not a short hash of the found object, it's probably a
                // branch and we also need to get a hash from GitHub, in case
                // the branch has moved.
                if let Some(local_object) = local_object {
                    if is_short_hash_of(rev, local_object) {
                        debug!("github fast path already has {local_object}");
                        return Ok(FastPathRev::UpToDate);
                    }
                }
                // If `rev` is a full commit hash, the only thing it can resolve
                // to is itself. Don't bother talking to GitHub in that case
                // either. (This ensures that we always attempt to fetch the
                // commit directly even if we can't reach the GitHub API.)
                if let Some(oid) = rev_to_oid(rev) {
                    debug!("github fast path is already a full commit hash {rev}");
                    return Ok(FastPathRev::NeedsFetch(oid));
                }
                rev
            } else {
                debug!("can't use github fast path with `rev = \"{}\"`", rev);
                return Ok(FastPathRev::Indeterminate);
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
    let mut handle = gctx.http()?.borrow_mut();
    debug!("attempting GitHub fast path for {}", url);
    handle.get(true)?;
    handle.url(&url)?;
    handle.useragent("cargo")?;
    handle.follow_location(true)?; // follow redirects
    handle.http_headers({
        let mut headers = List::new();
        headers.append("Accept: application/vnd.github.3.sha")?;
        if let Some(local_object) = local_object {
            headers.append(&format!("If-None-Match: \"{}\"", local_object))?;
        }
        headers
    })?;

    let mut response_body = Vec::new();
    let mut transfer = handle.transfer();
    transfer.write_function(|data| {
        response_body.extend_from_slice(data);
        Ok(data.len())
    })?;
    transfer.perform()?;
    drop(transfer); // end borrow of handle so that response_code can be called

    let response_code = handle.response_code()?;
    if response_code == 304 {
        debug!("github fast path up-to-date");
        Ok(FastPathRev::UpToDate)
    } else if response_code == 200 {
        let oid_to_fetch = str::from_utf8(&response_body)?.parse::<Oid>()?;
        debug!("github fast path fetch {oid_to_fetch}");
        Ok(FastPathRev::NeedsFetch(oid_to_fetch))
    } else {
        // Usually response_code == 404 if the repository does not exist, and
        // response_code == 422 if exists but GitHub is unable to resolve the
        // requested rev.
        debug!("github fast path bad response code {response_code}");
        Ok(FastPathRev::Indeterminate)
    }
}

/// Whether a `url` is one from GitHub.
fn is_github(url: &Url) -> bool {
    url.host_str() == Some("github.com")
}

/// Whether a `rev` looks like a commit hash (ASCII hex digits).
fn looks_like_commit_hash(rev: &str) -> bool {
    rev.len() >= 7 && rev.chars().all(|ch| ch.is_ascii_hexdigit())
}

/// Whether `rev` is a shorter hash of `oid`.
fn is_short_hash_of(rev: &str, oid: Oid) -> bool {
    let long_hash = oid.to_string();
    match long_hash.get(..rev.len()) {
        Some(truncated_long_hash) => truncated_long_hash.eq_ignore_ascii_case(rev),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::absolute_submodule_url;

    #[test]
    fn test_absolute_submodule_url() {
        let cases = [
            (
                "ssh://git@gitub.com/rust-lang/cargo",
                "git@github.com:rust-lang/cargo.git",
                "git@github.com:rust-lang/cargo.git",
            ),
            (
                "ssh://git@gitub.com/rust-lang/cargo",
                "./",
                "ssh://git@gitub.com/rust-lang/cargo/",
            ),
            (
                "ssh://git@gitub.com/rust-lang/cargo",
                "../",
                "ssh://git@gitub.com/rust-lang/",
            ),
            (
                "ssh://git@gitub.com/rust-lang/cargo",
                "./foo",
                "ssh://git@gitub.com/rust-lang/cargo/foo",
            ),
            (
                "ssh://git@gitub.com/rust-lang/cargo/",
                "./foo",
                "ssh://git@gitub.com/rust-lang/cargo/foo",
            ),
            (
                "ssh://git@gitub.com/rust-lang/cargo/",
                "../foo",
                "ssh://git@gitub.com/rust-lang/foo",
            ),
            (
                "ssh://git@gitub.com/rust-lang/cargo",
                "../foo",
                "ssh://git@gitub.com/rust-lang/foo",
            ),
            (
                "ssh://git@gitub.com/rust-lang/cargo",
                "../foo/bar/../baz",
                "ssh://git@gitub.com/rust-lang/foo/baz",
            ),
            (
                "git@github.com:rust-lang/cargo.git",
                "ssh://git@gitub.com/rust-lang/cargo",
                "ssh://git@gitub.com/rust-lang/cargo",
            ),
            (
                "git@github.com:rust-lang/cargo.git",
                "./",
                "git@github.com:rust-lang/cargo.git/./",
            ),
            (
                "git@github.com:rust-lang/cargo.git",
                "../",
                "git@github.com:rust-lang/cargo.git/../",
            ),
            (
                "git@github.com:rust-lang/cargo.git",
                "./foo",
                "git@github.com:rust-lang/cargo.git/./foo",
            ),
            (
                "git@github.com:rust-lang/cargo.git/",
                "./foo",
                "git@github.com:rust-lang/cargo.git/./foo",
            ),
            (
                "git@github.com:rust-lang/cargo.git",
                "../foo",
                "git@github.com:rust-lang/cargo.git/../foo",
            ),
            (
                "git@github.com:rust-lang/cargo.git/",
                "../foo",
                "git@github.com:rust-lang/cargo.git/../foo",
            ),
            (
                "git@github.com:rust-lang/cargo.git",
                "../foo/bar/../baz",
                "git@github.com:rust-lang/cargo.git/../foo/bar/../baz",
            ),
        ];

        for (base_url, submodule_url, expected) in cases {
            let url = absolute_submodule_url(base_url, submodule_url).unwrap();
            assert_eq!(
                expected, url,
                "base `{base_url}`; submodule `{submodule_url}`"
            );
        }
    }
}

/// Turns a full commit hash revision into an oid.
///
/// Git object ID is supposed to be a hex string of 20 (SHA1) or 32 (SHA256) bytes.
/// Its length must be double to the underlying bytes (40 or 64),
/// otherwise libgit2 would happily zero-pad the returned oid.
///
/// See:
///
/// * <https://github.com/rust-lang/cargo/issues/13188>
/// * <https://github.com/rust-lang/cargo/issues/13968>
pub(super) fn rev_to_oid(rev: &str) -> Option<Oid> {
    Oid::from_str(rev)
        .ok()
        .filter(|oid| oid.as_bytes().len() * 2 == rev.len())
}
