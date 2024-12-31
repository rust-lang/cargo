//! Helpers to gather the VCS information for `cargo package`.

use std::path::Path;
use std::path::PathBuf;

use anyhow::Context as _;
use cargo_util::paths;
use serde::Serialize;
use tracing::debug;

use crate::core::Package;
use crate::core::Workspace;
use crate::sources::PathEntry;
use crate::CargoResult;
use crate::GlobalContext;

use super::PackageOpts;

/// Represents the VCS information when packaging.
#[derive(Serialize)]
pub struct VcsInfo {
    git: GitVcsInfo,
    /// Path to the package within repo (empty string if root).
    path_in_vcs: String,
}

/// Represents the Git VCS information when packaging.
#[derive(Serialize)]
pub struct GitVcsInfo {
    sha1: String,
    /// Indicate whether or not the Git worktree is dirty.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    dirty: bool,
}

/// A shared builder for generating [`VcsInfo`] for packages inside the same workspace.
///
/// This is aimed to cache duplicate works like file system or VCS info lookups.
pub struct VcsInfoBuilder<'a, 'gctx> {
    ws: &'a Workspace<'gctx>,
    opts: &'a PackageOpts<'gctx>,
}

impl<'a, 'gctx> VcsInfoBuilder<'a, 'gctx> {
    pub fn new(
        ws: &'a Workspace<'gctx>,
        opts: &'a PackageOpts<'gctx>,
    ) -> VcsInfoBuilder<'a, 'gctx> {
        VcsInfoBuilder { ws, opts }
    }

    /// Builds an [`VcsInfo`] for the given `pkg` and its associated `src_files`.
    pub fn build(
        &mut self,
        pkg: &Package,
        src_files: &[PathEntry],
    ) -> CargoResult<Option<VcsInfo>> {
        check_repo_state(pkg, src_files, self.ws.gctx(), self.opts)
    }
}

/// Checks if the package source is in a *git* DVCS repository.
///
/// If *git*, and the source is *dirty* (e.g., has uncommitted changes),
/// and `--allow-dirty` has not been passed,
/// then `bail!` with an informative message.
/// Otherwise return the sha1 hash of the current *HEAD* commit,
/// or `None` if no repo is found.
#[tracing::instrument(skip_all)]
pub fn check_repo_state(
    p: &Package,
    src_files: &[PathEntry],
    gctx: &GlobalContext,
    opts: &PackageOpts<'_>,
) -> CargoResult<Option<VcsInfo>> {
    let Ok(repo) = git2::Repository::discover(p.root()) else {
        gctx.shell().verbose(|shell| {
            shell.warn(format!("no (git) VCS found for `{}`", p.root().display()))
        })?;
        // No Git repo found. Have to assume it is clean.
        return Ok(None);
    };

    let Some(workdir) = repo.workdir() else {
        debug!(
            "no (git) workdir found for repo at `{}`",
            repo.path().display()
        );
        // No git workdir. Have to assume it is clean.
        return Ok(None);
    };

    debug!("found a git repo at `{}`", workdir.display());
    let path = p.manifest_path();
    let path = paths::strip_prefix_canonical(path, workdir).unwrap_or_else(|_| path.to_path_buf());
    let Ok(status) = repo.status_file(&path) else {
        gctx.shell().verbose(|shell| {
            shell.warn(format!(
                "no (git) Cargo.toml found at `{}` in workdir `{}`",
                path.display(),
                workdir.display()
            ))
        })?;
        // No checked-in `Cargo.toml` found. This package may be irrelevant.
        // Have to assume it is clean.
        return Ok(None);
    };

    if !(status & git2::Status::IGNORED).is_empty() {
        gctx.shell().verbose(|shell| {
            shell.warn(format!(
                "found (git) Cargo.toml ignored at `{}` in workdir `{}`",
                path.display(),
                workdir.display()
            ))
        })?;
        // An ignored `Cargo.toml` found. This package may be irrelevant.
        // Have to assume it is clean.
        return Ok(None);
    }

    warn_symlink_checked_out_as_plain_text_file(gctx, src_files, &repo)?;

    debug!(
        "found (git) Cargo.toml at `{}` in workdir `{}`",
        path.display(),
        workdir.display(),
    );
    let path_in_vcs = path
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or("")
        .replace("\\", "/");
    let Some(git) = git(p, gctx, src_files, &repo, &opts)? else {
        // If the git repo lacks essensial field like `sha1`, and since this field exists from the beginning,
        // then don't generate the corresponding file in order to maintain consistency with past behavior.
        return Ok(None);
    };

    return Ok(Some(VcsInfo { git, path_in_vcs }));
}

