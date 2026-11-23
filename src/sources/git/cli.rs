//! Git operations through the `git` executable.

use crate::context::ProgressWhen;
use crate::sources::git::utils::note_git_config;
use crate::sources::git::utils::note_github_pull_request;
use crate::sources::git::utils::note_ssh_config;
use crate::util::GlobalContext;
use crate::util::errors::CargoResult;
use crate::util::errors::GitCliError;
use crate::util::network;

use cargo_util::ProcessBuilder;
use cargo_util::Stdio;
use cargo_util_terminal::Verbosity;
use tracing::debug;

pub(super) fn git_version() -> Option<GitVersion> {
    #[tracing::instrument(skip_all)]
    fn git_version() -> Option<GitVersion> {
        use std::process::Stdio;

        let output = std::process::Command::new("git")
            .arg("--version")
            .stdin(Stdio::null())
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let Ok(stdout) = String::from_utf8(output.stdout) else {
            return None;
        };
        // All else fails, we can at least report the lowest common denominator of what we support
        let default_version = GitVersion {
            major: 2,
            minor: 0,
            patch: 0,
        };
        Some(GitVersion::from_version_stdout(&stdout).unwrap_or(default_version))
    }

    static CACHE: std::sync::OnceLock<Option<GitVersion>> = std::sync::OnceLock::new();
    *CACHE.get_or_init(git_version)
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[allow(unused)]
pub(super) struct GitVersion {
    major: usize,
    minor: usize,
    patch: usize,
}

impl GitVersion {
    fn from_version_stdout(stdout: &str) -> Result<Self, anyhow::Error> {
        // See https://github.com/git/git/blob/f78ce2f7b6df702f93d40b85d6bda92a3f65da79/help.c#L779-L784
        let Some(version) = stdout.strip_prefix("git version ") else {
            anyhow::bail!("unrecognized `git --version` output: {stdout}")
        };
        let (version, _) = version.split_once(" ").unwrap_or((version, ""));
        let (version, _) = version.split_once("\n").unwrap_or((version, ""));
        version.parse()
    }
}

impl std::str::FromStr for GitVersion {
    type Err = anyhow::Error;

    fn from_str(version: &str) -> Result<Self, Self::Err> {
        let unreleased = "GIT";

        let s = version;
        let (major, s) = s.split_once(".").unwrap_or((s, ""));
        let mut major: usize = major.parse().map_err(|_err| {
            anyhow::format_err!("unrecognized major version `{major}` in `{version}`")
        })?;
        let (minor, s) = s.split_once(".").unwrap_or((s, ""));
        let mut minor: usize = if minor == unreleased {
            0
        } else {
            minor.parse().map_err(|_err| {
                anyhow::format_err!("unrecognized minor version `{minor}` in `{version}`")
            })?
        };
        let (patch, s) = s.split_once(".").unwrap_or((s, ""));
        let mut patch: usize = if patch == unreleased {
            0
        } else {
            patch.parse().map_err(|_err| {
                anyhow::format_err!("unrecognized patch version `{patch}` in `{version}`")
            })?
        };

        let more = s;
        if !more.is_empty() {
            // `more` can be either pre-release or post-release.
            // The difference should be minimal, so be conservative and assume they are pre-release
            if let Some(sub) = patch.checked_sub(1) {
                patch = sub;
            } else if let Some(sub) = minor.checked_sub(1) {
                minor = sub;
            } else if let Some(sub) = major.checked_sub(1) {
                major = sub;
            }
        }

        Ok(Self {
            major,
            minor,
            patch,
        })
    }
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
#[tracing::instrument(skip(repo, gctx))]
pub(super) fn fetch_with_cli(
    repo: &git2::Repository,
    url: &str,
    refspecs: &[String],
    tags: bool,
    shallow: gix::remote::fetch::Shallow,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    debug!(target: "git-fetch", backend = "git-cli");

    let Some(git_version) = git_version() else {
        anyhow::bail!(
            "`git` is not available
help: to still use `git` for fetching, please install it
help: to use Cargo's native git support, re-try with `net.git-fetch-with-cli = false`
https://doc.rust-lang.org/cargo/reference/config.html#netgit-fetch-with-cli"
        );
    };

    let mut cmd = ProcessBuilder::new("git");
    // Avoid potential for unused work that may also hang (#15775)
    cmd.arg("-c").arg("core.fsmonitor=false");
    // Avoid warnings with `--no-show-forced-updates`
    cmd.arg("-c").arg("advice.fetchShowForcedUpdates=false");
    // Have 429's retried in git, where supported
    let max_retries = gctx
        .net_config()?
        .retry
        .unwrap_or(network::retry::MAX_RETRY_DEFAULT);
    cmd.arg("-c").arg(format!("http.maxRetries={max_retries}"));
    cmd.arg("-c").arg("http.retryAfter=1"); // `network::retry` has a growing value starting with 500ms, so choosing 1s
    cmd.arg("-c").arg(format!(
        "http.maxRetryTime={}",
        network::retry::MAX_RETRY_SLEEP_S
    ));

    cmd.arg("fetch");
    if tags {
        cmd.arg("--tags");
    } else {
        cmd.arg("--no-tags");
    }
    if let gix::remote::fetch::Shallow::DepthAtRemote(depth) = shallow {
        let depth = 0i32.saturating_add_unsigned(depth.get());
        cmd.arg(format!("--depth={depth}"));
    }

    let progress_config = gctx.progress_config();
    let progress = match progress_config.when {
        ProgressWhen::Always => true,
        ProgressWhen::Never => false,
        ProgressWhen::Auto => {
            // Recreate the same conditions used with `Progress`
            let width = progress_config
                .width
                .or_else(|| gctx.shell().err_width().progress_max_width());
            gctx.shell().progress_supported() && width.is_some()
        }
    };
    if gctx.shell().verbosity() == Verbosity::Verbose {
        cmd.arg("--verbose");
    } else if progress {
        cmd.arg("--progress");
        // https://github.com/git/git/blob/1630431f326e15fcde608827b5ff38422528eb59/Documentation/RelNotes/2.41.0.adoc?plain=1#L113-L114
        let min_version_porcelain = GitVersion {
            major: 2,
            minor: 41,
            patch: 0,
        };
        if min_version_porcelain <= git_version {
            // Move ref update status to `stdout` and silence it
            cmd.arg("--porcelain").stdout(Stdio::Null);
        }
    } else {
        cmd.arg("--quiet");
    }

    // https://github.com/git/git/blob/1630431f326e15fcde608827b5ff38422528eb59/Documentation/RelNotes/2.23.0.adoc?plain=1#L56-L59
    let min_version_no_show_forced_update = GitVersion {
        major: 2,
        minor: 23,
        patch: 0,
    };
    if min_version_no_show_forced_update <= git_version {
        // skip unneeded expensive calculations
        cmd.arg("--no-show-forced-updates");
    }

    cmd.arg("--force") // handle force pushes
        .arg("--update-head-ok") // see discussion in #2078
        .arg("--recurse-submodules=no") // we handle this, incompatible with `--porcelain`
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
    // https://github.com/git/git/blob/1630431f326e15fcde608827b5ff38422528eb59/Documentation/RelNotes/2.54.0.adoc?plain=1#L107
    let min_version_max_retries = GitVersion {
        major: 2,
        minor: 54,
        patch: 0,
    };
    network::retry::with_retry(gctx, || {
        cmd.exec().map_err(|_error| {
            let spurious = git_version < min_version_max_retries;
            let pr_hint = note_github_pull_request(url).unwrap_or_default();
            let ssh_config_hint = note_ssh_config(gctx).unwrap_or_default().unwrap_or_default();
            let git_config_hint = note_git_config(gctx).unwrap_or_default().unwrap_or_default();
            let with_depth = if let gix::remote::fetch::Shallow::DepthAtRemote(depth) = shallow {
                format!(" with depth={depth}")
            } else {
                "".to_owned()
            };
            GitCliError::new(anyhow::format_err!("`git fetch` failed for {url}{with_depth}{pr_hint}{ssh_config_hint}{git_config_hint}"))
                .spurious(spurious)
                .workaround(
                    "help: re-try with `net.git-fetch-with-cli = false` to see if it resolves the problem
https://doc.rust-lang.org/cargo/reference/config.html#netgit-fetch-with-cli",
                )
                .into()
        })
    })?;

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn git_version_empty() {
        let input = "";
        GitVersion::from_version_stdout(input).unwrap_err();
    }

    #[test]
    fn git_version_version() {
        let input = "git version 2.19.0";
        let actual = GitVersion::from_version_stdout(input).unwrap();
        let expected = GitVersion {
            major: 2,
            minor: 19,
            patch: 0,
        };
        assert_eq!(actual, expected);
    }

    #[test]
    fn git_version_version_nl() {
        let input = "git version 2.19.0\n";
        let actual = GitVersion::from_version_stdout(input).unwrap();
        let expected = GitVersion {
            major: 2,
            minor: 19,
            patch: 0,
        };
        assert_eq!(actual, expected);
    }

    #[test]
    fn git_version_version_platform() {
        let input = "git version 2.19.0 (Apple Git-117)";
        let actual = GitVersion::from_version_stdout(input).unwrap();
        let expected = GitVersion {
            major: 2,
            minor: 19,
            patch: 0,
        };
        assert_eq!(actual, expected);
    }

    #[test]
    fn git_version_version_vendor() {
        let input = "git version 2.19.0.windows.1";
        let actual = GitVersion::from_version_stdout(input).unwrap();
        let expected = GitVersion {
            major: 2,
            minor: 18,
            patch: 0,
        };
        assert_eq!(actual, expected);
    }

    #[test]
    fn git_version_version_rc() {
        let input = "git version 2.19.0.rc0";
        let actual = GitVersion::from_version_stdout(input).unwrap();
        let expected = GitVersion {
            major: 2,
            minor: 18,
            patch: 0,
        };
        assert_eq!(actual, expected);
    }

    #[test]
    fn git_version_version_unreleased() {
        let input = "git version 2.55.GIT";
        let actual = GitVersion::from_version_stdout(input).unwrap();
        let expected = GitVersion {
            major: 2,
            minor: 55,
            patch: 0,
        };
        assert_eq!(actual, expected);
    }
}
