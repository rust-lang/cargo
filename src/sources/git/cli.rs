//! Git operations through the `git` executable.

use crate::context::ProgressWhen;
use crate::sources::git::utils::GitDatabase;
use crate::sources::git::utils::note_git_config;
use crate::sources::git::utils::note_github_pull_request;
use crate::sources::git::utils::note_ssh_config;
use crate::sources::git::utils::set_checkout_config;
use crate::util::GlobalContext;
use crate::util::errors::CargoResult;
use crate::util::errors::GitCliError;
use crate::util::network;

use cargo_util::ProcessBuilder;
use cargo_util::Stdio;
use cargo_util::paths;
use cargo_util_terminal::Verbosity;
use tracing::debug;

use std::path::Path;

pub(crate) fn git_version() -> Option<GitVersion> {
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
pub(crate) struct GitVersion {
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
pub(crate) fn fetch_with_cli(
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

    // The implicit gc from `git fetch` is redundant
    // as we run `maybe_gc_repo` unconditionally for all git backend before fetch.
    cmd.arg("-c").arg("maintenance.auto=false");
    cmd.arg("-c").arg("gc.auto=0");

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

    if gctx.shell().verbosity() == Verbosity::Verbose {
        cmd.arg("--verbose");
    } else if git_cli_progress(gctx) {
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

/// Whether `git` CLI should report progress.
fn git_cli_progress(gctx: &GlobalContext) -> bool {
    let progress_config = gctx.progress_config();
    match progress_config.when {
        ProgressWhen::Always => true,
        ProgressWhen::Never => false,
        ProgressWhen::Auto => {
            // Recreate the same conditions used with `Progress`
            let width = progress_config
                .width
                .or_else(|| gctx.shell().err_width().progress_max_width());
            gctx.shell().progress_supported() && width.is_some()
        }
    }
}

/// Whether to check out git dependencies with `git` CLI.
pub(crate) fn should_checkout_with_cli(gctx: &GlobalContext) -> bool {
    let opt_out = gctx.get_env("__CARGO_TEMPORARY_GIT_CLI_CHECKOUT_OPT_OUT");
    !matches!(opt_out, Ok("1")) && git_version().is_some()
}

/// Like [`checkout_with_libgit2`] but with `git` CLI.
///
/// [`checkout_with_libgit2`]: super::libgit2::checkout_with_libgit2
#[tracing::instrument(skip(database, gctx))]
pub(crate) fn checkout_with_cli(
    into: &Path,
    database: &GitDatabase,
    revision: git2::Oid,
    gctx: &GlobalContext,
) -> CargoResult<git2::Repository> {
    debug!(target: "git-checkout", backend = "git-cli");

    // This is currently implemented as
    //
    // 1. `git init`
    // 2. hardlink git database objects
    // 3. `git checkout --detach`
    //
    // We don't use `git clone --local`
    // because git doesn't hardlink from a shallow database
    // but falls back to a regular clone,
    // and it fetches nothing as our database has no `refs/heads/*`.

    let git_version = git_version();

    let git_dir = into.join(".git");

    let mut init = local_git_cmd(&git_dir, git_version);
    init.args(&["init", "--quiet", "--template="]);

    // https://github.com/git/git/blob/b3d7a52fac39193503a0b6728771d1bf6a161464/Documentation/git-init.txt?plain=1#L51-L54
    let min_version_object_format = GitVersion {
        major: 2,
        minor: 27,
        patch: 0,
    };
    if git_version.is_some_and(|v| min_version_object_format <= v) {
        let format = database.repo().object_format().str();
        init.arg(format!("--object-format={format}"));
    }

    // in v3 reftable will be the new default, so pin in case there is incompatibility
    // https://github.com/git/git/blob/3c2a3fdc388747b9eaf4a4a4f2035c1c9ddb26d0/Documentation/git-init.txt?plain=1#L61-L65
    let min_version_ref_format = GitVersion {
        major: 2,
        minor: 44,
        patch: 0,
    };
    if git_version.is_some_and(|v| min_version_ref_format <= v) {
        init.arg("--ref-format=files");
    }

    init.arg(into);

    gctx.shell()
        .verbose(|s| s.status("Running", &init.to_string()))?;
    init.exec_with_output()?;

    // This is the same as `git clone --local` and libgit2's local clone.
    // https://github.com/git/git/blob/1630431f326e15fcde608827b5ff38422528eb59/Documentation/git-clone.adoc?plain=1#L45-L52
    let db_git_dir = database.repo().path();
    link_or_copy_dir(&db_git_dir.join("objects"), &git_dir.join("objects"))?;
    if database.repo().is_shallow() {
        paths::copy(db_git_dir.join("shallow"), git_dir.join("shallow"))?;
    }

    let repo = git2::Repository::open(into)?;
    set_checkout_config(&repo)?;

    let mut checkout = local_git_cmd(&git_dir, git_version);

    checkout.arg("-c").arg("advice.detachedHead=false");

    checkout.args(&["checkout", "--quiet", "--force", "--detach"]);

    // https://github.com/git/git/blob/1630431f326e15fcde608827b5ff38422528eb59/Documentation/RelNotes/2.7.0.adoc?plain=1#L67-L70
    let min_version_progress = GitVersion {
        major: 2,
        minor: 7,
        patch: 0,
    };
    if git_cli_progress(gctx) && git_version.is_some_and(|v| min_version_progress <= v) {
        checkout.arg("--progress");
    }

    checkout.arg(revision.to_string()).cwd(into);

    gctx.shell()
        .verbose(|s| s.status("Running", &checkout.to_string()))?;

    checkout.exec()?;

    Ok(repo)
}

/// `git` command for isolated local operations in Cargo-managed Git repositories.
///
/// We should ignore user and system configurations as well as all `GIT_` env vars
/// so checkouts don't depend on the host machine.
fn local_git_cmd(git_dir: &Path, git_version: Option<GitVersion>) -> ProcessBuilder {
    const NULL_DEVICE: &str = if cfg!(windows) { "NUL" } else { "/dev/null" };

    let mut cmd = ProcessBuilder::new("git");

    #[expect(
        clippy::disallowed_methods,
        reason = "isolate git process from inherited environment"
    )]
    for (key, _) in std::env::vars_os() {
        let Some(key) = key.to_str() else {
            continue;
        };

        // `GIT_EXEC_PATH` may be needed to locate git subcommands
        if key.eq_ignore_ascii_case("GIT_EXEC_PATH") {
            continue;
        }

        // ignore case for Windows
        let git_prefix = "GIT_";
        if key
            .get(..git_prefix.len())
            .is_some_and(|p| p.eq_ignore_ascii_case(git_prefix))
        {
            cmd.env_remove(key);
        }
    }

    cmd.arg("-c").arg("core.fsmonitor=false");
    cmd.arg("-c")
        .arg(format!("core.attributesFile={NULL_DEVICE}"));

    cmd.env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_ATTR_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", NULL_DEVICE);

    // There is no way to skip per-user config until git 2.32.0 added `GIT_CONFIG_GLOBAL`,
    // so hide `$HOME` and `$XDG_CONFIG_HOME` behind a nonexistent path.
    // https://github.com/git/git/blob/1630431f326e15fcde608827b5ff38422528eb59/Documentation/RelNotes/2.32.0.adoc?plain=1#L88-L93
    let min_version_config_global = GitVersion {
        major: 2,
        minor: 32,
        patch: 0,
    };
    if !git_version.is_some_and(|v| min_version_config_global <= v) {
        let no_home = git_dir.join("cargo-no-home");
        cmd.env("HOME", &no_home).env("XDG_CONFIG_HOME", &no_home);
    }

    cmd
}

/// Recursively hardlinks files from `src` into `dst`,
/// falling back to copying (e.g. across filesystems).
fn link_or_copy_dir(src: &Path, dst: &Path) -> CargoResult<()> {
    for entry in walkdir::WalkDir::new(src) {
        let entry = entry?;
        let rel = entry.path().strip_prefix(src).expect("walked from src");
        let target = dst.join(rel);
        if entry.file_type().is_dir() {
            paths::create_dir_all(&target)?;
        } else if std::fs::hard_link(entry.path(), &target).is_err() {
            paths::copy(entry.path(), &target)?;
        }
    }
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