/// Warns if any symlinks were checked out as plain text files.
///
/// Git config [`core.symlinks`] defaults to true when unset.
/// In git-for-windows (and git as well),
/// the config should be set to false explicitly when the repo was created,
/// if symlink support wasn't detected [^1].
///
/// We assume the config was always set at creation time and never changed.
/// So, if it is true, we don't bother users with any warning.
///
/// [^1]: <https://github.com/git-for-windows/git/blob/f1241afcc7956918d5da33ef74abd9cbba369247/setup.c#L2394-L2403>
///
/// [`core.symlinks`]: https://git-scm.com/docs/git-config#Documentation/git-config.txt-coresymlinks
fn warn_symlink_checked_out_as_plain_text_file(
    gctx: &GlobalContext,
    src_files: &[PathEntry],
    repo: &git2::Repository,
) -> CargoResult<()> {
    if repo
        .config()
        .and_then(|c| c.get_bool("core.symlinks"))
        .unwrap_or(true)
    {
        return Ok(());
    }

    if src_files.iter().any(|f| f.maybe_plain_text_symlink()) {
        let mut shell = gctx.shell();
        shell.warn(format_args!(
            "found symbolic links that may be checked out as regular files for git repo at `{}`\n\
            This might cause the `.crate` file to include incorrect or incomplete files",
            repo.workdir().unwrap().display(),
        ))?;
        let extra_note = if cfg!(windows) {
            "\nAnd on Windows, enable the Developer Mode to support symlinks"
        } else {
            ""
        };
        shell.note(format_args!(
            "to avoid this, set the Git config `core.symlinks` to `true`{extra_note}",
        ))?;
    }

    Ok(())
}

/// The real git status check starts from here.
fn git(
    pkg: &Package,
    gctx: &GlobalContext,
    src_files: &[PathEntry],
    repo: &git2::Repository,
    opts: &PackageOpts<'_>,
) -> CargoResult<Option<GitVcsInfo>> {
    // This is a collection of any dirty or untracked files. This covers:
    // - new/modified/deleted/renamed/type change (index or worktree)
    // - untracked files (which are "new" worktree files)
    // - ignored (in case the user has an `include` directive that
    //   conflicts with .gitignore).
    let mut dirty_files = Vec::new();
    collect_statuses(repo, &mut dirty_files)?;
    // Include each submodule so that the error message can provide
    // specifically *which* files in a submodule are modified.
    status_submodules(repo, &mut dirty_files)?;

    // Find the intersection of dirty in git, and the src_files that would
    // be packaged. This is a lazy n^2 check, but seems fine with
    // thousands of files.
    let cwd = gctx.cwd();
    let mut dirty_src_files: Vec<_> = src_files
        .iter()
        .filter(|src_file| dirty_files.iter().any(|path| src_file.starts_with(path)))
        .map(|p| p.as_ref())
        .chain(dirty_metadata_paths(pkg, repo)?.iter())
        .map(|path| {
            pathdiff::diff_paths(path, cwd)
                .as_ref()
                .unwrap_or(path)
                .display()
                .to_string()
        })
        .collect();
    let dirty = !dirty_src_files.is_empty();
    if !dirty || opts.allow_dirty {
        // Must check whetherthe repo has no commit firstly, otherwise `revparse_single` would fail on bare commit repo.
        // Due to lacking the `sha1` field, it's better not record the `GitVcsInfo` for consistency.
        if repo.is_empty()? {
            return Ok(None);
        }
        let rev_obj = repo.revparse_single("HEAD")?;
        Ok(Some(GitVcsInfo {
            sha1: rev_obj.id().to_string(),
            dirty,
        }))
    } else {
        dirty_src_files.sort_unstable();
        anyhow::bail!(
            "{} files in the working directory contain changes that were \
             not yet committed into git:\n\n{}\n\n\
             to proceed despite this and include the uncommitted changes, pass the `--allow-dirty` flag",
            dirty_src_files.len(),
            dirty_src_files.join("\n")
        )
    }
}

