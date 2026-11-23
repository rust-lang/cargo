//! Git operations through libgit2.

use crate::sources::git::utils::note_github_pull_request;
use crate::sources::git::utils::reinitialize;
use crate::util::GlobalContext;
use crate::util::HumanBytes;
use crate::util::MetricsCounter;
use crate::util::Progress;
use crate::util::errors::CargoResult;
use crate::util::network;
use crate::util::network::http::http_handle;
use crate::util::network::http::needs_custom_http_transport;

use git2::ErrorClass;
use tracing::debug;
use url::Url;

use std::sync::Once;
use std::time::{Duration, Instant};

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
                let msg = format!(
                    concat!(
                        "network failure seems to have happened\n",
                        "if a proxy or similar is necessary `net.git-fetch-with-cli` may help here\n",
                        "https://doc.rust-lang.org/cargo/reference/config.html#netgit-fetch-with-cli",
                        "{}"
                    ),
                    note_github_pull_request(url).unwrap_or_default()
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

/// Prepares the callbacks for fetching a git repository.
///
/// The main purpose of this function is to construct everything before a fetch.
/// This will attempt to setup a progress bar, the authentication for git,
/// ssh known hosts check, and the network retry mechanism.
///
/// The callback is provided a fetch options, which can be used by the actual
/// git fetch.
pub(super) fn with_fetch_options(
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

#[tracing::instrument(skip(repo, gctx))]
pub(super) fn fetch_with_libgit2(
    repo: &mut git2::Repository,
    remote_url: &str,
    refspecs: Vec<String>,
    tags: bool,
    shallow: gix::remote::fetch::Shallow,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    debug!(target: "git-fetch", backend = "libgit2");

    static INIT: Once = Once::new();
    INIT.call_once(|| {
        // SAFETY:
        //
        // The unsafety of the registration function derives from two aspects:
        //
        // 1. This call must be synchronized with all other registration calls as
        //    well as construction of new transports.
        // 2. The argument is leaked.
        //
        // We're clear on point (1) because this is the only `register` call we make in the
        // cargo-binary and cargo-library is not officially supported and it is under a lock.
        // Technically, `git2_curl` also has a lock but that isn't part of their API guarantees.
        //
        // We're mostly clear on point (2) because we'd only free it after everything is done anyway
        unsafe { init_git_transports(gctx) };
    });

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

/// Configure libgit2 to use libcurl if necessary.
///
/// If the user has a non-default network configuration, then libgit2 will be
/// configured to use libcurl instead of the built-in networking support so
/// that those configuration settings can be used.
///
/// # Safety
///
/// See [git2_curl::register]
#[tracing::instrument(skip_all)]
unsafe fn init_git_transports(gctx: &GlobalContext) {
    match needs_custom_http_transport(gctx) {
        Ok(true) => {}
        _ => return,
    }

    let handle = match http_handle(gctx) {
        Ok(handle) => handle,
        Err(..) => return,
    };

    unsafe {
        git2_curl::register(handle);
    }
}