/// Checks whether files at paths specified in `package.readme` and
/// `package.license-file` have been modified.
///
/// This is required because those paths may link to a file outside the
/// current package root, but still under the git workdir, affecting the
/// final packaged `.crate` file.
fn dirty_metadata_paths(pkg: &Package, repo: &git2::Repository) -> CargoResult<Vec<PathBuf>> {
    let mut dirty_files = Vec::new();
    let workdir = repo.workdir().unwrap();
    let root = pkg.root();
    let meta = pkg.manifest().metadata();
    for path in [&meta.license_file, &meta.readme] {
        let Some(path) = path.as_deref().map(Path::new) else {
            continue;
        };
        let abs_path = paths::normalize_path(&root.join(path));
        if paths::strip_prefix_canonical(abs_path.as_path(), root).is_ok() {
            // Inside package root. Don't bother checking git status.
            continue;
        }
        if let Ok(rel_path) = paths::strip_prefix_canonical(abs_path.as_path(), workdir) {
            // Outside package root but under git workdir,
            if repo.status_file(&rel_path)? != git2::Status::CURRENT {
                dirty_files.push(if abs_path.is_symlink() {
                    // For symlinks, shows paths to symlink sources
                    workdir.join(rel_path)
                } else {
                    abs_path
                });
            }
        }
    }
    Ok(dirty_files)
}

/// Helper to collect dirty statuses for a single repo.
fn collect_statuses(repo: &git2::Repository, dirty_files: &mut Vec<PathBuf>) -> CargoResult<()> {
    let mut status_opts = git2::StatusOptions::new();
    // Exclude submodules, as they are being handled manually by recursing
    // into each one so that details about specific files can be
    // retrieved.
    status_opts
        .exclude_submodules(true)
        .include_ignored(true)
        .include_untracked(true);
    let repo_statuses = repo.statuses(Some(&mut status_opts)).with_context(|| {
        format!(
            "failed to retrieve git status from repo {}",
            repo.path().display()
        )
    })?;
    let workdir = repo.workdir().unwrap();
    let this_dirty = repo_statuses.iter().filter_map(|entry| {
        let path = entry.path().expect("valid utf-8 path");
        if path.ends_with("Cargo.lock") && entry.status() == git2::Status::IGNORED {
            // It is OK to include Cargo.lock even if it is ignored.
            return None;
        }
        // Use an absolute path, so that comparing paths is easier
        // (particularly with submodules).
        Some(workdir.join(path))
    });
    dirty_files.extend(this_dirty);
    Ok(())
}

/// Helper to collect dirty statuses while recursing into submodules.
fn status_submodules(repo: &git2::Repository, dirty_files: &mut Vec<PathBuf>) -> CargoResult<()> {
    for submodule in repo.submodules()? {
        // Ignore submodules that don't open, they are probably not initialized.
        // If its files are required, then the verification step should fail.
        if let Ok(sub_repo) = submodule.open() {
            status_submodules(&sub_repo, dirty_files)?;
            collect_statuses(&sub_repo, dirty_files)?;
        }
    }
    Ok(())
}
